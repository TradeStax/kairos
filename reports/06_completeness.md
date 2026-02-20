# Report 06: Implementation Completeness & Gaps Audit

**Date**: 2026-02-20
**Codebase**: Kairos — native futures charting desktop app (Rust + Iced v0.14)
**Workspace crates**: `src/` (kairos), `data/` (kairos-data), `exchange/` (kairos-exchange), `study/` (kairos-study), `script/` (kairos-script)

---

## Executive Summary — Top 10 Gaps by Impact

| # | Gap | Impact | Priority |
|---|-----|--------|----------|
| 1 | **Options data pipeline dead-end** — `OptionsMessage` enum is `#[allow(dead_code)]`, the `Options` variant in `Message` is annotated dead, handlers log & do nothing | Zero user-visible options functionality despite full exchange layer | P0 |
| 2 | **Link-group ticker switching is a stub** — `SwitchLinkGroup` handler sets the group field but has a TODO saying the actual data reload is not implemented | Linked panes do not synchronize on ticker change | P0 |
| 3 | **Replay skip (JumpForward/JumpBackward)** — both messages are `#[allow(dead_code)]` with comment "Planned"; there is no handler | Replay playback is missing navigation controls | P1 |
| 4 | **FocusWidget effect is silently dropped** — `Effect::FocusWidget(_id)` is received and explicitly `(Task::none(), None)` returned with a TODO | Modal opening keyboard-focus flow is broken | P1 |
| 5 | **`TickerInfo` / `FuturesTickerInfo` type duplication** — a `// TODO: Remove once TickerInfo is fully migrated to FuturesTickerInfo` compatibility shim exists in `comparison/mod.rs` plus `Ladder` and `TimeAndSales` still use the old `TickerInfo` type | Ongoing maintenance hazard; old type still leaks | P1 |
| 6 | **`src/` crate (kairos) has virtually zero tests** — 11 test functions across 4 files, all in chart sub-modules (`lod`, `study/*`). The entire UI, message routing, and pane lifecycle has no coverage | Any regression in app-level logic is invisible to CI | P1 |
| 7 | **Script (JS) indicators have no UI entry point** — the `kairos-script` crate and its `ScriptRegistry` are initialized at startup and merged into the unified registry, but there is no UI panel, menu item, or documentation showing users how to load/use custom `.js` indicators | Feature is silently operational but inaccessible to users | P1 |
| 8 | **All `panic!()` calls in `study/` tests are load-bearing** — 35+ `panic!` calls inside `#[test]` bodies are used to assert output variant type ("Expected Bars output", "expected Lines output"). A wrong output variant silently passes until tested | Test fragility; wrong output type panics in tests instead of failing gracefully | P2 |
| 9 | **Branching migration paths are unsupported** — `get_migration_path` has a `// TODO: support branching/parallel versions` comment; currently only linear v0→v1→v2 is possible | Future schema changes requiring non-linear migrations will break | P2 |
| 10 | **`src/chart/study/` and `src/chart/indicator/` modules are `#[allow(dead_code)]`** — the old indicator directory was deleted (git shows it as `D`), but references to `study` sub-modules survive via `#[allow(dead_code)]` on the whole module | Dead code accumulation obscures real usage | P2 |

---

## 1. TODO / FIXME / HACK Inventory

| File | Line | Comment | Severity |
|------|------|---------|----------|
| `data/src/state/persistence.rs` | 115 | `// TODO: support branching/parallel versions` | P2 |
| `data/src/repository/traits.rs` | 159 | `// TODO: Consider extracting to a separate extension trait when additional providers need similar capabilities` | P3 |
| `src/chart/comparison/mod.rs` | 512 | `// TODO: Remove once TickerInfo is fully migrated to FuturesTickerInfo` | P1 |
| `src/screen/dashboard/update.rs` | 166 | `// TODO: Handle ticker switching in link groups` | P0 |
| `src/screen/dashboard/update.rs` | 207 | `// TODO: Implement widget focusing with the specific ID` | P1 |
| `src/modals/replay/mod.rs` | 97 | `#[allow(dead_code)] // Planned: skip forward in replay` | P1 |
| `src/modals/replay/mod.rs` | 99 | `#[allow(dead_code)] // Planned: skip backward in replay` | P1 |
| `src/app/mod.rs` | 122 | `#[allow(dead_code)] // Options data pipeline not yet wired` | P0 |
| `src/app/mod.rs` | 191 | `#[allow(dead_code)] // Options data pipeline not yet wired` | P0 |

