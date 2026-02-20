# Implementation Plan — Kairos Codebase Review

**Date**: 2026-02-20
**Author**: Implementation Plan Agent
**Input**: Reports 01-08 (Architecture, Quality, Robustness, Performance, Consistency, Completeness, Synthesis, Target Architecture)

---

## Overview

This document is the master implementation plan derived from eight independent audit reports covering 101 findings (7 Critical, 23 High, 37 Medium, 34 Low) across a ~76K LOC Rust desktop application. The plan is organized into seven phases spanning approximately six weeks, ordered by criticality and dependency relationships.

**Guiding Principles:**
- Every phase boundary produces a compilable, test-passing codebase
- Critical crash/security fixes come first, before any refactoring
- Structural changes precede type system changes (avoid merge conflicts)
- Performance work follows structural work (measurements are more reliable on clean code)
- Each item includes a verification step so the change can be validated independently

---

## Phase 0 — Critical Fixes (Day 1)

These items fix crashes, data corruption, or security issues. Each must be landed immediately and independently.

---

### P0-001: Fix `active_dashboard().expect()` Crash on Corrupt State

**Description**: `active_dashboard()` and `active_dashboard_mut()` call `.expect("No active layout")` and `.expect("No active dashboard")` on every UI tick. A corrupt `app-state.json` (missing layout, empty layout list, power-loss during save) makes the app permanently unlaunchable.

**Files affected**:
- `src/app/state.rs:13-33`
- `src/app/mod.rs` (all callers of `active_dashboard()`)
- `src/app/update/chart.rs`
- `src/app/update/download.rs`
- `src/app/update/feeds.rs`
- `src/app/update/navigation.rs`
- `src/app/update/preferences.rs`
- `src/app/update/replay.rs`

**Change**: Return `Option<&Dashboard>` / `Option<&mut Dashboard>` instead of panicking. Callers handle `None` by showing an empty state or synthesizing a default layout. In `Kairos::new()`, ensure at least one layout always exists by creating a default if the loaded state has zero layouts.

**Dependencies**: None
**Estimated effort**: M
**Risk**: Medium (touches many callers, but each change is mechanical: add `?` or `if let Some`)
**Verification**: Delete `app-state.json`, launch app, confirm it opens with a default layout. Also corrupt the file with truncated JSON, confirm the app still launches.
**Category**: robustness

---

### P0-002: Replace `block_on()` in Replay Handlers With `Task::perform`

**Description**: 9+ `block_on()` calls in `src/app/update/replay.rs` block the Iced UI event loop thread. Operations like play/pause/stop/seek/set_speed/jump/load_data all call `Handle::current().block_on(engine.method())` inside `spawn_blocking`, creating nested runtime interaction that can deadlock under thread pool exhaustion and causes visible UI freezing during replay.

**Files affected**:
- `src/app/update/replay.rs:16-17,21-22,36-37,49-50,57-58,62-63` (all `replay_engine_action` usages)
- `src/app/update/replay.rs:186-260` (`replay_load_data` function)
- `src/app/update/replay.rs:286-300` (histogram computation)

**Change**: Replace `spawn_blocking` + `block_on` pattern with `Task::perform(async move { engine.lock().await.play().await })`. For the initial `replay_load_data` that does CPU-bound aggregation, use `Task::perform` wrapping the async operations directly. Add a new `ReplayResult` message variant if needed for async completion.

**Dependencies**: None
**Estimated effort**: L
**Risk**: Medium (replay behavior may change subtly; requires manual testing of all replay operations)
**Verification**: Run replay on ES data: test play, pause, stop, seek, speed change, and jump. Confirm no UI freezes. Monitor with `RUST_LOG=debug` for any runtime warnings.
**Category**: robustness/performance

---

### P0-003: Replace `block_on()` in Chart Data Loading

**Description**: `src/app/update/chart.rs:189-191` calls `spawn_blocking` + `block_on` for chart data rebuild operations. This blocks a Tokio blocking thread while performing async I/O, risking thread pool starvation.

**Files affected**:
- `src/app/update/chart.rs:189-191`

**Change**: Convert to `Task::perform(async move { ... })` pattern. The rebuild operation should be fully async.

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low (single call site, behavior should be identical)
**Verification**: Load a candlestick chart, switch timeframe, confirm chart data loads without UI freezing.
**Category**: robustness/performance

---

### P0-004: Fix `layouts.first().unwrap()` Panic in Download Handler

**Description**: `src/app/update/download.rs:91,284` calls `layouts.first().unwrap()` as fallback when `active_layout_id()` returns `None`. If the layout list is empty (possible after a failed state load), this panics in a background task path.

**Files affected**:
- `src/app/update/download.rs:91,284`

**Change**: Replace `.unwrap()` with `.ok_or_else(|| "No layouts available".to_string())` and propagate the error, or return `Task::none()` with an error toast.

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: (Hard to reproduce without state corruption.) Code review to confirm no `.unwrap()` remains on `layouts.first()`.
**Category**: robustness

---

### P0-005: Make State Persistence Atomic

**Description**: `data/src/state/persistence.rs` writes directly to `app-state.json`. A crash or power loss during write corrupts the state file, which then triggers P0-001's crash path.

**Files affected**:
- `data/src/state/persistence.rs` (the `save_state` function)

**Change**: Write to `app-state.json.tmp` first, then `fs::rename` to `app-state.json`. On Unix, `rename` is atomic. On Windows, use `std::fs::rename` which replaces the target atomically on NTFS.

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low (transparent change, no API surface impact)
**Verification**: Run the app, change a setting, kill the process during save (add a 2-second `thread::sleep` in the write path temporarily). Confirm the state file is either the old state or the new state, never corrupted.
**Category**: robustness

---

### P0-006: Fix `Price + Price` Operator Panic on Overflow

**Description**: `exchange/src/util.rs:277,316` — the `+` and `-` operators on `exchange::Price` call `checked_add`/`checked_sub` then `.expect(...)`. Any arithmetic on extreme price values (possible with malformed exchange data) will abort the process. These operators are used in rendering hot paths.

**Files affected**:
- `exchange/src/util.rs:255,277,316` (`add_steps`, `Add` impl, `Sub` impl)

**Change**: Replace `expect` with `saturating_add`/`saturating_sub`. Add explicit `checked_add`/`checked_sub` methods for callers that need overflow detection.

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low (saturating arithmetic is safe and behavior under normal values is identical)
**Verification**: Write a unit test: `Price::from_units(i64::MAX) + Price::from_units(1)` should return `Price::from_units(i64::MAX)`, not panic.
**Category**: robustness

---

### P0-007: Fix `from_utf8().unwrap()` on Exchange Symbol Bytes

**Description**: `data/src/domain/futures.rs:277,282,292` — `FuturesTicker::as_str()`, `product()`, `display_name()` call `std::str::from_utf8(...).unwrap()`. Non-UTF-8 bytes from exchange data deserialization would panic in the data layer.

**Files affected**:
- `data/src/domain/futures.rs:277,282,292,358`

