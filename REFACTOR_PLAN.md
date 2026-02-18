# Comprehensive Refactor Plan

## Overview

Full architectural review of the Flowsurface codebase across all three layers (data, exchange, src/UI) following the addition of stream-based multi-connection data feed support. This document catalogs every issue found and provides a prioritized implementation plan.

**Review scope**: 55+ data files, 31 exchange files, 90+ UI/src files
**Total issues found**: ~190 across all layers

---

## PART 1: CRITICAL ARCHITECTURAL GAPS

### 1.1 Sidebar Connections Quick-Menu — NOT IMPLEMENTED

**Status**: Entirely missing
**Vision**: Sidebar popover/dropdown showing up to 5 connections (live first, then recent historical). Each with connect/disconnect toggle. "Manage" button at bottom opens the full dialog.

**Current state**: Sidebar has a `data_feeds_button` (folder icon) that opens the full `DataFeedsModal` as a sidebar-anchored panel. No lightweight connections popover exists. No `Menu::Connections` variant in sidebar config.

**What needs to happen**:
- Add `Menu::Connections` variant to `data/src/config/sidebar.rs`
- Create new `src/modal/pane/connections_menu.rs` — lightweight popover component
- Show up to 5 connections: all live feeds first, then recent historical feeds to fill remaining slots
- Each row: status dot, name, provider badge, connect/disconnect button
- Bottom: "Manage Connections" button that closes popover and opens full DataFeedsModal
- Wire into sidebar.rs as a new button (network/plug icon) above the existing data_feeds_button

### 1.2 Data Feeds "Manage" Dialog — WRONG LAYOUT

**Status**: Partially implemented with wrong layout
**Vision**: Large center-screen dialog with left panel (selectable connection list) and right panel (edit form for selected connection). Save/Cancel at bottom.

**Current state**: `DataFeedsModal` is a single-column max-width-500 panel that shows EITHER the list OR the edit form, never both. Uses `View` enum to switch between states.

**What needs to happen**:
- Redesign `DataFeedsModal` as a split-pane layout (list-detail pattern)
- Left panel (~200px): scrollable list of all connections, each selectable, with status indicators
- Right panel (~400px): edit form for selected connection, or empty state prompt
- Selected connection highlighted in left panel
- Add/delete buttons in left panel footer
- Save/Cancel buttons in right panel footer
- Dialog should be wider (~700px) and use `main_dialog_modal` for center positioning
- Remove the separate List/Edit/Create view switching — always show both panels

### 1.3 Rithmic Stream Events Never Reach Charts — DEAD END

**Status**: Events buffered but never routed
**Location**: `src/app/update.rs:930-995`

The `RithmicStreamEvent` handler has TODO comments:
```rust
// TODO: Route to active dashboard panes when live
// streaming is integrated into the chart system
```

Trade and depth events from Rithmic are logged but discarded. Charts never receive real-time data.

**What needs to happen**:
- Route `TradeReceived` events to active chart panes via `Dashboard::Message::ExchangeEvent`
- Route `DepthReceived` events to depth panels (ladder, heatmap)
- Match stream events to panes by ticker symbol
- Handle event ordering (real-time events appended after historical data)

### 1.4 Rithmic Streaming Task Structure — FUNDAMENTALLY BROKEN

**Status**: One-shot task pretending to be long-lived stream
**Location**: `src/app/update.rs:832-902`

The streaming is launched via `Task::perform(async { ... }, |_| ConnectionLost)`. This is a fire-and-forget task that:
- Blocks until the stream ends
- Returns a single `ConnectionLost` message when done
- Cannot produce intermediate messages (trades/depth) back to the app

Events are pushed to a global `RITHMIC_EVENTS` buffer and polled by subscription every 50ms. But the subscription only drains ONE event per poll cycle (`events.into_iter().next()`), creating a massive bottleneck under load.

**What needs to happen**:
- Fix subscription to drain ALL events per poll: `events.drain(..).collect::<Vec<_>>()`
- Process all drained events in a batch, not just `.next()`
- Consider replacing global buffer with proper channel-based subscription
- Add backpressure handling for high-frequency data

### 1.5 No Real-time + Historical Data Merge Path

**Status**: Not implemented
**Impact**: Cannot seamlessly combine Rithmic real-time with Databento historical