### Full `#[allow(dead_code)]` Inventory (production code, not tests)

The following production modules are suppressed from dead-code lint, indicating significant unused surface area:

| Location | Scope |
|----------|-------|
| `src/style/mod.rs:5` | Single item |
| `src/components/primitives/mod.rs:1` | Entire module (`#![allow]`) |
| `src/components/form/mod.rs:1` | Entire module |
| `src/components/overlay/mod.rs:1` | Entire module |
| `src/components/display/mod.rs:1` | Entire module |
| `src/components/input/mod.rs:1` | Entire module |
| `src/components/layout/mod.rs:1` | Entire module |
| `src/chart/mod.rs:14,16` | `study` and `study_renderer` sub-modules |
| `src/chart/core/mod.rs:12` | Core module |
| `src/chart/drawing/mod.rs:9` | Drawing module |
| `src/chart/perf/mod.rs:1` | Perf module |
| `src/modals/drawing_tools/mod.rs:23,152` | Drawing tools |
| `src/app/services.rs:117` | `MarketDataServiceResult` struct |
| `src/app/mod.rs:122,191` | Options message variants |
| `src/screen/dashboard/sidebar.rs:326` | Single field |
| `src/chart/study/value_area.rs:6,14` | Study items |

---

## 2. Placeholder & Stub Code

### 2.1 Options Pipeline — Complete Dead End

**Files**: `src/app/mod.rs`, `src/app/update/options.rs`

```rust
// src/app/mod.rs:122
#[derive(Debug, Clone)]
#[allow(dead_code)] // Options data pipeline not yet wired
pub enum OptionsMessage {
    OptionChainLoaded { ... }
    GexProfileLoaded { ... }
}

// src/app/mod.rs:191
#[allow(dead_code)] // Options data pipeline not yet wired
Options(OptionsMessage),
```

The handlers in `src/app/update/options.rs` exist and route correctly, but the `OptionsMessage` variants are never produced — there are no `Task::perform(...)` calls that yield these messages. The options service is initialized (if Massive API key is set) but is never called after startup. The entire UI flow for displaying option chains or GEX profiles is missing.

### 2.2 Link Group Ticker Switching Stub

**File**: `src/screen/dashboard/update.rs:162–169`

```rust
if let Some(state) = self.get_mut_pane(main_window.id, window, pane) {
    state.link_group = group;
    state.modal = None;

    // TODO: Handle ticker switching in link groups
    // For now, just set the link group without loading data
    // Will implement proper chart loading once pane.rs is refactored
}
```

The link group assignment works, but the intent — when a linked pane switches ticker, all panes sharing that link group should also switch — is not implemented.

### 2.3 FocusWidget Effect Dropped

**File**: `src/screen/dashboard/update.rs:206–209`

```rust
pane::Effect::FocusWidget(_id) => {
    // TODO: Implement widget focusing with the specific ID
    // For now, this effect is not critical for core functionality
    (Task::none(), None)
}
```

The effect is produced in `src/screen/dashboard/pane/update.rs:467–473` to focus the search field when a modal opens, but it is silently discarded.

### 2.4 Replay JumpForward / JumpBackward

**File**: `src/modals/replay/mod.rs:97–100`

```rust
#[allow(dead_code)] // Planned: skip forward in replay
JumpForward,
#[allow(dead_code)] // Planned: skip backward in replay
JumpBackward,
```

The replay modal has no UI buttons wired to these messages, and no handler processes them in `src/app/update/replay.rs`.

---

## 3. Feature Completeness Matrix