**Change**: Replace `from_utf8(...).unwrap()` with `from_utf8_lossy(...)` or return `Result<&str, ...>`. For `Display` impl, use lossy conversion.

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: Write a unit test constructing a `FuturesTicker` with non-UTF-8 bytes, confirm `as_str()` returns a replacement character string instead of panicking.
**Category**: robustness

---

### P0-008: Fix `assert!(!trades.is_empty())` in Production Aggregation

**Description**: `data/src/domain/aggregation.rs:126,276` — `assert!` in production code. If called with an empty trade/candle slice (edge case: zero-trade day, filter producing empty results), the app crashes.

**Files affected**:
- `data/src/domain/aggregation.rs:126,136,144,148,276,282,285,288`

**Change**: Change `assert!` to `debug_assert!`. Add an early return with `Err(AggregationError::EmptyInput)` at the call site for the empty case. The subsequent `.unwrap()` calls on `max()`/`min()`/`last()` become safe because the function returns early on empty input.

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: Existing tests pass. Add a test: `build_candle_from_trades(&[])` returns `Err`, not a panic.
**Category**: robustness

---

## Phase 1 — Structural Foundation (Week 1)

File/module reorganization and crate boundary changes. Each step compiles and tests pass independently. These changes are mostly code-motion with minimal logic changes.

---

### P1-001: Extract Global Singletons From `src/app/mod.rs` to `src/app/globals.rs`

**Description**: Move the 4 `OnceLock` statics (`DOWNLOAD_PROGRESS`, `RITHMIC_EVENTS`, `REPLAY_EVENTS`, `RITHMIC_SERVICE_RESULT`) and their getter functions to a dedicated file.

**Files affected**:
- `src/app/mod.rs:31-68` (source)
- New file: `src/app/globals.rs`
- All callers of `get_download_progress()`, `get_rithmic_events()`, `get_replay_events()`

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low (pure code motion, no logic change)
**Verification**: `cargo build`, `cargo test`, `cargo clippy`
**Category**: architecture

---

### P1-002: Extract Message Enums From `src/app/mod.rs` to `src/app/messages.rs`

**Description**: Move `Message`, `ChartMessage`, `DownloadMessage`, `OptionsMessage` enum definitions to a dedicated file. Keep `mod.rs` focused on the `Kairos` struct.

**Files affected**:
- `src/app/mod.rs` (source — enum definitions)
- New file: `src/app/messages.rs`
- All files importing `app::Message`, `app::ChartMessage`, `app::DownloadMessage`

**Dependencies**: P1-001 (globals extracted first reduces mod.rs, avoids merge conflict)
**Estimated effort**: M
**Risk**: Low (pure code motion, many import path updates)
**Verification**: `cargo build`, `cargo test`
**Category**: architecture

---

### P1-003: Extract `view_with_modal()` to `src/app/sidebar_view.rs`

**Description**: Move the 375-line `view_with_modal()` method to a dedicated file. This is the largest single function in `mod.rs`.

**Files affected**:
- `src/app/mod.rs:640-1014` (source)
- New file: `src/app/sidebar_view.rs`

**Dependencies**: P1-001, P1-002
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`, visually confirm sidebar modals still render correctly
**Category**: architecture

---

### P1-004: Extract `view()` to `src/app/view.rs`

**Description**: Move `Kairos::view()` (~230 lines) to a dedicated file. After extracting `view_with_modal` (P1-003), `view()` becomes the main remaining code block in `mod.rs`.

**Files affected**:
- `src/app/mod.rs:401-631` (source)
- New file: `src/app/view.rs`

**Dependencies**: P1-003
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`, `cargo run --release`, confirm main window renders
**Category**: architecture

---

### P1-005: Extract `FUTURES_PRODUCTS` and `build_tickers_info()` to `src/app/ticker_registry.rs`

**Description**: Move the `FUTURES_PRODUCTS` constant and `build_tickers_info()` function from `mod.rs` to a dedicated file. This is preparation for P2-009 (consolidating all three FUTURES_PRODUCTS definitions).

**Files affected**:
- `src/app/mod.rs:1018+` (source)
- New file: `src/app/ticker_registry.rs`

**Dependencies**: P1-002
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`
**Category**: architecture

---

### P1-006: Move `DataError`, `data_path()`, `open_data_folder()` Out of `data/src/lib.rs`

**Description**: `data/src/lib.rs` defines `DataError`, `data_path()`, `open_data_folder()`, and `lock_or_recover()`. These should be in their respective modules: `DataError` to `data/src/error.rs` (new file), and I/O functions to the GUI crate (preparation for P1-008).

**Files affected**:
- `data/src/lib.rs` (source)
- New file: `data/src/error.rs`
- All callers of `data::DataError`

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build --package kairos-data`, `cargo test --package kairos-data`
**Category**: architecture

---

### P1-007: Move Platform I/O Functions Out of `kairos-data`

**Description**: Move `data_path()` and `open_data_folder()` to `src/platform.rs` in the GUI crate. These functions use `dirs-next` and `open` which are platform I/O, not business logic.

**Files affected**:
- `data/src/lib.rs` (remove functions)
- New file: `src/platform.rs`
- All callers in `src/` that use `data::data_path()` or `data::open_data_folder()`
- `data/Cargo.toml` (remove `dirs-next` and `open` after P1-008 completes)

**Dependencies**: P1-006
**Estimated effort**: M
**Risk**: Low (callers just change import path)
**Verification**: `cargo build`, `cargo run --release`, confirm data folder opens from settings
**Category**: architecture

---

### P1-008: Move `SecretsManager` Out of `kairos-data`

**Description**: Move `data/src/secrets/mod.rs` (436 lines) to `src/secrets.rs` in the GUI crate. Keep `ApiProvider` and `ApiKeyStatus` enums in `data/src/config/` as they are domain configuration.

**Files affected**:
- `data/src/secrets/mod.rs` (source — move to GUI crate)
- New file: `src/secrets.rs`
- `data/Cargo.toml` (remove `keyring`, `base64`)
- `src/app/services.rs` (update import)
- `src/app/update/feeds.rs` (update import)

**Dependencies**: P1-007
**Estimated effort**: M
**Risk**: Low
**Verification**: `cargo build --package kairos-data` (confirm no `keyring` dependency), `cargo run --release`, configure Rithmic credentials via UI, confirm they persist.
**Category**: architecture

---

### P1-009: Delete `src/chart/study/` Dead Module

**Description**: `src/chart/study/` contains 5 files implementing POC, value area, volume profile, imbalance, and npoc — all duplicated by the `kairos-study` crate. Only `TradeGroup` and `Footprint` type aliases are actively used (in `candlestick/mod.rs`).

**Files affected**:
- `src/chart/study/` (entire directory — delete)
- `src/chart/mod.rs:14` (remove `#[allow(dead_code)] pub(crate) mod study;`)
- `src/chart/candlestick/mod.rs` (move `TradeGroup`/`Footprint` aliases here or to `footprint.rs`)

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low (confirmed dead code via `#[allow(dead_code)]` and report analysis)
**Verification**: `cargo build`, `cargo test`, confirm candlestick/footprint charts still render
**Category**: quality

---

### P1-010: Remove `#![allow(dead_code)]` From Component Modules

