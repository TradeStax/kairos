# Error Handling & Robustness Audit — Kairos

**Scope:** All workspace crates — `src/` (kairos), `data/` (kairos-data), `exchange/` (kairos-exchange), `study/` (kairos-study)
**Date:** 2026-02-20

---

## Executive Summary — Top 5 Crash Risks

1. **[CRITICAL] `active_dashboard()` and `active_dashboard_mut()` will panic if no layout is loaded** (`src/app/state.rs:17,21,28,32`). These are called on every UI tick and every message handler. If the layout manager ever finds itself with zero layouts (corrupt state file, migration bug), the app crashes immediately on launch or during any interaction.

2. **[CRITICAL] `layouts.first().unwrap()` as fallback for missing active layout** (`src/app/update/download.rs:91,284`). If `active_layout_id()` returns `None` and `layouts` is empty (possible after a failed state load), this panics in download message handlers — a background task path that should never crash the application.

3. **[HIGH] `Price::add` and `Price::sub` operator overloads panic on overflow** (`exchange/src/util.rs:277,316`). The `+` and `-` operators on `Price` call `checked_add` / `checked_sub` then `.expect(...)`. Any arithmetic on extreme price values (unlikely but possible with malformed exchange data) will abort the process rather than returning an error. These are in hot rendering paths.

4. **[HIGH] `build_candle_from_trades` asserts non-empty trades** (`data/src/domain/aggregation.rs:126`). This private function is called from `aggregate_trades_to_candles` only after partitioning non-empty buckets — structurally safe now — but the `assert!` converts a logic invariant into a production crash site if any caller is ever changed.

5. **[HIGH] `FuturesTicker::as_str()`, `product()`, `display_name()` unwrap UTF-8 conversion** (`data/src/domain/futures.rs:277,282,292`). These call `std::str::from_utf8(...).unwrap()` on raw bytes that come from exchange data deserialization. Any non-UTF-8 byte sequence (network corruption, encoding mismatch) panics in the data layer rather than returning an error.

---

## 1. Panic Path Inventory

### 1.1 `src/` — Application Layer

| File | Line(s) | Call | Risk |
|------|---------|------|------|
| `src/app/state.rs` | 17, 21, 28, 32 | `.expect("No active layout")` / `.expect("No active dashboard")` | **Critical** — called on every UI update |
| `src/app/mod.rs` | 323 | `.expect("No layouts available")` | **Critical** — during `new()`, panics at startup if state is empty |
| `src/app/update/download.rs` | 91, 284 | `layouts.first().unwrap().id.unique` | **High** — panics if layout list is empty in background task |
| `src/app/update/download.rs` | 138, 220, 387 | `get_download_progress().lock().unwrap()` | **Medium** — panics if mutex is poisoned (another thread panicked inside a lock) |
| `src/app/update/download.rs` | 241, 248 | `.unwrap()` (on download results) | **Medium** — needs context (see notes below) |
| `src/app/update/download.rs` | 255, 481, 486 | `downloaded_tickers.lock().unwrap()` | **Medium** — poisoned mutex |
| `src/app/update/feeds.rs` | 105 | `.unwrap()` | **High** — on `downloaded_tickers.lock()`, no poison recovery |
| `src/app/update/feeds.rs` | 162 | `feed_manager.get(feed_id).unwrap()` | **High** — called immediately after checking the feed exists, but race if the manager is mutated between the guard and this access |
| `src/app/update/feeds.rs` | 165 | `_ => unreachable!()` | **Medium** — assumes `connect_rithmic_feed` is only called when `FeedConfig::Rithmic` variant; violated by any future caller |
| `src/app/update/navigation.rs` | 31 | `layouts.last_mut().unwrap().id.name` | **Low** — only called after `insert_layout`, structurally safe |
| `src/chart/overlay/ruler.rs` | 123, 126 | `.unwrap()` on `partial_cmp` | **Low** — on f32 comparison; panics only on NaN, which is theoretically possible in malformed price data |
| `src/chart/study/poc.rs` | 80 | `find_poc(&footprint).unwrap()` | **High** — if footprint is somehow empty (zero-trade candle), panics in rendering path |
| `src/chart/study/volume_profile.rs` | 176, 180 | `.get(&price_key(...)).unwrap()` | **Low** — in tests only |
| `src/components/layout/multi_split.rs` | 49 | `assert!(panels.len() >= 2)` | **Medium** — public constructor, crashes if UI code passes empty vector |
| `src/modals/data_feeds/preview.rs` | 110 | `self.points.last().unwrap()` | **Medium** — panics if `points` is empty; should be guarded by `if self.points.is_empty()` before rendering |
| `src/modals/pane/calendar.rs` | 48,62,67,96,98,100 | `NaiveDate::from_ymd_opt(...).unwrap()` | **Low** — hardcoded valid date constants; `month + 1` overflow at December handled by the `if month.month() == 12` guard |
| `src/modals/pane/settings/kline.rs` | 172 | `footprint.unwrap()` | **High** — called in view function after checking `chart_type.is_footprint()` but the `footprint` variable may be `None` if data is loading |
| `src/modals/replay/mod.rs` | 656,669,997,1006,1007 | `NaiveDate::from_ymd_opt(...).unwrap()` | **Low** — mostly hardcoded or derived from month arithmetic with guards |
| `src/layout.rs` | 75 | `downloaded_tickers.lock().unwrap()` | **Medium** — unrecovered mutex poison during state serialization |
| `src/screen/dashboard/loading/chart_loading.rs` | 33, 156 | `.unwrap()` | **Medium** — on `downloaded_tickers.lock()` |
| `src/screen/dashboard/loading/feed_management.rs` | 53 | `.unwrap()` | **Medium** — same pattern |
| `src/screen/dashboard/panel/timeandsales.rs` | 195 | `pop_front().unwrap()` | **Low** — called only after `front()` check in while condition; logically safe |
| `src/screen/dashboard/update.rs` | 61 | `focus_pane.unwrap()` | **Medium** — needs context on whether `focus_pane` is always `Some` here |