| Feature | Status | Notes |
|---------|--------|-------|
| Candlestick / Footprint chart | **Complete** | Full OHLCV + footprint rendering |
| Heatmap chart | **Complete** | Order flow depth heatmap |
| Comparison chart | **Partial** | Working; `TickerInfo` ↔ `FuturesTickerInfo` shim pending removal |
| Options data display | **Stub** | Service exists; UI pipeline not wired |
| GEX profile display | **Stub** | Service exists; UI pipeline not wired |
| Replay playback | **Partial** | Play/Pause/Seek work; JumpForward/JumpBackward not implemented |
| Link group synchronization | **Partial** | Group assignment works; ticker propagation not implemented |
| Drawing tools | **Complete** | 14 tools with persistence and property editor |
| Study/indicator system | **Complete** | 15 native studies + JS scripting engine |
| Custom JS indicators | **Partial** | Engine and registry work; no UI for management/hot-reload |
| Theme editor | **Complete** | HSV color editing, custom themes |
| Layout manager | **Complete** | Multiple layouts with save/load |
| Rithmic real-time feed | **Complete** | WebSocket streaming |
| Databento historical feed | **Complete** | .dbn.zst caching |
| Massive / Polygon options | **Partial** | Repository layer complete; not wired to UI |
| Widget focus on modal open | **Stub** | `FocusWidget` effect silently dropped |
| Multi-window popouts | **Complete** | Full OS window support |
| Migration system | **Partial** | Linear-only; branching TODO present |
| Secrets / keyring | **Complete** | OS keyring + file + env var fallback |
| LOD rendering | **Complete** | Level-of-detail for chart perf |

---

## 4. Test Coverage Assessment

### 4.1 Per-Crate Summary

| Crate | Test Count (approx.) | Coverage Areas | Critical Gaps |
|-------|---------------------|----------------|---------------|
| `kairos` (src/) | ~11 functions, 4 files | `lod`, `study/{volume_profile, poc, imbalance}` | UI/message routing, pane lifecycle, chart update logic, services — **entirely untested** |
| `kairos-data` | ~93 functions, 25 files | Domain types, aggregation, persistence, state, services | MarketDataService integration paths |
| `kairos-exchange` | ~42 functions, 14 files | Massive adapter decoding, Databento caching, error types | Rithmic mapper integration |
| `kairos-study` | ~104 functions, 19 files | All 15 studies with compute + incremental tests | Study-to-renderer integration |
| `kairos-script` | ~6 functions, 1 file (ta.rs) | SMA, EMA, RMA helper functions | Bridge, compiler, loader, manifest parsing |

### 4.2 Critical Zero-Coverage Paths in `kairos` (src/)

- `src/app/update/` — all 8 handler files (chart, download, feeds, navigation, options, preferences, replay, mod)
- `src/app/mod.rs` — Iced `update()` dispatch
- `src/screen/dashboard/` — pane lifecycle, pane content switching, pane grid operations
- `src/chart/candlestick/` — KlineChart compute and render
- `src/chart/heatmap/` — HeatmapChart render
- `src/chart/comparison/` — ComparisonChart data loading and series management
- `src/modals/` — all modal update logic
- `src/window.rs` — multi-window management

### 4.3 Test Fragility in `study/`

35+ `panic!()` calls in test functions use pattern:

```rust
_ => panic!("Expected Bars output"),
```

This is an assertion pattern that panics on wrong output type — tests will "fail" as panics rather than test failures. While functionally equivalent in practice, it bypasses standard test reporting and makes CI output harder to read.

### 4.4 Files with `#[cfg(test)]` Blocks but Substantive Logic Gaps

`data/src/services/market_data.rs` — has `#[cfg(test)]` but only 1 test function (`test_get_cache_stats`). The primary `get_chart_data` and `rebuild_chart_data` paths are not tested.

`data/src/services/options_data.rs` — has `#[cfg(test)]` but only 1 mock-based test. No integration tests for chain/contract queries.

---

## 5. Missing Error Handling

### 5.1 Silently Ignored Results

In `src/screen/dashboard/pane/lifecycle.rs:68`:
```rust
let _ = s.set_parameter(key, pv);
```

In `src/chart/candlestick/mod.rs:838`:
```rust
let _ = s.set_parameter(key, value);
```

In `src/modals/pane/indicator_manager.rs:252` and `286`:
```rust
let _ = snapshot.set_parameter(&key, value.clone());
let _ = snapshot.set_parameter(key, value.clone());
```

`set_parameter` returns `Result<(), _>` — errors are silently swarded with `let _`. If a parameter key is mistyped or the value is invalid, the UI will appear to accept the change without actually applying it.

### 5.2 Unwraps in Production Paths (non-test)

**`src/` crate**: 52 total `.unwrap()` / `.expect()` occurrences across 22 files:
- `src/modals/pane/calendar.rs` — 7 occurrences (date parsing)
- `src/app/update/download.rs` — 10 occurrences
- `src/app/state.rs` — 5 occurrences
- `src/modals/replay/mod.rs` — 5 occurrences