**Description**: All six component sub-modules (`display`, `form`, `input`, `layout`, `overlay`, `primitives`) carry `#![allow(dead_code)]`. Remove the suppression and delete genuinely unused items.

**Files affected**:
- `src/components/display/mod.rs:1`
- `src/components/form/mod.rs:1`
- `src/components/input/mod.rs:1`
- `src/components/layout/mod.rs:1`
- `src/components/overlay/mod.rs:1`
- `src/components/primitives/mod.rs:1`
- `src/components/mod.rs` (remove `#![allow(unused_imports)]`)
- Various files within each module (delete unused items)

**Dependencies**: None
**Estimated effort**: M
**Risk**: Low (compile-guided: compiler tells you exactly what is unused)
**Verification**: `cargo build` with no warnings (for dead code), `cargo test`
**Category**: quality

---

### P1-011: Remove Stale `#[allow(dead_code)]` Suppressions

**Description**: Several `#[allow(dead_code)]` annotations are stale (the items ARE used): `src/chart/perf/mod.rs:1` (lod is actively used), `src/chart/drawing/mod.rs:10` (persistence). Remove stale suppressions to restore compiler checking.

**Files affected**:
- `src/chart/perf/mod.rs:1`
- `src/chart/drawing/mod.rs:9-10`
- `src/chart/core/mod.rs:12`
- `src/chart/mod.rs:16` (`study_renderer` — assess if wired or truly dead)

**Dependencies**: P1-009 (removing study/ first simplifies this)
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build` with no new warnings
**Category**: quality

---

### P1-012: Feature-Gate Options Pipeline

**Description**: The `OptionsMessage` enum, `Message::Options` variant, and handler functions are annotated `#[allow(dead_code)]`. The options service is initialized at startup but never used. Feature-gate with `#[cfg(feature = "options")]` to avoid dead initialization.

**Files affected**:
- `src/app/mod.rs:122-191` (message variants)
- `src/app/update/options.rs` (handlers)
- `src/app/services.rs:42-113` (service initialization)
- `Cargo.toml` (add `options` feature)

**Dependencies**: P1-002 (messages extracted first)
**Estimated effort**: S
**Risk**: Low (feature-gating suppresses cleanly)
**Verification**: `cargo build` (without `--features options`) has no dead-code warnings for options types. `cargo build --features options` still compiles.
**Category**: quality

---

### P1-013: Delete `draw_drawings` Dead Wrapper

**Description**: `src/chart/drawing/render.rs:15-25` — `draw_drawings()` is a thin wrapper never called (callers use the two inner functions directly). It is suppressed with `#[allow(dead_code)]`.

**Files affected**:
- `src/chart/drawing/render.rs:15-25`

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`
**Category**: quality

---

## Phase 2 — Type System & Error Handling Cleanup (Week 2)

Type unification, error type redesign, removal of `unwrap()` calls. Changes in this phase touch many files but are mechanically systematic.

---

### P2-001: Remove `iced_core` From `kairos-data`

**Description**: Define a local `Rgba(f32,f32,f32,f32)` and `ThemeId(String)` in the data crate. Replace all `iced_core::Color` usage in pane state and `iced_core::Theme` in theme config. Write a state migration (v2 -> v3) for the serialization format change. Add `From<Rgba> for iced::Color` and vice versa in the GUI crate.

**Files affected**:
- `data/Cargo.toml` (remove `iced_core`)
- `data/src/config/theme.rs` (replace `Theme(iced_core::Theme)` with `ThemeId(String)`)
- `data/src/state/pane.rs` (`CandleStyle`, `ComparisonConfig` — replace `iced_core::Color` with `Rgba`)
- `data/src/drawing/types.rs` (replace `iced_core::Color` `From` impl with local `Rgba`)
- `data/src/state/persistence.rs` (add v2->v3 migration)
- `src/app/mod.rs` (add `From` impls at GUI boundary)
- `src/style/` (add conversion helpers)

**Dependencies**: P1-006, P1-007, P1-008 (all I/O deps removed first so `iced_core` is the last)
**Estimated effort**: L
**Risk**: High (persisted state format changes; migration must handle all existing state files)
**Verification**: Save state with old format, upgrade, confirm all colors and theme survive. Run full app and verify all chart colors render correctly.
**Category**: architecture

---

### P2-002: Unify `data::Price` and `exchange::Price`

**Description**: Merge `exchange::util::Price` checked arithmetic into `data::domain::types::Price` as saturating. Delete `exchange::util::Price` struct. Add `pub use kairos_data::Price;` to `exchange/src/util.rs`. Update all `exchange::util::Price` usages.

**Files affected**:
- `data/src/domain/types.rs` (add `saturating_add`, `saturating_sub`, `checked_add`, `checked_sub`, `fmt_with_precision`)
- `exchange/src/util.rs` (delete `Price` struct, add `pub use kairos_data::Price;`, keep `PriceStep`, `Power10`)
- ~50 files across all crates (update `exchange::util::Price` to `data::Price`)

**Dependencies**: P0-006 (Price overflow fix first)
**Estimated effort**: XL
**Risk**: High (touches ~50 files; `exchange::Price.units` was public, `data::Price.units` is private — all direct field access must change to `.units()` method)
**Verification**: `cargo build` on all workspace crates. `cargo test` passes. Run the app, load various chart types, confirm price display is correct.
**Category**: consistency

---

### P2-003: Change `FuturesTickerInfo.tick_size` From `f32` to `Price`

**Description**: `data/src/domain/futures.rs` — `FuturesTickerInfo.tick_size: f32`. Change to `tick_size: Price`. Delete the `min_ticksize()` conversion method (now identity). Update all construction sites.

**Files affected**:
- `data/src/domain/futures.rs:402-408` (field type change)
- `src/app/mod.rs` or `src/app/ticker_registry.rs` (`build_tickers_info` — construct with `Price`)
- All consumers of `ticker_info.tick_size` and `ticker_info.min_ticksize()`

**Dependencies**: P2-002 (Price unification first)
**Estimated effort**: M
**Risk**: Medium (many call sites, but compiler will catch all)
**Verification**: `cargo build`, `cargo test`, load a chart and confirm tick sizes display correctly
**Category**: consistency

---

### P2-004: Delete `exchange::TickerInfo` — Unify to `FuturesTickerInfo`

**Description**: `exchange/src/types.rs` defines `TickerInfo` as a near-duplicate of `FuturesTickerInfo`. Replace all usages in `Ladder`, `TimeAndSales`, `ComparisonChart` with `FuturesTickerInfo`. Delete the compatibility shim in `comparison/mod.rs`.

**Files affected**:
- `exchange/src/types.rs` (delete `TickerInfo` struct and `From` impls)
- `src/chart/comparison/mod.rs:512-530` (delete `ticker_info_to_old_format` shim)
- `src/chart/comparison/types.rs:22` (`Series.ticker_info: TickerInfo` -> `FuturesTickerInfo`)
- `src/screen/dashboard/panel/ladder.rs:11,65`
- `src/screen/dashboard/panel/timeandsales.rs:8,80`

**Dependencies**: P2-003 (tick_size as Price first)
**Estimated effort**: M
**Risk**: Medium (panel types need updating, but compiler-guided)
**Verification**: `cargo build`, test ladder and time-and-sales panels with live data
**Category**: consistency

---

### P2-005: Rename Exchange Wire Types to `Raw*`

**Description**: Rename `exchange::types::Trade` to `RawTrade`, `Kline` to `RawKline`, `Depth` to `RawDepth` to clearly distinguish wire-format types from domain types. Ensure a single `From<RawTrade> for data::Trade` conversion exists.

**Files affected**:
- `exchange/src/types.rs` (rename structs)
- `exchange/src/adapter/databento/mapper.rs` (update all usages)
- `exchange/src/adapter/rithmic/mapper.rs` (update all usages)
- `exchange/src/adapter/massive/mapper.rs` (update all usages)
- `exchange/src/repository/` (update all usages)

**Dependencies**: P2-004
**Estimated effort**: M
**Risk**: Low (rename within exchange crate only, no cross-crate breakage)
**Verification**: `cargo build --package kairos-exchange`, `cargo test --package kairos-exchange`
**Category**: consistency

---

### P2-006: Implement `AppError` on `InternalError`

**Description**: `src/error.rs` — `InternalError` is a 3-variant enum holding free-form strings. It does not implement `AppError`. Add `severity` and `retriable` fields to each variant and implement the `AppError` trait.

**Files affected**:
- `src/error.rs` (add fields, implement `AppError` trait)

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`, confirm error messages display in toasts with correct severity
**Category**: robustness