### 1.2 `data/` — Data Layer

| File | Line(s) | Call | Risk |
|------|---------|------|------|
| `data/src/domain/aggregation.rs` | 126 | `assert!(!trades.is_empty())` | **High** — production assert; should be debug_assert |
| `data/src/domain/aggregation.rs` | 136,144,148 | `.unwrap()` on `max()`, `min()`, `last()` | **High** — safe only because `build_candle_from_trades` asserts non-empty, but depends on that assertion |
| `data/src/domain/aggregation.rs` | 276 | `assert!(!candles.is_empty())` in `aggregate_candle_bucket` | **High** — same pattern for candle aggregation |
| `data/src/domain/aggregation.rs` | 282,285,288 | `.unwrap()` on `max()`, `min()`, `last()` | **High** — dependent on assert above |
| `data/src/domain/futures.rs` | 277, 282, 292 | `std::str::from_utf8(...).unwrap()` | **High** — panics on any non-UTF-8 byte in exchange-sourced data |
| `data/src/domain/futures.rs` | 358 | `self.display_name().unwrap()` | **High** — in `Display` impl, panics if `has_display_name` is true but bytes are invalid UTF-8 |
| `data/src/domain/types.rs` | 297, 306 | `.unwrap()` on `and_hms_opt(0,0,0)` | **Low** — hardcoded valid time (midnight), safe |
| `data/src/domain/types.rs` | 215 | `DateTime::from_timestamp(0, 0).unwrap()` | **Low** — epoch zero is always valid |
| `data/src/services/feed_merger.rs` | 88 | `result.last().unwrap()` | **Low** — called only after `result.push(trades[0])` and skipping first element, so `result` is always non-empty |

### 1.3 `exchange/` — Exchange Layer

| File | Line(s) | Call | Risk |
|------|---------|------|------|
| `exchange/src/util.rs` | 107 | `fmt_into(...).unwrap()` in `Price::to_string` | **Low** — writing to a `String` can only fail on OOM; panics if memory exhausted |
| `exchange/src/util.rs` | 124 | `.expect("Price::to_string unit overflow")` | **Medium** — panics if `precision.power` causes `PRICE_SCALE + power > 18` (i64 overflow for `10^exp`) |
| `exchange/src/util.rs` | 237 | `.expect("min_tick_units overflowed")` | **Medium** — panics on extreme tick sizes; could occur with unusual instruments |
| `exchange/src/util.rs` | 255 | `.expect("add_steps overflowed")` | **High** — panics during price arithmetic; in rendering hot paths |
| `exchange/src/util.rs` | 277 | `.expect("Price add overflowed")` | **High** — `Price + Price` panics on overflow; used by operator overload in rendering/calculation paths |
| `exchange/src/util.rs` | 316 | `.expect("Price sub overflowed")` | **High** — `Price - Price` panics on overflow |
| `exchange/src/adapter/databento/fetcher.rs` | 1316, 1317 | `.expect("aggregate_group: empty bars")` | **Low** — only called after non-empty check but .expect is redundant |