User starts Rithmic at 10 AM but wants chart from 9 AM. Currently no mechanism to:
- Query Databento for 9-10 AM historical data
- Query Rithmic for 10 AM+ real-time data
- Merge into continuous timeline

**What needs to happen**:
- Composite repository in `data/src/repository/composite.rs` needs to orchestrate multi-source queries
- Feed merger (`data/src/services/feed_merger.rs`) needs to handle real-time + historical segments
- Chart loading must support incremental append (historical base + live stream overlay)

---

## PART 2: DATA LAYER ISSUES

### 2.1 CRITICAL: Mutex Poisoning in market_data.rs

**File**: `data/src/services/market_data.rs`
**Lines**: 122, 142-144, 155-156, 178-179, 193-194, 212-213, 231-232, 251-252, 272-273

All 9+ `.lock().unwrap()` calls will panic if any thread panics while holding the lock.

**Fix**: Replace all with `.lock().unwrap_or_else(|e| e.into_inner())`

### 2.2 CRITICAL: Fake Encryption in Secrets

**File**: `data/src/secrets/mod.rs:273-290`

API keys stored with base64 encoding labeled as "obfuscation". This provides zero security.

**Fix**:
- Set file permissions to 0600 on Unix
- Add clear warning that file fallback is not secure
- Document that OS keyring is the primary (secure) storage

### 2.3 HIGH: Repository Trait Leaks Databento Abstractions

**File**: `data/src/repository/traits.rs:114-175`

`TradeRepository` trait has 5 methods with `_databento` in their names and uses `u16` schema discriminants instead of proper types.

**Fix**:
- Rename to provider-agnostic names (`check_cache_coverage`, `prefetch_to_cache`)
- Replace `u16` discriminant with proper enum or string identifier
- Unify progress callback signatures (currently `LoadingStatus` vs `(usize, usize)`)

### 2.4 HIGH: Composite Repository Stats Bug

**File**: `data/src/repository/composite.rs:137-149`

`stats()` aggregates hits/misses but never calls `update_hit_rate()` — always reports 0% hit rate.

**Fix**: Add `combined.hit_rate = combined.hits as f64 / (combined.hits + combined.misses) as f64;`

### 2.5 HIGH: FeedStatus Missing Serialization

**File**: `data/src/feed/types.rs:67-68`

`FeedStatus` lacks `Serialize`/`Deserialize` derives. While `DataFeed` correctly uses `#[serde(skip)]` for status, the type should still be serializable for logging/debugging.

**Fix**: Add `Serialize, Deserialize` derives to `FeedStatus`

### 2.6 MEDIUM: Feed Manager Allows Duplicate IDs

**File**: `data/src/feed/manager.rs:41-43`

`add()` pushes without checking for duplicate feed IDs.

**Fix**: Check `self.feeds.iter().any(|f| f.id == feed.id)` before push

### 2.7 MEDIUM: Feed Merger Panics on Empty Trades

**File**: `data/src/services/feed_merger.rs:136-137`

`build_candle_from_trades()` uses `assert!(!trades.is_empty())` — panics on empty input.

**Fix**: Return `Result` or skip empty buckets

### 2.8 MEDIUM: Hardcoded Dedup Tolerance

**File**: `data/src/services/feed_merger.rs:10-12`

`DEDUP_TOLERANCE_MS = 1` is hardcoded. Different feeds have different timing precision.

**Fix**: Make configurable via parameter

### 2.9 MEDIUM: AppState Tightly Coupled to DataFeedManager

**File**: `data/src/state/app_state.rs:160-161`

DataFeedManager directly embedded in AppState. Every feed config change requires state migration.

**Fix**: Consider separate persistence file for feed configs

### 2.10 LOW: PartialEq on DataFeed Ignores Status

**File**: `data/src/feed/types.rs:313-317`

Only compares `id`, which is intentional but should derive `Eq` and `Hash` explicitly.

### 2.11 LOW: Dead Comments in sidebar.rs

**File**: `data/src/config/sidebar.rs:8,18,38,47`

Comments about "removed tickers_table" are leftover refactoring artifacts.

---

## PART 3: EXCHANGE LAYER ISSUES

### 3.1 CRITICAL: RithmicError Not in Main Error Enum