---

### P2-007: Add `From<AdapterError> for exchange::Error`

**Description**: `exchange/src/adapter/error.rs` defines `AdapterError`. `exchange/src/error.rs` defines `Error`. There is no `From` conversion between them, forcing manual mapping at every boundary.

**Files affected**:
- `exchange/src/error.rs` (add `From<AdapterError>` impl)

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build --package kairos-exchange`
**Category**: robustness

---

### P2-008: Standardize Mutex Poison Recovery

**Description**: The codebase mixes `lock().unwrap()` (panics on poison) and `lock().unwrap_or_else(|e| e.into_inner())` (recovers). Standardize to use `lock_or_recover()` (which already exists in `data/src/lib.rs`) everywhere.

**Files affected**:
- `src/app/update/download.rs:138,220,255,387,481,486`
- `src/app/update/feeds.rs:105`
- `src/layout.rs:75`
- `src/screen/dashboard/loading/chart_loading.rs:33,156`
- `src/screen/dashboard/loading/feed_management.rs:53`
- Any other `lock().unwrap()` sites

**Dependencies**: P1-006 (ensure `lock_or_recover` is accessible from GUI crate)
**Estimated effort**: S
**Risk**: Low (mechanical replacement)
**Verification**: `cargo build`, grep for bare `.lock().unwrap()` — should find zero in production code
**Category**: robustness

---

### P2-009: Consolidate `FUTURES_PRODUCTS` Into Single Definition

**Description**: The CME futures product catalogue is defined in 3 places with different shapes and partially overlapping content. Create a single canonical `data/src/products.rs` with a `FuturesProduct` struct.

**Files affected**:
- New file: `data/src/products.rs` (canonical definition)
- `data/src/lib.rs` (add `pub mod products;`)
- `src/app/mod.rs` or `src/app/ticker_registry.rs` (delete local `FUTURES_PRODUCTS`)
- `src/modals/download/mod.rs:16` (delete local constant, import from data)
- `exchange/src/adapter/databento/mapper.rs:76` (delete local vec!, import from data)

**Dependencies**: P2-003 (tick_size as Price), P1-005 (ticker_registry extracted)
**Estimated effort**: M
**Risk**: Medium (must reconcile differences: exchange mapper has ZT.c.0 not in UI list, tick size differences for ZF)
**Verification**: `cargo build` on all crates. Run the app, verify all 12 futures products appear in download and chart UIs. Verify ZT.c.0 is included if it was previously in the exchange mapper.
**Category**: consistency

---

### P2-010: Stop Re-exporting Third-Party Types From `exchange/src/lib.rs`

**Description**: `exchange/src/lib.rs` re-exports `databento::dbn::Schema` and `rithmic_rs::RithmicEnv`. This leaks third-party types into the exchange crate's public API.

**Files affected**:
- `exchange/src/lib.rs` (remove `pub use databento::dbn::Schema` and `pub use rithmic_rs::RithmicEnv`)
- `src/` (update imports to use `exchange`-local wrapper types or import directly from third-party)

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`
**Category**: architecture

---

### P2-011: Stop Re-exporting Domain Types From `exchange/src/lib.rs`

**Description**: `exchange/src/lib.rs` re-exports `FuturesTicker`, `FuturesTickerInfo`, `FuturesVenue`, etc. from `kairos-data`. GUI should import these from `data` directly.

**Files affected**:
- `exchange/src/lib.rs` (remove `pub use kairos_data::domain::...`)
- `src/app/mod.rs:20` and other GUI files (change `use exchange::FuturesTicker` to `use data::FuturesTicker`)

**Dependencies**: P2-004 (TickerInfo unified first)
**Estimated effort**: S
**Risk**: Low (import path changes only)
**Verification**: `cargo build`
**Category**: architecture

---

### P2-012: Replace Hardcoded "XX & Company" / "XXNCO" Strings

**Description**: Placeholder company names are hardcoded in `src/app/mod.rs:383-422`. Define `const APP_NAME: &str = "Kairos"` and use it.

**Files affected**:
- `src/app/mod.rs:383,385,422`

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: Run the app, confirm title bar shows "Kairos" instead of placeholder names.
**Category**: quality

---

### P2-013: Replace `from_utf8().unwrap()` With `from_utf8_lossy()` in `FuturesTicker`

**Description**: (Complementary to P0-007 — this covers additional `.unwrap()` sites in the Display impl and related methods.)

**Files affected**:
- `data/src/domain/futures.rs:358` (Display impl)

**Dependencies**: P0-007
**Estimated effort**: S
**Risk**: Low
**Verification**: Unit test with non-UTF-8 bytes in Display context.
**Category**: robustness

---

### P2-014: Fix `footprint.unwrap()` in Kline Settings View

**Description**: `src/modals/pane/settings/kline.rs:172` — `footprint.unwrap()` called in a view function after checking `chart_type.is_footprint()` but the variable may be `None` if data is loading.

**Files affected**:
- `src/modals/pane/settings/kline.rs:172`

**Change**: Replace with `if let Some(fp) = footprint { ... }` guard.

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: Open kline settings while chart is loading, confirm no panic.
**Category**: robustness

---

### P2-015: Fix `points.last().unwrap()` in Data Feed Preview

**Description**: `src/modals/data_feeds/preview.rs:110` — panics if `self.points` is empty.

**Files affected**:
- `src/modals/data_feeds/preview.rs:110`

**Change**: Add `if self.points.is_empty() { return; }` guard before accessing `last()`.

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: Open data feed preview with no data points, confirm no panic.
**Category**: robustness

---

### P2-016: Fix `find_poc().unwrap()` in Rendering Path

**Description**: `src/chart/study/poc.rs:80` — `find_poc(&footprint).unwrap()` panics if footprint is empty (candle with zero trades).