### 1.4 `study/` — Study Layer

All `panic!()` and `.unwrap()` in `study/` crate are **exclusively inside `#[cfg(test)]` modules** (confirmed by reviewing the surrounding context). The `panic!("expected Lines output")`, `panic!("expected Markers, got...")` etc. are test assertions validating the correct `StudyOutput` variant is produced. These are justified test-only panics and pose **no production risk**.

The non-test `.unwrap()` calls in study (e.g., `study/src/momentum/macd.rs:124`) are all in test helper functions inside `mod tests {}` blocks.

---

## 2. Error Type Design

### Strengths

- Three well-defined error hierarchies: `data::domain::error::AppError` (trait), `exchange::error::Error` (concrete impl), `src::error::InternalError` (UI layer).
- `AppError` trait mandates `user_message()`, `is_retriable()`, `severity()` — this is an excellent pattern.
- `exchange::error::Error` properly wraps `databento::Error`, `databento::dbn::Error`, `io::Error`, `RithmicError` with `#[from]`.
- `AdapterError` in `exchange/src/adapter/error.rs` covers the four main adapter failure modes.

### Weaknesses

**[MEDIUM] `InternalError` is an overly coarse 3-variant enum** (`src/error.rs`):
- `Chart(String)`, `Data(String)`, `Rendering(String)` all hold a free-form string.
- String-wrapped errors lose structured information for logging, metrics, and programmatic recovery.
- There is no `is_retriable()` or `severity()` on `InternalError` — the UI error type doesn't implement `AppError`.

**[MEDIUM] Dual error types for similar concerns**: `AdapterError` (in `exchange/src/adapter/error.rs`) and `exchange::error::Error` (in `exchange/src/error.rs`) overlap substantially. `AdapterError` is a subset of `exchange::error::Error`. There is no clear conversion between them — they coexist in the same crate for different subsystems, creating inconsistency.

**[LOW] Catch-all strings hide real errors**: `Error::Fetch(String)`, `Error::Parse(String)`, `Error::Cache(String)` all use `String` rather than wrapping the original error type. `from_str(...)` and `.to_string()` conversions discard the original `std::io::Error`, HTTP error codes, and parse context. The `#[from] std::io::Error` variant exists alongside `Cache(String)` for the same I/O scenario.

**[LOW] `AdapterError::FetchError(#[from] reqwest::Error)` leaks HTTP internals** in `user_message()` indirectly — the display string for `reqwest::Error` can include internal URLs and headers.

---

## 3. Error Propagation

### 3.1 Errors Properly Propagated

- All `exchange` async functions return `Result<T, exchange::error::Error>` or `Result<T, AdapterError>` and are properly awaited with `?` propagation.
- Rithmic client connection uses `?` throughout and performs cleanup on partial failure (ticker plant cleaned up when history plant fails).
- Download pipeline converts errors to `String` with `.map_err(|e| e.to_string())` before crossing the `Task::perform` boundary, which is necessary because closures must be `'static`.

### 3.2 Silently Swallowed Errors

**[HIGH] `let _ = self.status_tx.send(...)` in Rithmic client** (`exchange/src/adapter/rithmic/client.rs:46,55,64,75,90,104,237`). Feed status updates are sent via unbounded channel but the result is discarded. If the receiver has been dropped (app closed while connecting), the error is silently ignored — this is likely intentional (best-effort status) but means no reconnect logic can observe the failure.

**[HIGH] `let _ = event_tx.try_send(...)` for replay events** (`data/src/services/replay_engine.rs:585,599,611,612`). Replay market data events are sent via bounded channel. If the channel is full (the consumer — the UI — is lagging), **data is silently dropped**. There is a log at line 621 for `emit_event`, but the inline `let _ =` calls at 585/599/611/612 have no logging whatsoever. Under heavy replay load, the UI may receive incomplete data without any indication.

**[MEDIUM] `.ok()` on `next_entry()` in cache directory scan** (`exchange/src/adapter/databento/cache.rs:218`): `entries.next_entry().await.ok().flatten()`. An I/O error reading a directory entry is silently treated as end-of-iteration. Corrupt or permission-denied cache entries are skipped without logging.