**File**: `exchange/src/error.rs`

Main `Error` enum has `#[from] databento::Error` but no `#[from] RithmicError`. Cannot use `?` for Rithmic errors.

**Fix**: Add `#[error("Rithmic error: {0}")] Rithmic(#[from] crate::adapter::rithmic::RithmicError)`

### 3.2 CRITICAL: No Rithmic Trade Disk Cache

**File**: `exchange/src/repository/rithmic_trades.rs`

Historical trades loaded from Rithmic API are not cached to disk. Every query re-fetches.

**Fix**: Implement per-date disk caching like Databento's `.dbn.zst` pattern

### 3.3 HIGH: Client Dual-Plant Connection Race

**File**: `exchange/src/adapter/rithmic/client.rs:44-110`

If history plant connection fails, ticker plant connection remains open. No cleanup.

**Fix**: Disconnect ticker plant in history plant error handler

### 3.4 HIGH: RithmicDepthRepository is Empty Placeholder

**File**: `exchange/src/repository/rithmic_depth.rs:18-28`

Struct has no fields, all trait methods return `NotFound`. No path to store depth from streaming.

**Fix**: Add ability to accept and store depth snapshots from streaming layer

### 3.5 MEDIUM: Mapper Aggressor Assumptions

**File**: `exchange/src/adapter/rithmic/mapper.rs:28-32`

Assumes aggressor=1 is Buy, anything else is Sell. No handling for unknown values.

**Fix**: Handle all known values explicitly, log unknowns

### 3.6 MEDIUM: Trade Price Precision Loss

**File**: `exchange/src/adapter/rithmic/mapper.rs:25`

Converts Rithmic f64 price to f32, losing precision.

**Fix**: Keep as f64 until conversion to `Price` type

### 3.7 MEDIUM: Timestamp Microsecond Precision Loss

**File**: `exchange/src/adapter/rithmic/mapper.rs:18-20`

`usecs / 1000` truncates microseconds when converting to milliseconds.

**Fix**: Use proper rounding or higher precision timestamps

### 3.8 MEDIUM: OrderBook zip() Length Mismatch

**File**: `exchange/src/adapter/rithmic/mapper.rs:122-135`

`zip()` silently truncates if price and size arrays differ in length.

**Fix**: Validate lengths match, log warning if not

### 3.9 MEDIUM: has_trades() Always Returns True

**File**: `exchange/src/repository/rithmic_trades.rs:83-91`

Returns `Ok(true)` regardless of connection state or data availability.

**Fix**: Check connection state, return appropriate result

### 3.10 MEDIUM: find_gaps() Always Returns Empty

**File**: `exchange/src/repository/rithmic_trades.rs:115-122`

Returns empty vec — tells caller "no gaps" even when data doesn't exist.

**Fix**: Implement proper gap detection or return error indicating unsupported

### 3.11 MEDIUM: Streaming Silently Ignores Message Types

**File**: `exchange/src/adapter/rithmic/streaming.rs:106-108`

`_ => {}` silently drops unhandled Rithmic message types.

**Fix**: Log at warn level what messages are being ignored

### 3.12 MEDIUM: No Config Validation Before Connect

**File**: `exchange/src/adapter/rithmic/mod.rs:109-150`

`from_feed_config()` doesn't validate empty user_id, password, or system_name.

**Fix**: Validate required fields, return `RithmicError::Config` on empty

### 3.13 LOW: Status Channel Send Failures Silenced

**File**: `exchange/src/adapter/rithmic/client.rs:50,63`

`let _ = self.status_tx.send(...)` silently ignores send failures.

**Fix**: Log warnings on send failure

### 3.14 LOW: Incomplete Mapper Test Coverage

**File**: `exchange/src/adapter/rithmic/mapper.rs:157-192`

Only 2 tests. Missing: sell side, missing fields, BBO mapping, OrderBook mapping, timestamp precision.

---

## PART 4: UI/APP LAYER ISSUES

### 4.1 CRITICAL: Only One Rithmic Connection at a Time

**File**: `src/app/mod.rs:78-85`

App stores single `rithmic_client`, `rithmic_feed_id`. Cannot support multiple simultaneous Rithmic connections.

**Fix**: Use `HashMap<FeedId, RithmicConnection>` for multiple connections