**Files affected**:
- `src/chart/study/poc.rs:80`

**Change**: Replace with `if let Some(poc) = find_poc(&footprint)` guard.

**Dependencies**: P1-009 (if study/ is deleted, this becomes N/A; otherwise fix before deletion)
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`
**Category**: robustness

---

### P2-017: Fix `feed_manager.get(feed_id).unwrap()` Race Condition

**Description**: `src/app/update/feeds.rs:162` — `.unwrap()` on a feed lookup that could return `None` if the manager was mutated between the existence check and the access.

**Files affected**:
- `src/app/update/feeds.rs:162`

**Change**: Replace with `if let Some(feed) = feed_manager.get(feed_id)` guard.

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`
**Category**: robustness

---

### P2-018: Replace `_ => unreachable!()` in `connect_rithmic_feed`

**Description**: `src/app/update/feeds.rs:165` — assumes `connect_rithmic_feed` is only called with `FeedConfig::Rithmic`. Any future caller with a different variant triggers an unreachable panic.

**Files affected**:
- `src/app/update/feeds.rs:165`

**Change**: Replace with `_ => { log::warn!("..."); return Task::none(); }`.

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`
**Category**: robustness

---

## Phase 3 — Code Quality & Dead Code Removal (Week 3)

Dead code deletion, duplication removal, naming standardization, and large file splits.

---

### P3-001: Split `exchange/src/adapter/databento/fetcher.rs` (1,411 Lines)

**Description**: Split the largest file in the project into focused sub-modules: `fetcher/mod.rs` (re-exports, `HistoricalDataManager` struct), `fetcher/manager.rs` (lifecycle/orchestration), `fetcher/gaps.rs` (date range gap detection), `fetcher/cost.rs` (cost estimation), `fetcher/download.rs` (concurrent download).

**Files affected**:
- `exchange/src/adapter/databento/fetcher.rs` (split into directory)
- `exchange/src/adapter/databento/mod.rs` (update module declaration)

**Dependencies**: None
**Estimated effort**: L
**Risk**: Medium (large refactor, must preserve all public API)
**Verification**: `cargo build --package kairos-exchange`, `cargo test --package kairos-exchange`, test historical data download
**Category**: architecture

---

### P3-002: Split `data/src/domain/chart.rs` (723 Lines) Into Submodules

**Description**: Split into `chart/mod.rs`, `chart/config.rs` (ChartConfig, ChartBasis, ChartType), `chart/data.rs` (ChartData, DataSegment, DataGap), `chart/view.rs` (ViewConfig, Autoscale, LoadingStatus), `chart/kline.rs` (KlineDataPoint, FootprintMode), `chart/heatmap.rs` (HeatmapIndicator, CoalesceKind).

**Files affected**:
- `data/src/domain/chart.rs` (split into directory)
- `data/src/domain/mod.rs` (update module declaration)
- `data/src/lib.rs` (update re-exports if needed)

**Dependencies**: None
**Estimated effort**: M
**Risk**: Low (pure code motion within same crate)
**Verification**: `cargo build --package kairos-data`, `cargo test --package kairos-data`
**Category**: architecture

---

### P3-003: Delete Duplicate `dashboard_modal` Wrapper

**Description**: `src/modals/mod.rs:77-90` is a backward-compatible wrapper around the new `ModalShell` API. Migrate all callers to the `ModalShell` constructor in `src/components/overlay/modal_shell.rs`.

**Files affected**:
- `src/modals/mod.rs:77-90` (delete old function)
- All callers of the old `dashboard_modal` function

**Dependencies**: None
**Estimated effort**: M
**Risk**: Low (compiler-guided migration)
**Verification**: `cargo build`, visually confirm all modals still render
**Category**: quality

---

### P3-004: Extract Confirm Dialog Overlay Helper

**Description**: The ~14-line confirm-dialog overlay block is duplicated in `view_with_modal` (Settings arm and DataFeeds arm). Extract to a helper function.

**Files affected**:
- `src/app/mod.rs:772-786,957-971` (or `src/app/sidebar_view.rs` if extracted by P1-003)

**Dependencies**: P1-003 (sidebar view extracted first)
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`, confirm dialog still works in Settings and DataFeeds modals
**Category**: quality

---

### P3-005: Extract Chart Text Size Constant

**Description**: `let text_size = 9.0 / chart.scaling;` appears 3 times in `src/chart/heatmap/render.rs`. Extract as `const CHART_LABEL_BASE_PX: f32 = 9.0;` and helper function.

**Files affected**:
- `src/chart/heatmap/render.rs:397,545,723`

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`, visually confirm heatmap labels
**Category**: quality

---

### P3-006: Introduce `VolumeBarSpec` Struct for `draw_volume_bar`

**Description**: `draw_volume_bar()` takes 11 parameters (exceeds clippy limit of 5). Introduce `VolumeBarSpec { buy_qty, sell_qty, max_qty, buy_color, sell_color, alpha }` struct.

**Files affected**:
- `src/chart/mod.rs:431-498` (refactor function signature)
- `src/chart/heatmap/render.rs` (update call sites)
- `src/chart/candlestick/render.rs` (update call sites)

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`, `cargo clippy`, visually confirm volume bars
**Category**: quality

---

### P3-007: Introduce `ClusterLayout`/`ClusterStyle` Structs for `draw_clusters`

**Description**: `draw_clusters()` takes 17 parameters. Group into `ClusterLayout` and `ClusterStyle` structs.

**Files affected**:
- `src/chart/candlestick/footprint.rs:392-411` (refactor signature)
- Callers of `draw_clusters`

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`, visually confirm footprint clusters
**Category**: quality

---

### P3-008: Replace `panic!("Expected ...")` in Study Tests With `assert_matches!`

**Description**: 35+ `panic!()` calls in `study/src/` tests use `match output { X => ..., _ => panic!("Expected X") }`. Replace with `assert_matches!` (stable since Rust 1.82) or `assert!(matches!(...))`.

**Files affected**:
- `study/src/orderflow/big_trades.rs:689-1079`
- Other study test files with the same pattern

**Dependencies**: None
**Estimated effort**: M
**Risk**: Low (test-only changes)
**Verification**: `cargo test --package kairos-study` — all tests pass with cleaner output
**Category**: quality

---

### P3-009: Extract LOD Threshold Constants

**Description**: `src/chart/perf/lod.rs:111-120` — six magic number thresholds for LOD level decisions. Extract named constants.

**Files affected**:
- `src/chart/perf/lod.rs`

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`, `cargo test`
**Category**: quality

---

### P3-010: Rename `FLOWSURFACE_DATA_PATH` to `KAIROS_DATA_PATH`

**Description**: Old branding remnant. `data/src/lib.rs:122` and `data/src/state/persistence.rs:299` reference the old project name.

**Files affected**:
- `data/src/lib.rs:122` (or `src/platform.rs` if moved by P1-007)
- `data/src/state/persistence.rs:299`

**Dependencies**: P1-007
**Estimated effort**: S
**Risk**: Low (env var rename could break existing user setups — document in release notes)
**Verification**: Set `KAIROS_DATA_PATH`, confirm the app uses it. Set `FLOWSURFACE_DATA_PATH`, confirm it is ignored (or add backward-compat fallback).
**Category**: consistency