**`data/` crate**: 95 total across 19 files:
- `data/src/services/gex_calculator.rs` — 11 occurrences
- `data/src/domain/aggregation.rs` — 12 occurrences
- `data/src/services/replay_engine.rs` — 8 occurrences

Many are within `#[cfg(test)]` blocks (acceptable), but several are in production service code.

---

## 6. Configuration Gaps

### 6.1 Settings in Types but Not Exposed in UI

**`HeatmapConfig.rendering_mode`** — The `TradeRenderingMode::Auto` variant resolves at render time but there is a `// Already resolved above` comment with `unreachable!()` at `src/chart/heatmap/render.rs:502`. The Auto mode is not surfaced as a distinct option in the settings UI.

**`KlineConfig.candle_style`** (bull/bear colors) — Defined in `data/src/state/pane.rs`, but the kline settings modal (`src/modals/pane/settings/kline.rs`) may not expose all color customization fields.

### 6.2 Feature Flags Declared but Not Exposed

The workspace `Cargo.toml` defines a single `debug` feature:
```toml
[features]
debug = ["iced/hot"]
```

Iced's hot-reload feature is gated but there is no documentation or tooling around its use.

### 6.3 `ScaleFactor` Configuration

`ScaleFactor` is persisted in `AppState` but there is no visible UI slider or text input to change DPI scaling factor at runtime (other than theme editor if it exposes it).

---

## 7. Documentation Gaps

### 7.1 README Files

| File | Status |
|------|--------|
| `README.md` (root) | Present but **empty** — contains only an SVG image tag (`<img .../>`) with no text content |
| `data/README.md` | **Comprehensive** — full architecture, API, testing, and dependency docs |
| `exchange/README.md` | Present but contents not audited |
| `study/README.md` | Not audited |
| `script/` | **No README** — the JS scripting system for custom indicators has no user documentation |

The root `README.md` being empty is a significant gap for onboarding.

### 7.2 Missing Doc Comments on Public APIs

The `kairos-script` crate (`script/`) lacks documentation for:
- The JavaScript indicator API surface (what globals are available, how `plot()` works)
- How to create a custom `.js` indicator
- Where to place user scripts
- Expected manifest format

Without this, the scripting feature is inaccessible to users even if the engine works.

### 7.3 `CLAUDE.md` Accuracy

`CLAUDE.md` lists `src/chart/indicator/` as an active module with sub-directories (`kline/`, `plot/`). Both directories are **deleted** per the git status:
```
D src/chart/indicator/kline/bollinger.rs
D src/chart/indicator/kline/delta.rs
... (9 files deleted)
D src/chart/indicator/mod.rs
D src/chart/indicator/plot/bar.rs
D src/chart/indicator/plot/line.rs
D src/chart/indicator/plot/mod.rs
```

---

## 8. Git Status Analysis

### 8.1 Deleted Files Still Referenced

The `src/chart/indicator/` directory (14 files) is deleted in working tree. Based on code search, the old `chart::indicator` path is no longer imported anywhere — the refactoring successfully moved indicators to `study/` crate and `src/chart/study/`. No dangling imports were found pointing to the deleted paths.

However, `CLAUDE.md` still documents the deleted module path as active.

### 8.2 New Untracked Files

The following untracked files should be reviewed for inclusion in the next commit:

| File | Purpose | Commit? |
|------|---------|---------|
| `src/modals/download/api_key_modal.rs` | API key setup UI | Yes |
| `src/modals/pane/indicator_manager.rs` | New indicator manager modal | Yes |
| `src/modals/pane/settings/big_trades_debug.rs` | Debug modal for big trades | Yes |
| `README.md` | Root README (currently near-empty) | After content is added |
| `CODEBASE_REVIEW_PROMPT.md` | Internal review prompt | No (internal tooling) |
| `assets/icons/` | UI icon assets | Yes |
| `assets/illustrations/` | SVG illustrations | Yes |
| `assets/scripts/` | 17 bundled JS indicator scripts | Yes |
| `build.rs` | Build script | Yes |
| `script/` | JS scripting engine crate | Yes |

### 8.3 Pattern of Ongoing Refactoring

The git diff reveals a systematic refactoring in progress:

1. **Indicator system migrated**: Old `src/chart/indicator/` (kline-specific indicator rendering) has been deleted. Studies now live in `study/` crate with a unified registry. The new `IndicatorManagerModal` (`indicator_manager.rs`) is the replacement UI.

2. **`TickerInfo` → `FuturesTickerInfo` migration**: Exchange layer transitioned to `FuturesTickerInfo`, but `Ladder`, `TimeAndSales`, and `ComparisonChart` internal types (`Series.ticker_info: TickerInfo`) still use the old type. A compatibility shim exists in `comparison/mod.rs`.

3. **Modals reorganization**: `src/modals/pane/indicators.rs` was deleted; `indicator_manager.rs` is its replacement.

4. **JS scripting crate added**: The `script/` crate is entirely new (untracked).

---

## Gap Detail Cards

### [PRIORITY: P0] Options Data Pipeline is Incomplete

**Description**: The `OptionsMessage` enum, the `Message::Options` variant, and the handler functions in `src/app/update/options.rs` all exist but are annotated `#[allow(dead_code)]`. No code path produces these messages. The `OptionsDataService`, `GexCalculationService`, and the three `Massive*Repository` types are initialized at startup (if API key is present) but are never called thereafter.

**Affected files**:
- `src/app/mod.rs:122,191` — dead enum annotated
- `src/app/update/options.rs` — handlers that log only
- `src/app/services.rs:42–113` — service initialization
- `src/app/update/mod.rs:39–46` — routing that does nothing useful

**Recommended action**: Wire `Task::perform(...)` calls to load option chains/GEX when a pane targets an equity ticker, store results in `Kairos::options_data`, and render them in the pane view. Or explicitly remove if options support is descoped.

---

### [PRIORITY: P0] Link Group Ticker Switching is a Stub

**Description**: `SwitchLinkGroup` sets `state.link_group = group` and clears the modal, but includes a TODO that chart loading after group assignment is not implemented.

**Affected files**:
- `src/screen/dashboard/update.rs:143–169`

**Recommended action**: After assigning the link group, call `switch_tickers_in_group` for all panes in the same group. The infrastructure (`switch_tickers_in_group`) already exists — it just needs to be invoked.

---

### [PRIORITY: P1] `FocusWidget` Effect Silently Dropped

**Description**: The pane update returns `Effect::FocusWidget(id)` to signal that a widget (e.g., search field) should receive keyboard focus when a modal opens. The dashboard update handler explicitly returns `Task::none()` with a TODO.

**Affected files**:
- `src/screen/dashboard/update.rs:206–209`
- `src/screen/dashboard/pane/update.rs:467–473`
- `src/screen/dashboard/pane/effects.rs:11`

**Recommended action**: Use `iced::widget::text_input::focus(id)` wrapped in `Task::perform` to forward the focus to Iced's runtime.

---

### [PRIORITY: P1] `kairos` App Crate Has Near-Zero Test Coverage

**Description**: The main application crate (`src/`) has 11 test functions total, all in low-level chart utilities. No tests exist for: message dispatch, pane lifecycle, chart loading, download flows, feed management, replay, modal state, or drawing operations.

**Recommended action**: Introduce unit tests for pure update functions (those that take state + message and return new state), starting with `ChartMessage`, `DownloadMessage`, and pane `Effect` handling.

---

### [PRIORITY: P1] Replay Skip Controls Not Implemented

**Description**: `Message::JumpForward` and `Message::JumpBackward` are declared but suppressed as dead code. The `ReplayEngine` has a `jump()` method available; only the UI wiring and handler dispatch are missing.

**Affected files**:
- `src/modals/replay/mod.rs:97–100`
- `src/app/update/replay.rs` (handler missing)

**Recommended action**: Add two buttons in the replay controller UI that emit `JumpForward`/`JumpBackward`; add a handler in `src/app/update/replay.rs` that calls `replay_engine.lock().jump(...)`.

---

### [PRIORITY: P1] TickerInfo / FuturesTickerInfo Dual Types

**Description**: `exchange::TickerInfo` (old type) and `exchange::FuturesTickerInfo` (new type) coexist. Conversion functions `ticker_info_to_old_format` / `old_format_to_ticker_info` live in `src/chart/comparison/mod.rs` with an explicit removal TODO. `TimeAndSalesPanel` and `LadderPanel` still use `TickerInfo`.