### 4.2 CRITICAL: Feed Changes Not Persisted to Disk

**File**: `src/app/update.rs:670-671`

`Action::FeedsUpdated` logs but never saves to disk. Feed modifications lost on restart.

**Fix**: Call `save_state()` after feed CRUD operations

### 4.3 HIGH: Rithmic Status Channel Never Consumed

**File**: `src/app/services.rs:153-185`

`RithmicServiceResult` includes `status_rx` channel but it's never read. Status updates from rithmic-rs library are lost.

**Fix**: Spawn task to consume status_rx and forward as Messages

### 4.4 HIGH: Subscription Drains Only One Event Per Cycle

**File**: `src/app/subscriptions.rs:28`

`events.into_iter().next()` processes only first event. Under load, events queue unboundedly.

**Fix**: Drain all events per cycle, batch process

### 4.5 HIGH: No Validation Before Rithmic Connect

**File**: `src/app/update.rs:564-626`

Checks password exists but doesn't validate system_name, user_id, or subscribed_tickers are non-empty.

**Fix**: Validate all required fields before attempting connection

### 4.6 HIGH: No Connection Timeout

**File**: `src/app/update.rs:586-612`

`initialize_rithmic_service()` awaited without timeout. UI shows "Connecting..." forever if server is down.

**Fix**: Wrap in `tokio::time::timeout(Duration::from_secs(30), ...)`

### 4.7 HIGH: No Confirmation Before Feed Deletion

**File**: `src/modal/pane/data_feeds.rs:200-203`

`RemoveFeed` immediately deletes without confirmation dialog. No undo.

**Fix**: Show confirm dialog before deletion

### 4.8 HIGH: Edit Form No Unsaved Changes Warning

**File**: `src/modal/pane/data_feeds.rs:243-244`

`CancelEdit` silently discards form changes.

**Fix**: Check for changes, show warning if modified

### 4.9 MEDIUM: ToggleEnabled Has No UI Button

**File**: `src/modal/pane/data_feeds.rs:252-256`

`ToggleEnabled` message exists but no button generates it. Dead code.

**Fix**: Add enable/disable toggle to feed card

### 4.10 MEDIUM: OpenDownloadUI is Dead End

**File**: `src/app/update.rs:673-683`

Shows toast "Use the calendar in Data Management" — doesn't actually do anything useful.

**Fix**: Actually open the Data Management panel or remove the button

### 4.11 MEDIUM: Subscribed Tickers Not Updated on Live Edit

**File**: `src/modal/pane/data_feeds.rs:349-355`

Editing a connected Rithmic feed's tickers saves config but doesn't re-subscribe.

**Fix**: Trigger re-subscription or reconnection after ticker list changes

### 4.12 MEDIUM: No Rate Limiting on Connect Clicks

Clicking "Connect" rapidly spawns multiple connection attempts.

**Fix**: Disable button while status is `Connecting`

### 4.13 MEDIUM: Auto-Reconnect on Startup Missing

**File**: `src/app/mod.rs`

If app restarts, previously connected feeds don't auto-reconnect.

**Fix**: On startup, check for feeds with `auto_reconnect=true` and trigger connect

### 4.14 MEDIUM: RithmicConnected Handler Too Complex

**File**: `src/app/update.rs:783-911`

130+ lines of deeply nested logic for a single message variant.

**Fix**: Extract to helper methods: `handle_rithmic_connected()`, `start_rithmic_streaming()`

### 4.15 MEDIUM: Magic Uuid::nil() Sentinel

**File**: `src/app/update.rs:218,303`

`Uuid::nil()` used to distinguish "global" from "pane-specific" operations.

**Fix**: Use `Option<Uuid>` or dedicated enum

### 4.16 LOW: Audio Depth Streams TODO

**File**: `src/app/mod.rs:696-697`

`depth_streams_list = vec![]` with TODO comment. Audio depth preview non-functional.

### 4.17 LOW: Status Polling O(n*m) Complexity

**File**: `src/app/update.rs:159-181`

`UpdateLoadingStatus` iterates all layouts x all panes every 500ms.

**Fix**: Use indexed lookup or event-driven updates

### 4.18 LOW: Download Progress Subscription Emits Dummy Messages

**File**: `src/app/subscriptions.rs:56-68`