---

### P3-011: Update `exchange/Cargo.toml` Description

**Description**: Still reads "Databento adapter for futures market data in Kairos" but the crate implements Databento, Rithmic, and Massive adapters.

**Files affected**:
- `exchange/Cargo.toml:3`

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: Read the file.
**Category**: completeness

---

### P3-012: Remove `_tick_size` Unused Parameter in `draw_clusters`

**Description**: `src/chart/candlestick/footprint.rs:402` — `_tick_size: f32` is unused (underscore prefix). Remove the parameter if the function is free-standing.

**Files affected**:
- `src/chart/candlestick/footprint.rs:402`
- All callers of `draw_clusters`

**Dependencies**: P3-007 (if struct refactor happens first, this parameter is part of that change)
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`
**Category**: quality

---

### P3-013: Move `PushFrequency` From `exchange/src/lib.rs` to `exchange/src/adapter/stream.rs`

**Description**: `PushFrequency` is a domain type defined directly in `lib.rs` instead of in the module where it's used.

**Files affected**:
- `exchange/src/lib.rs` (remove definition)
- `exchange/src/adapter/stream.rs` (add definition)
- Callers of `exchange::PushFrequency`

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`
**Category**: architecture

---

### P3-014: Extract Databento Methods From `TradeRepository` Trait

**Description**: The `TradeRepository` trait in `data/src/repository/traits.rs` contains 5 Databento-specific methods (`_databento` suffix). Extract to `exchange/src/ext/databento.rs` as `DatabentoTradeExt` extension trait.

**Files affected**:
- `data/src/repository/traits.rs` (remove 5 methods)
- New file: `exchange/src/ext/databento.rs`
- New file: `exchange/src/ext/mod.rs`
- `exchange/src/repository/databento/trades.rs` (implement both traits)
- GUI callers that use Databento-specific methods (import extension trait)

**Dependencies**: None
**Estimated effort**: M
**Risk**: Medium (trait boundary change affects downstream callers)
**Verification**: `cargo build` on all crates, `cargo test`, test historical data download
**Category**: architecture

---

## Phase 4 — Performance Optimization (Week 4)

Rendering pipeline optimization, allocation reduction, async cleanup. Each with expected impact and measurement method.

---

### P4-001: Heatmap — Compute `visible_data_count` Once Per Frame

**Description**: `src/chart/heatmap/render.rs:86-98` and `render.rs:428-440` — the same BTreeMap range scan + summation runs twice per frame.

**Files affected**:
- `src/chart/heatmap/render.rs`

**Expected impact**: 10-50ms saved per frame during busy markets (one full BTreeMap scan eliminated)
**Measurement**: Add `log::debug!` timing around heatmap `draw()`, compare before/after

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`, visual regression check on heatmap rendering
**Category**: performance

---

### P4-002: Candlestick — Binary Search for Visible Candle Count

**Description**: `src/chart/candlestick/render.rs:76-87` — `iter().filter().count()` does O(N) scan per frame. Replace with `partition_point` binary search for O(log N).

**Files affected**:
- `src/chart/candlestick/render.rs:76-87`

**Expected impact**: 10K comparisons -> ~14 comparisons per frame (for 10K candles)
**Measurement**: Profile with `perf` or `cargo flamegraph` during rapid zoom

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`, visual regression check on candlestick chart
**Category**: performance

---

### P4-003: Candlestick — Binary Search for Crosshair Candle Lookup

**Description**: `src/chart/candlestick/render.rs:507` — `candles.iter().find(...)` is O(N) on every mouse move. Replace with binary search.

**Files affected**:
- `src/chart/candlestick/render.rs:507`

**Expected impact**: 10K comparisons -> ~14 per cursor move
**Measurement**: Profile during rapid cursor movement over candlestick chart

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`, confirm crosshair tooltip still shows correct candle data
**Category**: performance

---

### P4-004: Heatmap — Precompute Volume Profile Outside Draw Closure

**Description**: `src/chart/heatmap/render.rs:644-688` — volume profile is rebuilt inside the `Cache::draw` closure on every main cache invalidation. Precompute on data update and store as cached field.

**Files affected**:
- `src/chart/heatmap/mod.rs` (add `cached_volume_profile: Option<Vec<(f32, f32)>>` field)
- `src/chart/heatmap/render.rs:644-688` (read from cache instead of computing)

**Expected impact**: 10-50ms saved per frame with volume profile active
**Measurement**: Toggle volume profile on/off, compare frame times

**Dependencies**: None
**Estimated effort**: M
**Risk**: Medium (must correctly invalidate cache on data update)
**Verification**: Enable volume profile on heatmap, confirm it displays correctly after zooming and scrolling
**Category**: performance

---

### P4-005: Heatmap — Replace Vec Linear Scan in `add_trade` With BTreeMap

**Description**: `src/chart/heatmap/data.rs:183-195` — O(M) linear scan per trade for grouping. Replace `Vec<GroupedTrade>` with `BTreeMap<(i64, bool), f32>`.

**Files affected**:
- `src/chart/heatmap/data.rs:183-195`
- `src/chart/heatmap/data.rs` (GroupedTrade storage type change)
- `src/chart/heatmap/render.rs` (adapt iteration over grouped trades)

**Expected impact**: O(N*M) -> O(N*log M) during heatmap construction; significant for dense price levels
**Measurement**: Load a full-day ES heatmap, measure construction time

**Dependencies**: None
**Estimated effort**: M
**Risk**: Medium (storage type change affects rendering iteration)
**Verification**: `cargo build`, visual regression check on heatmap trade markers
**Category**: performance

---

### P4-006: Comparison Chart — Replace Linear Scan With `partition_point`

**Description**: `src/chart/comparison/render.rs:29` — `pts.iter().position(|(x, _)| *x >= ctx.min_x)` is O(N). Replace with `partition_point`.

**Files affected**:
- `src/chart/comparison/render.rs:29`
- `src/chart/comparison/types.rs:233` (if similar pattern exists)

**Expected impact**: Significant for long comparison series (millions of points)
**Measurement**: Load multi-year comparison, measure render time

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`, visual regression check on comparison chart
**Category**: performance

---

### P4-007: Aggregation — Move Sort Validation to Debug-Only

**Description**: `data/src/domain/aggregation.rs:89-94,204-209` — full O(N) sort validation on every chart load. Databento data is guaranteed pre-sorted.

**Files affected**:
- `data/src/domain/aggregation.rs:89-94,204-209`

**Expected impact**: 1-10ms saved per chart load (for 2-5M trades)
**Measurement**: Time `aggregate_trades_to_candles` before/after

**Dependencies**: P0-008 (assert fix should be done first)
**Estimated effort**: S
**Risk**: Low (data is sorted by contract, validation remains in debug builds)
**Verification**: `cargo test --package kairos-data`, confirm aggregation tests pass in both debug and release
**Category**: performance

---

### P4-008: Reduce Polling Overhead When Idle

**Description**: `src/app/subscriptions.rs:9-27` — `rithmic_event_monitor` and `replay_event_monitor` poll every 50ms even when no feed is active. Add an `AtomicBool` guard.