**[MEDIUM] `src/chart/candlestick/mod.rs:838` — `let _ = s.set_parameter(key, value)`**: Study parameter updates silently discard errors. If parameter validation fails, the study configuration is silently unchanged with no user notification.

**[MEDIUM] `src/modals/pane/indicator_manager.rs:252,286` — same pattern**: `let _ = snapshot.set_parameter(...)`. Same issue — indicator parameter updates that fail (type mismatch, out of range) are completely silent.

**[LOW] `data/src/state/persistence.rs:380` — `let _ = std::fs::remove_file(path)`**: Failed temp file cleanup is silently ignored. Can accumulate orphaned temp files.

**[LOW] `data/src/services/options_data.rs:364-366`** — `.stats().await.ok()` on repo stats: treats repository failures as "no stats", which is acceptable degradation.

### 3.3 Error Conversion Between Layers

**[MEDIUM] No conversion from `AdapterError` to `exchange::error::Error`**: The two error types in the `exchange` crate are not connected via `From` implementations. Code using `AdapterError` in adapters cannot easily propagate to callers expecting `exchange::error::Error`.

**[LOW] `InternalError::from(String)` converts all strings to `Data` variant**: Any string-based error from exchange layer arrives in the UI as `InternalError::Data(...)` regardless of whether it is a chart, data, or rendering issue.

---

## 4. Resource Cleanup

### 4.1 Good Practices

- Rithmic client performs explicit cleanup of the ticker plant handle when history plant connection fails (`client.rs:77-82`).
- Download progress is cleaned up in both success and failure paths in `handle_download_complete`.
- Temp file cleanup in `exchange/src/adapter/databento/fetcher.rs:1065` uses `let _ = tokio::fs::remove_file(...)` — acceptable since temp files in the OS temp directory are eventually cleaned by the OS.

### 4.2 Potential Resource Leaks

**[HIGH] Rithmic `_ticker_plant` and `_history_plant` drop behavior**: `RithmicClient` holds `Option<RithmicTickerPlant>` prefixed with `_` to suppress unused warnings. If `RithmicClient` is dropped without calling `disconnect()`, the destructor of the underlying plants will close TCP connections. Whether the Rithmic protocol expects a graceful disconnect message before TCP close is unclear — ungraceful disconnects may consume a Rithmic license seat until the server detects the dead connection.

**[MEDIUM] `RITHMIC_EVENTS` global mutex grows unboundedly**: The `Vec<exchange::Event>` in `get_rithmic_events()` is drained during processing but if the subscription loop stalls (e.g., UI thread busy), events accumulate without a size limit.

**[MEDIUM] `DOWNLOAD_PROGRESS` leak on panic**: If a download task panics after inserting into `DOWNLOAD_PROGRESS` but before the completion handler clears it, the progress entry remains permanently, incrementing counts erroneously.

**[LOW] Replay engine `playback_handle: Option<JoinHandle>` not awaited**: In `ReplayEngine`, the `tokio::JoinHandle` for background playback is stored as `Option`. On `stop()`, the handle is aborted but not awaited. If the task holds resources or sends a final event on drop, there is a race between the abort and the event channel.

---

## 5. Concurrency Safety

### 5.1 Mutex Usage

**[MEDIUM] Mixed poison recovery strategies**: Some call sites use `lock().unwrap()` (panics on poison), others use `lock().unwrap_or_else(|e| e.into_inner())` (recovers from poison). The inconsistency means a panic in one task can cause a second panic in a different task that encounters the poisoned mutex.

Sites using bare `lock().unwrap()` (no poison recovery):
- `src/app/update/download.rs:138` — download progress lock
- `src/app/update/download.rs:220` — download complete handler
- `src/app/update/download.rs:255` — ticker count display
- `src/app/update/download.rs:481,486` — historical download complete
- `src/layout.rs:75` — state serialization
- `src/screen/dashboard/loading/chart_loading.rs:33,156` — pane initialization
- `src/screen/dashboard/loading/feed_management.rs:53` — feed affiliation
- `src/app/update/feeds.rs:105` — ticker list rebuild

**[LOW] `downloaded_tickers` lock held across multiple operations**: In `handle_download_complete`, `downloaded_tickers.lock().unwrap()` is called three times sequentially (register, list_tickers, count) rather than once. This is not a deadlock but creates unnecessary lock contention.