**Affected files**:
- `src/chart/comparison/mod.rs:512–530` — shim
- `src/chart/comparison/types.rs:22` — `Series.ticker_info: TickerInfo`
- `src/screen/dashboard/panel/timeandsales.rs:8,80` — `TickerInfo` import
- `src/screen/dashboard/panel/ladder.rs:11,65` — `TickerInfo` import

**Recommended action**: Replace all `TickerInfo` usages in panel types with `FuturesTickerInfo`, then delete the compatibility shim.

---

### [PRIORITY: P1] Custom JS Indicators Have No User-Facing UI

**Description**: The `kairos-script` crate provides a complete QuickJS-based indicator engine. 17 bundled `.js` scripts exist in `assets/scripts/`. The registry is initialized at startup. However, there is no UI to: (a) browse custom scripts, (b) see which scripts loaded, (c) reload scripts after editing, or (d) place scripts in the user scripts directory.

**Affected files**:
- `script/` — entire crate
- `src/app/services.rs:4–39` — initialization

**Recommended action**: Add a "Custom Indicators" section in the data feeds or settings modal showing script count, load status, and a button to open the user scripts folder.

---

### [PRIORITY: P2] Study `panic!()` in Tests

**Description**: 35+ instances of `panic!("Expected X output")` inside `#[test]` functions in `study/`. While these do cause test failures, they produce panic backtraces rather than clean assertion messages, making failures harder to diagnose in CI.

**Recommended action**: Replace `_ => panic!("Expected X output")` with `assert!(matches!(output, StudyOutput::X(_)), "Expected X, got {:?}", output)`.

---

### [PRIORITY: P2] Branching Migration Path Not Supported

**Description**: `get_migration_path` in `data/src/state/persistence.rs:114–127` performs a linear forward scan (`v0 → v1 → v2`). The comment acknowledges branching is needed for concurrent development on multiple migration branches.

**Affected files**:
- `data/src/state/persistence.rs:114–128`

**Recommended action**: This is acceptable for a project at current scale. Document the constraint explicitly; revisit when adding the third migration.

---

### [PRIORITY: P2] Root README is Empty

**Description**: `README.md` at the workspace root contains only:
```html
<img src="./assets/illustrations/header.svg" ... />
```

No text, no feature description, no build instructions.

**Recommended action**: Populate with project overview, feature list, build instructions, and links to crate-level READMEs.

---

### [PRIORITY: P3] `#[allow(dead_code)]` on Entire Component Modules

**Description**: All six component sub-modules in `src/components/` are blanket-suppressed from dead-code warnings:
- `display/`, `form/`, `input/`, `layout/`, `overlay/`, `primitives/`

This hides which components are actually unused, making cleanup impossible without temporarily removing the suppression.

**Recommended action**: Remove `#![allow(dead_code)]` from each module once the refactoring stabilizes, and resolve the resulting warnings individually.

---

## Appendix: Test Count Summary

| File | Test Count |
|------|-----------|
| `study/src/orderflow/big_trades.rs` | 19 |
| `study/src/volatility/atr.rs` | 7 |
| `study/src/trend/ema.rs` | 7 |
| `study/src/trend/sma.rs` | 7 |
| `study/src/momentum/rsi.rs` | 7 |
| `study/src/momentum/macd.rs` | 6 |
| `study/src/volume/obv.rs` | 4 |
| `study/src/volume/cvd.rs` | 4 |
| `study/src/volatility/bollinger.rs` | 5 |
| `study/src/momentum/stochastic.rs` | 5 |
| `study/src/trend/vwap.rs` | 5 |
| `data/src/services/gex_calculator.rs` | 8 |
| `data/src/domain/panel/trade_aggregator.rs` | 6 |
| `data/src/feed/types.rs` | 5 |
| `data/src/domain/panel/depth_grouping.rs` | 7 |
| `data/src/services/feed_merger.rs` | 5 |
| `data/src/state/persistence.rs` | 6 |
| `data/src/domain/aggregation.rs` | 6 |
| `data/src/domain/options.rs` | 6 |
| `data/src/domain/panel/chase_tracker.rs` | 4 |
| **`src/` (kairos)** | **~11 (all low-level chart utils)** |
| `exchange/src/adapter/massive/decoder.rs` | 5 |
| `exchange/src/adapter/massive/mapper.rs` | 5 |
| `script/src/runtime/ta.rs` | 6 |