**Files affected**:
- `src/app/subscriptions.rs`
- `src/app/globals.rs` (add `AtomicBool` flags)

**Expected impact**: Reduces ~50-100 wakeups/second to near-zero when idle
**Measurement**: Monitor CPU usage with no data feeds connected

**Dependencies**: P1-001 (globals extracted)
**Estimated effort**: S
**Risk**: Low
**Verification**: Monitor CPU usage idle vs. active feed; confirm no regression in event delivery latency
**Category**: performance

---

### P4-009: Heatmap — Coalesce Identical Consecutive DepthRuns

**Description**: `src/chart/heatmap/data.rs:131-157` — `DepthRun` accumulation is unbounded for long sessions. Merge adjacent runs at the same price when quantity matches.

**Files affected**:
- `src/chart/heatmap/data.rs:131-157`

**Expected impact**: Reduces memory for long sessions (4.6M allocations -> much fewer with coalescing)
**Measurement**: Monitor memory usage during extended ES session

**Dependencies**: None
**Estimated effort**: M
**Risk**: Medium (must preserve visual fidelity of depth heatmap)
**Verification**: Run heatmap for extended period, compare visual output and memory usage
**Category**: performance

---

## Phase 5 — Feature Completion & Testing (Week 5+)

Implementation gaps, test coverage, completing partial features.

---

### P5-001: Implement Link Group Ticker Switching

**Description**: `src/screen/dashboard/update.rs:162-169` — `SwitchLinkGroup` sets the group field but does not propagate ticker changes to linked panes. The infrastructure (`switch_tickers_in_group`) already exists.

**Files affected**:
- `src/screen/dashboard/update.rs:162-169`

**Dependencies**: None
**Estimated effort**: M
**Risk**: Medium (must test multi-pane ticker synchronization)
**Verification**: Create 3 panes in same link group, switch ticker on one, confirm all update
**Category**: completeness

---

### P5-002: Implement Replay JumpForward/JumpBackward

**Description**: `src/modals/replay/mod.rs:97-100` — both messages are declared but suppressed as dead code. The `ReplayEngine.jump()` method exists.

**Files affected**:
- `src/modals/replay/mod.rs` (add UI buttons, remove `#[allow(dead_code)]`)
- `src/app/update/replay.rs` (add handler)

**Dependencies**: P0-002 (block_on fix must be done first)
**Estimated effort**: M
**Risk**: Low
**Verification**: Run replay, click forward/backward buttons, confirm position changes
**Category**: completeness

---

### P5-003: Implement `FocusWidget` Effect

**Description**: `src/screen/dashboard/update.rs:206-209` — `Effect::FocusWidget(_id)` is silently dropped. Use `iced::widget::text_input::focus(id)` to forward focus.

**Files affected**:
- `src/screen/dashboard/update.rs:206-209`

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: Open a modal with a search field, confirm the text input receives keyboard focus
**Category**: completeness

---

### P5-004: Add Unit Tests for GUI Crate Message Handlers

**Description**: The `kairos` (src/) crate has ~11 test functions total. Add unit tests for pure update functions: `ChartMessage`, `DownloadMessage`, pane `Effect` handling.

**Files affected**:
- `src/app/update/chart.rs` (add `#[cfg(test)]` module)
- `src/app/update/download.rs` (add tests)
- `src/screen/dashboard/pane/update.rs` (add tests)

**Dependencies**: P1-001, P1-002 (structural extractions make testing easier)
**Estimated effort**: XL
**Risk**: Low (test-only additions)
**Verification**: `cargo test` — new tests pass
**Category**: completeness

---

### P5-005: Add JS Indicator User-Facing UI

**Description**: The `kairos-script` crate provides a complete QuickJS-based indicator engine. 17 bundled `.js` scripts exist. But there is no UI for browsing, loading, or managing custom scripts.

**Files affected**:
- `src/modals/pane/indicator_manager.rs` (add "Custom Scripts" section)
- New file or section in settings modal

**Dependencies**: None
**Estimated effort**: L
**Risk**: Medium (new UI work)
**Verification**: Open indicator manager, see custom scripts listed with load status
**Category**: completeness

---

### P5-006: Wire Options Pipeline or Document as Descoped

**Description**: The options data pipeline (Massive/Polygon) has full exchange and service layers but zero UI wiring. Either wire `Task::perform` calls to load option chains/GEX or explicitly document as descoped.

**Files affected**:
- `src/app/update/options.rs` (add `Task::perform` calls)
- `src/screen/dashboard/pane/` (add options pane type)
- OR: Add `#[cfg(feature = "options")]` feature gate (done in P1-012)

**Dependencies**: P1-012 (feature-gated first)
**Estimated effort**: XL
**Risk**: High (new feature work, not just refactoring)
**Verification**: If wired: load an options chain, display GEX profile. If descoped: confirm feature is cleanly gated.
**Category**: completeness

---

### P5-007: Reduce `data/src/lib.rs` Re-export Surface

**Description**: `data/src/lib.rs` re-exports 50+ symbols at the crate root, including internal types like `aggregate_trades_to_candles`. Reduce to ~20 essential re-exports.

**Files affected**:
- `data/src/lib.rs` (trim re-exports)
- All `src/` files importing from `data::` (update to use full module paths)

**Dependencies**: P2-011 (exchange re-exports cleaned first)
**Estimated effort**: M
**Risk**: Low (compiler-guided: remove re-export, fix import errors)
**Verification**: `cargo build` on all crates
**Category**: architecture

---

### P5-008: Split `Side` Into `TradeSide` + `BookSide`

**Description**: `data/src/domain/types.rs` — `Side { Buy, Sell, Bid, Ask }` mixes trade-side and orderbook-side semantics. Split into two enums. This is a medium-priority refactor with moderate blast radius.

**Files affected**:
- `data/src/domain/types.rs` (split enum)
- `data/src/domain/entities.rs` (`Trade.side` -> `TradeSide`)
- `exchange/src/types.rs` (align `TradeSide`)
- `src/chart/` (update rendering code)
- `study/src/` (update study input/output types)

**Dependencies**: P2-005 (exchange wire types renamed first)
**Estimated effort**: L
**Risk**: Medium (touches domain, exchange, chart rendering, studies)
**Verification**: `cargo build` on all crates, `cargo test`, visual regression on all chart types
**Category**: consistency

---

### P5-009: Change `Study::compute()` to Return `Result`

**Description**: `study/src/traits.rs:30` — `compute` returns `()` instead of `Result<(), StudyError>`. Callers cannot detect computation failures.

**Files affected**:
- `study/src/traits.rs:30`
- All `Study` implementations (~15 files in `study/src/`)
- `src/chart/candlestick/mod.rs` (caller)
- `src/chart/study_renderer/` (caller)

**Dependencies**: None
**Estimated effort**: M
**Risk**: Medium (trait change affects all implementors)
**Verification**: `cargo build` on all crates, `cargo test --package kairos-study`
**Category**: consistency

---

## Phase 6 — Documentation & Polish (Ongoing)

Doc comments, README updates, CLAUDE.md refresh.