### 5.2 Deadlock Analysis

**[LOW] No cross-mutex lock ordering**: The codebase holds at most one of `{downloaded_tickers, data_feed_manager, download_progress}` at a time in all observed call sites. No deadlock risk identified from nested locking.

**[MEDIUM] `tokio::spawn_blocking` holding `std::sync::Mutex`**: In `src/app/update/chart.rs:189-191`, a `spawn_blocking` closure captures and locks a `std::sync::Mutex` (`engine.lock()`), then calls `block_on`. If the blocking thread pool is exhausted and the Tokio runtime is waiting for a task that itself needs a blocking thread, this could stall. The `unwrap_or_else(|e| e.into_inner())` poison recovery is correct here.

### 5.3 OnceLock Globals

The three `OnceLock` globals (`DOWNLOAD_PROGRESS`, `RITHMIC_EVENTS`, `REPLAY_EVENTS`) are properly initialized via `get_or_init` and are `'static`. No races on initialization. The `Arc<Mutex<...>>` wrapping provides thread-safety for access. This pattern is acceptable for the Elm architecture workaround.

---

## 6. Edge Cases

### 6.1 Empty Collections

**[MEDIUM] `src/modals/data_feeds/preview.rs:110` — `points.last().unwrap()`**: The rendering function is called even when `self.points` is empty (zero historical data). Should guard: `if let Some(last) = self.points.last()`.

**[MEDIUM] `src/chart/study/poc.rs:80` — `find_poc(&footprint).unwrap()`**: Called in a rendering path without checking if the footprint is empty. An empty footprint (candle with trades filtered out entirely) returns `None` from `find_poc`, causing a panic.

### 6.2 Integer Arithmetic

**[HIGH] `Price + Price` and `Price - Price` panic on overflow** (see Section 1.3): These are `impl std::ops::Add/Sub for Price`, meaning any usage site in normal arithmetic expressions can abort. Consider returning `saturating_add`/`saturating_sub` or making these checked operations that return `Option<Price>`.

**[MEDIUM] `u64` timestamp arithmetic can overflow**: `Timestamp(u64)` operations like `abs_diff` in `feed_merger.rs` and millisecond calculations cannot overflow `u64` for any realistic timestamp, but the `timestamp_millis() as u64` cast in `domain/types.rs:222` silently wraps on negative timestamps (pre-epoch). The `from_timestamp` call is guarded by `unwrap_or(epoch_zero)`.

**[LOW] `(ms % 1000) * 1_000_000` for nanoseconds** (`data/src/domain/types.rs:214`): Maximum value is `999 * 1_000_000 = 999_000_000`, which fits in `u32`. No overflow risk.

### 6.3 Zero Division

**[LOW] `tick_size == 0` guard in `Price::round_to_step`** (`exchange/src/util.rs:181-183`): Correctly handles `unit <= 1` by returning `self` — no division by zero possible.

**[LOW] `steps_between_inclusive` guards `step.units <= 0`**: Properly returns `None`. Safe.

**[MEDIUM] `div_euclid(rhs)` in `Price::div` where `rhs: i64`**: No guard against `rhs == 0`. If a call site passes zero (e.g., deriving tick count from zero-sized range), this panics.

### 6.4 String / UTF-8 Handling

**[HIGH] `FuturesTicker::as_str()`, `product()`, `display_name()` — `from_utf8(...).unwrap()`** (`data/src/domain/futures.rs:277,282,292`): These methods are called in `Display` impls, serialization, and rendering. Non-UTF-8 bytes from the exchange (e.g., Databento returning a symbol with a non-ASCII character) will panic. Fix: use `from_utf8_lossy` or return `Result<&str, ...>`.

**[MEDIUM] `symbol.split('.').next().unwrap()`** (`src/app/mod.rs:1053`, `src/modals/pane/tickers.rs:117`): `split('.')` always returns at least one element for a non-empty string, but for an empty string `""`, `next()` returns `None` and `.unwrap()` panics. FUTURES_PRODUCTS symbols are hardcoded constants so this is currently safe, but fragile.

---

## 7. Graceful Degradation

### 7.1 Feed Disconnection

**[GOOD] Databento feed**: Disconnection is handled gracefully — panes are marked as `LoadingStatus::Error`, and reconnection triggers a full reload. No panic paths.