Emits `Uuid::nil()` dummy messages then filters them out. Wasteful.

**Fix**: Only emit when there's actual progress

---

## PART 5: PRIORITIZED IMPLEMENTATION PLAN

### Phase 1: Fix Critical Bugs & Safety (Estimated: 2-3 days)

| # | Task | Files | Impact |
|---|------|-------|--------|
| 1 | Fix mutex poisoning (unwrap → unwrap_or_else) | data/services/market_data.rs | Prevents crash cascade |
| 2 | Add RithmicError to main Error enum | exchange/error.rs | Enables error propagation |
| 3 | Fix subscription to drain ALL events per cycle | src/app/subscriptions.rs | Prevents event bottleneck |
| 4 | Add feed persistence on CRUD operations | src/app/update.rs | Prevents data loss on restart |
| 5 | Fix composite repo stats hit_rate | data/repository/composite.rs | Correct metrics |
| 6 | Add duplicate feed ID prevention | data/feed/manager.rs | Data integrity |
| 7 | Add config validation before Rithmic connect | src/app/update.rs, exchange/rithmic/mod.rs | Prevents silent failures |
| 8 | Add connection timeout (30s) | src/app/update.rs | Prevents indefinite hang |
| 9 | Fix client dual-plant connection cleanup | exchange/rithmic/client.rs | Prevents resource leak |

### Phase 2: Connections UX — Sidebar Menu + Manage Dialog (Estimated: 3-4 days)

| # | Task | Files | Impact |
|---|------|-------|--------|
| 10 | Add `Menu::Connections` to sidebar config | data/config/sidebar.rs | Foundation for new UX |
| 11 | Create connections quick-menu component | src/modal/pane/connections_menu.rs (new) | Sidebar popover |
| 12 | Add connections button to sidebar | src/screen/dashboard/sidebar.rs | Entry point |
| 13 | Wire connections menu into app view | src/app/mod.rs (view_with_modal) | Display popover |
| 14 | Redesign DataFeedsModal as split-pane | src/modal/pane/data_feeds.rs | List-detail layout |
| 15 | Add "Manage" button to connections menu | src/modal/pane/connections_menu.rs | Links popover → dialog |
| 16 | Add confirm dialog for feed deletion | src/modal/pane/data_feeds.rs | Safety |
| 17 | Add unsaved changes warning on cancel | src/modal/pane/data_feeds.rs | Safety |
| 18 | Add enable/disable toggle to feed cards | src/modal/pane/data_feeds.rs | Missing feature |
| 19 | Disable connect button while connecting | src/modal/pane/data_feeds.rs | Rate limiting |

### Phase 3: Rithmic Streaming & Event Routing (Estimated: 3-4 days)

| # | Task | Files | Impact |
|---|------|-------|--------|
| 20 | Route stream events to chart panes | src/app/update.rs | Live data in charts |
| 21 | Support multiple Rithmic connections | src/app/mod.rs, update.rs | Multi-connection |
| 22 | Consume Rithmic status channel | src/app/update.rs, subscriptions.rs | Status updates |
| 23 | Handle ticker re-subscription on edit | src/app/update.rs | Config hot-reload |
| 24 | Add auto-reconnect on startup | src/app/mod.rs | Session continuity |
| 25 | Fix mapper precision (f64, timestamps) | exchange/rithmic/mapper.rs | Data quality |
| 26 | Add explicit aggressor value handling | exchange/rithmic/mapper.rs | Robustness |
| 27 | Log ignored Rithmic message types | exchange/rithmic/streaming.rs | Observability |

### Phase 4: Repository & Data Architecture (Estimated: 2-3 days)

| # | Task | Files | Impact |
|---|------|-------|--------|
| 28 | Remove Databento-specific method names from trait | data/repository/traits.rs | Clean abstraction |
| 29 | Unify progress callback signatures | data/repository/traits.rs | API consistency |
| 30 | Implement Rithmic trade disk cache | exchange/repository/rithmic_trades.rs | Performance |
| 31 | Implement RithmicDepthRepository storage | exchange/repository/rithmic_depth.rs | Depth data |
| 32 | Fix has_trades() / find_gaps() stubs | exchange/repository/rithmic_trades.rs | Correctness |
| 33 | Implement real-time + historical merge | data/services/feed_merger.rs | Seamless data |
| 34 | Make dedup tolerance configurable | data/services/feed_merger.rs | Flexibility |
| 35 | Fix panic in build_candle_from_trades | data/services/feed_merger.rs, data/domain/aggregation.rs | Robustness |