---

### P6-001: Update CLAUDE.md to Reflect Current Architecture

**Description**: `CLAUDE.md` lists `src/chart/indicator/` as active (it's deleted). It also lists `SecretsManager` in `data/` (to be moved). Update to match post-refactoring state.

**Files affected**:
- `CLAUDE.md`

**Dependencies**: All structural changes in Phases 1-3 complete
**Estimated effort**: M
**Risk**: Low
**Verification**: Read the document, compare with actual file tree
**Category**: completeness

---

### P6-002: Populate Root README.md

**Description**: Root `README.md` contains only an SVG image tag. Add project overview, feature list, build instructions, screenshots, and links to crate-level READMEs.

**Files affected**:
- `README.md`

**Dependencies**: None
**Estimated effort**: M
**Risk**: Low
**Verification**: Read the README
**Category**: completeness

---

### P6-003: Document JS Scripting System

**Description**: No documentation exists for: the JavaScript indicator API surface, how to create a custom `.js` indicator, where to place user scripts, or the expected manifest format.

**Files affected**:
- New file: `script/README.md`
- Optionally: `docs/scripting.md`

**Dependencies**: P5-005 (UI exists to reference)
**Estimated effort**: M
**Risk**: Low
**Verification**: Follow the documentation to create a custom indicator, confirm it loads
**Category**: completeness

---

### P6-004: Document `draw_volume_bar` Parameters

**Description**: The 11-parameter (now struct-based after P3-006) `draw_volume_bar` function has no doc comment.

**Files affected**:
- `src/chart/mod.rs:431` (or new location after P3-006)

**Dependencies**: P3-006
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo doc --open`, confirm documentation renders
**Category**: completeness

---

### P6-005: Fix `FuturesTicker` Serialization Hardcoded Venue String

**Description**: `data/src/domain/futures.rs` — `Serialize` impl uses hardcoded `"CMEGlobex"` instead of `self.venue.to_string()`.

**Files affected**:
- `data/src/domain/futures.rs:347-397`

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low (verify existing state files still deserialize correctly)
**Verification**: Serialize/deserialize a `FuturesTicker`, confirm round-trip identity. Check that existing `app-state.json` files load correctly.
**Category**: consistency

---

### P6-006: Use `StateVersion` Newtype Consistently

**Description**: `data/src/state/persistence.rs` — `AppState.version` is stored as `u32`, compared with `StateVersion::CURRENT.0`. Use `StateVersion` throughout.

**Files affected**:
- `data/src/state/persistence.rs`
- `data/src/state/app.rs` (if `version` field exists)

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build --package kairos-data`, `cargo test --package kairos-data`
**Category**: consistency

---

### P6-007: Log Dropped Replay Events

**Description**: `data/src/services/replay_engine.rs:585,599,611,612` — replay events silently dropped with `let _ =` when the channel is full. Add logging.

**Files affected**:
- `data/src/services/replay_engine.rs:585,599,611,612`

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: Run replay at high speed, check logs for drop warnings
**Category**: robustness

---

### P6-008: Log Study Parameter Update Errors

**Description**: `src/chart/candlestick/mod.rs:838`, `src/modals/pane/indicator_manager.rs:252,286`, `src/screen/dashboard/pane/lifecycle.rs:68` — `let _ = s.set_parameter(...)` silently discards errors.

**Files affected**:
- `src/chart/candlestick/mod.rs:838`
- `src/modals/pane/indicator_manager.rs:252,286`
- `src/screen/dashboard/pane/lifecycle.rs:68`

**Dependencies**: None
**Estimated effort**: S
**Risk**: Low
**Verification**: Set an invalid study parameter, confirm error appears in logs
**Category**: robustness

---

### P6-009: Add Sidebar Positioning Helper

**Description**: `src/app/mod.rs:640-1014` (or `sidebar_view.rs` after P1-003) — sidebar alignment/padding pattern is repeated 6 times. Extract a helper function.

**Files affected**:
- `src/app/sidebar_view.rs` (after P1-003)

**Dependencies**: P1-003
**Estimated effort**: S
**Risk**: Low
**Verification**: `cargo build`, visual regression on sidebar modals
**Category**: quality

---

---

## Summary Table

### Items Per Phase

| Phase | Description | Items | S | M | L | XL |
|-------|------------|-------|---|---|---|-----|
| **P0** | Critical Fixes (Day 1) | 8 | 6 | 1 | 1 | 0 |
| **P1** | Structural Foundation (Week 1) | 13 | 7 | 4 | 0 | 0 |
| **P2** | Type System & Error Handling (Week 2) | 18 | 11 | 4 | 1 | 1 |
| **P3** | Code Quality & Dead Code (Week 3) | 14 | 9 | 4 | 1 | 0 |
| **P4** | Performance Optimization (Week 4) | 9 | 5 | 3 | 0 | 0 |
| **P5** | Feature Completion (Week 5+) | 9 | 1 | 4 | 2 | 2 |
| **P6** | Documentation & Polish (Ongoing) | 9 | 6 | 3 | 0 | 0 |
| **Total** | | **80** | **45** | **23** | **5** | **3** |

### Effort Estimation Key

| Size | Estimated Time | Description |
|------|---------------|-------------|
| S | 1-2 hours | Single file, mechanical change |
| M | 2-4 hours | Multiple files, straightforward |
| L | 4-8 hours | Cross-crate, significant refactoring |
| XL | 1-2 days | Major structural change, many files |

### Total Effort Estimate

- **S items (45)**: ~67 hours
- **M items (23)**: ~69 hours
- **L items (5)**: ~30 hours
- **XL items (3)**: ~36 hours
- **Grand Total**: ~202 hours (~5 developer-weeks)

### Critical Path

The longest dependency chain determines the minimum calendar time:

```
P0-006 (Price overflow fix)
  -> P2-002 (Unify Price types)  [XL]
    -> P2-003 (tick_size as Price)  [M]
      -> P2-004 (Delete TickerInfo)  [M]
        -> P2-005 (Rename wire types)  [M]
          -> P2-011 (Remove exchange re-exports)  [S]
            -> P5-007 (Reduce data re-exports)  [M]
```

Parallel critical path (independent of Price chain):
```
P1-006 (Extract DataError from lib.rs)
  -> P1-007 (Move platform I/O out)
    -> P1-008 (Move SecretsManager out)
      -> P2-001 (Remove iced_core from data)  [L]
```

Both chains can run in parallel, with the Price unification chain being the bottleneck at ~3-4 days.

### Risk Summary

| Risk Level | Count | Key Items |
|-----------|-------|-----------|
| **High** | 3 | P2-001 (iced_core removal/migration), P2-002 (Price unification), P5-006 (options wiring) |
| **Medium** | 15 | P0-001, P0-002, P3-001, P3-014, P4-004, P4-005, P5-001, P5-005, P5-008, P5-009, etc. |
| **Low** | 62 | Most items — mechanical changes guided by compiler |

### Category Distribution

| Category | Count |
|----------|-------|
| architecture | 16 |
| robustness | 17 |
| quality | 14 |
| performance | 9 |
| consistency | 12 |
| completeness | 12 |
| **Total** | **80** |