**[MEDIUM] Rithmic feed disconnection**: `exchange/src/adapter/rithmic/streaming.rs:90,95,108` sends `Event::ConnectionLost` via `let _ =`. The upper layer (`src/app/update/feeds.rs`) processes `RithmicConnected { result: Err(_) }` and shows a toast — but there is no retry logic or exponential backoff. After disconnection, the user must manually reconnect via UI.

**[LOW] No heartbeat / liveness check**: The Rithmic streaming loop processes messages but has no timeout for idle connections. A frozen TCP connection (no RST received) would appear connected to the application indefinitely.

### 7.2 Historical Data Missing

**[GOOD] Missing date range fallback**: In `chart_loading.rs`, `get_range()` returning `None` falls back to `DateRange::last_n_days(preset_days)` with a warning log. The app shows an empty chart rather than crashing.

### 7.3 State File Corruption

**[MEDIUM] `load_state` failure falls back to default**: In `data/src/state/persistence.rs`, if deserialization fails, the app starts with a default state. This is handled. However, if the state file is partially written (power loss during save), the JSON may be valid but semantically corrupt — no checksums or atomic writes are used.

**[MEDIUM] No atomic writes for state persistence**: `save_state` writes directly to `app-state.json` without first writing to a temp file and renaming. A crash during write corrupts the state file. Fix: write to `app-state.json.tmp`, then `rename`.

---

## Summary Table

| Finding | Severity | File | Recommended Fix |
|---------|----------|------|-----------------|
| `active_dashboard()` panics if no layout | Critical | `src/app/state.rs:17,28` | Return `Option<&Dashboard>`, propagate None upward |
| `expect("No layouts available")` in `new()` | Critical | `src/app/mod.rs:323` | Return `Result` or synthesize a default layout |
| `layouts.first().unwrap()` fallback | High | `src/app/update/download.rs:91,284` | Return `Task::none()` with error toast if no layouts |
| `Price + Price` panics on overflow | High | `exchange/src/util.rs:277,316` | Use `saturating_add/sub` or remove `Add/Sub` impl in favor of explicit checked functions |
| `from_utf8(...).unwrap()` on exchange symbols | High | `data/src/domain/futures.rs:277,282,292` | Replace with `from_utf8_lossy` or propagate as `Result` |
| `assert!` in production aggregation | High | `data/src/domain/aggregation.rs:126,276` | Change to `debug_assert!` + return `Err` at call site |
| `footprint.unwrap()` in view function | High | `src/modals/pane/settings/kline.rs:172` | Guard with `if let Some(fp) = footprint` |
| Replay events silently dropped on full channel | High | `data/src/services/replay_engine.rs:585,599` | Log dropped events; consider bounded-with-drop policy |
| `feed_manager.get(feed_id).unwrap()` | High | `src/app/update/feeds.rs:162` | Use `if let Some(feed) = ...` guard |
| `_ => unreachable!()` in `connect_rithmic_feed` | Medium | `src/app/update/feeds.rs:165` | Replace with explicit `return Task::none()` with log |
| `points.last().unwrap()` in preview render | Medium | `src/modals/data_feeds/preview.rs:110` | Guard `if self.points.is_empty() { return vec![]; }` |
| `find_poc().unwrap()` in rendering | Medium | `src/chart/study/poc.rs:80` | Guard with `if let Some(poc) = find_poc(...)` |
| `downloaded_tickers.lock().unwrap()` (bare) | Medium | Multiple sites in `src/app/` | Replace with `lock().unwrap_or_else(|e| e.into_inner())` |
| `RITHMIC_EVENTS` unbounded growth | Medium | `src/app/mod.rs:43` | Add capacity limit or drain before processing |
| Study parameter update errors silently ignored | Medium | `src/chart/candlestick/mod.rs:838` | Log errors from `set_parameter` |
| No atomic writes for state persistence | Medium | `data/src/state/persistence.rs` | Write to temp file then `fs::rename` |
| `InternalError` lacks `AppError` impl | Medium | `src/error.rs` | Implement `user_message()`, `is_retriable()`, `severity()` |
| `AdapterError` / `exchange::Error` not connected | Medium | `exchange/src/` | Add `From<AdapterError> for Error` |
| `Price::div` no zero guard | Low | `exchange/src/util.rs:285` | Add `assert_ne!(rhs, 0)` or return `Option<Price>` |
| Test panics in study/ are justified | None | `study/src/` | No action needed |