### Phase 5: Polish & Code Quality (Estimated: 1-2 days)

| # | Task | Files | Impact |
|---|------|-------|--------|
| 36 | Extract RithmicConnected handler to methods | src/app/update.rs | Readability |
| 37 | Replace Uuid::nil() sentinels with Option | src/app/update.rs | Type safety |
| 38 | Set file permissions on secrets fallback | data/secrets/mod.rs | Security |
| 39 | Add FeedStatus serialization derives | data/feed/types.rs | Debugging |
| 40 | Fix download progress dummy messages | src/app/subscriptions.rs | Efficiency |
| 41 | Add mapper test coverage | exchange/rithmic/mapper.rs | Quality |
| 42 | Clean up dead comments in sidebar.rs | data/config/sidebar.rs | Cleanliness |
| 43 | Remove/fix OpenDownloadUI dead end | src/app/update.rs, data_feeds.rs | UX |

---

## PART 6: FILE CHANGE SUMMARY

| File | Phase | Changes |
|------|-------|---------|
| `data/src/config/sidebar.rs` | 2,5 | Add Connections menu variant, clean comments |
| `data/src/feed/manager.rs` | 1 | Duplicate ID prevention |
| `data/src/feed/types.rs` | 5 | Add Serialize to FeedStatus, fix PartialEq |
| `data/src/repository/traits.rs` | 4 | Remove databento names, unify callbacks |
| `data/src/repository/composite.rs` | 1 | Fix stats hit_rate |
| `data/src/services/market_data.rs` | 1 | Fix mutex poisoning |
| `data/src/services/feed_merger.rs` | 4 | Configurable dedup, fix panic, merge logic |
| `data/src/domain/aggregation.rs` | 4 | Fix panic in build_candle_from_trades |
| `data/src/secrets/mod.rs` | 5 | File permissions, security warning |
| `exchange/src/error.rs` | 1 | Add RithmicError variant |
| `exchange/src/adapter/rithmic/mod.rs` | 1 | Config validation |
| `exchange/src/adapter/rithmic/client.rs` | 1 | Dual-plant cleanup |
| `exchange/src/adapter/rithmic/mapper.rs` | 3 | Precision, aggressor handling |
| `exchange/src/adapter/rithmic/streaming.rs` | 3 | Log ignored messages |
| `exchange/src/repository/rithmic_trades.rs` | 4 | Disk cache, fix stubs |
| `exchange/src/repository/rithmic_depth.rs` | 4 | Implement storage |
| `src/app/mod.rs` | 2,3 | Multi-connection support, connections menu view |
| `src/app/update.rs` | 1,3,5 | Event routing, persistence, validation, refactor |
| `src/app/subscriptions.rs` | 1,5 | Fix event drain, dummy messages |
| `src/app/services.rs` | 3 | Status channel consumption |
| `src/screen/dashboard/sidebar.rs` | 2 | Connections button |
| `src/modal/pane/connections_menu.rs` | 2 | NEW — sidebar connections popover |
| `src/modal/pane/data_feeds.rs` | 2 | Split-pane redesign, deletion confirm, toggles |
| `src/modal/pane/mod.rs` | 2 | Export connections_menu |

---

## Verification Checklist

After all phases:
- [ ] `cargo build` — zero errors
- [ ] `cargo clippy --workspace` — zero warnings in modified files
- [ ] `cargo test --workspace` — all unit tests pass
- [ ] Manual: Sidebar connections menu shows live feeds first, up to 5
- [ ] Manual: Manage dialog shows split-pane list + edit form
- [ ] Manual: Add/edit/delete feeds with persistence across restart
- [ ] Manual: Rithmic connect → streaming data appears in charts
- [ ] Manual: Rithmic disconnect → clean shutdown, status updates
- [ ] Manual: Multiple Rithmic connections simultaneously
- [ ] Manual: Connection timeout shows error after 30s
- [ ] Manual: Auto-reconnect on connection loss
- [ ] Manual: No panic on empty data or invalid config
