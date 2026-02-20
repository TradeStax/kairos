# Architecture & Structure Audit — Kairos

**Codebase**: Kairos desktop charting platform
**Date**: 2026-02-20
**Total source files**: ~365 Rust files, ~76K LOC (excluding target/)
**Crates analyzed**: kairos (GUI), kairos-data, kairos-exchange, kairos-study, kairos-script

---

## Executive Summary — Top 5 Critical Findings

1. **GUI Framework Leak into Data Layer** — `kairos-data` depends on `iced_core` (a GUI rendering library) and embeds `iced_core::Color` directly into persisted pane state and domain types. This couples the supposedly pure business logic crate to the GUI framework, making it impossible to use `kairos-data` without pulling in an Iced dependency, and makes the domain model serialization format fragile under Iced version changes.

2. **`src/app/mod.rs` is a God Object** — The `Kairos` struct and its `mod.rs` is 1,063 lines and serves as hub for: application state, global `OnceLock` singletons (4 of them), all top-level message variants, the full `view()` function rendering all sidebar modals, and the `new()` constructor with complex service initialization wiring. The `view()` function alone is ~600 lines.

3. **Duplicate / Parallel Type System for Market Data** — The `exchange` crate defines its own `Trade`, `Kline`, `Depth`, `TradeSide`, `OpenInterest`, and `TickerInfo` types in `exchange/src/types.rs`, while `kairos-data` defines `Trade`, `Candle`, `DepthSnapshot`, `Side` in its domain layer. These parallel types require constant mapping and create translation friction throughout the codebase. Each boundary crossing requires manual conversion.

4. **Blocking Async on the UI Thread** — `src/app/services.rs` spawns new `tokio::runtime::Runtime` instances and calls `block_on()` synchronously during `Kairos::new()`. Additionally, `src/app/update/replay.rs` and `src/app/update/chart.rs` call `Handle::current().block_on()` inside the Iced `update()` handler, which runs on the Iced event loop. This risks blocking the entire UI for the duration of those operations.

5. **Legacy `src/chart/study/` Module Duplicates `kairos-study` Crate** — There is a `src/chart/study/` directory containing `poc.rs`, `value_area.rs`, `imbalance.rs`, `volume_profile.rs`, `npoc.rs` annotated with `#[allow(dead_code)]` in its `mod.rs`. These shadow or duplicate logic already present in the dedicated `kairos-study` crate. The module is suppressed but still compiled, creating dead weight and confusion about which implementation is canonical.

---

## 1. Workspace & Crate Boundaries

### Dependency Graph

```
kairos (GUI)
├── kairos-data
├── kairos-exchange
│   └── kairos-data
├── kairos-study
│   └── kairos-data
└── kairos-script
    ├── kairos-data
    └── kairos-study
```

**Finding 1.1 — Dependency Direction is Correct** [SEVERITY: Low]
The high-level dependency flow is architecturally sound: `exchange` depends on `data`, `study` depends on `data`, `script` depends on both `data` and `study`, and the GUI (`kairos`) depends on all of them. There are no circular dependencies in `Cargo.toml`. The `data` crate has no dependency on `exchange`, `study`, or the GUI.

**Finding 1.2 — `kairos-data` Depends on `iced_core`** [SEVERITY: Critical]
Despite the crate being described as "pure business logic, no I/O", it lists `iced_core` as a dependency in `data/Cargo.toml`. The `iced_core` dependency is used in:
- `data/src/config/theme.rs` — the `Theme` wrapper type holds an `iced_core::Theme`
- `data/src/state/pane.rs` — `CandleStyle`, `ComparisonConfig` store `iced_core::Color`
- `data/src/drawing/types.rs` — `SerializableColor` implements `From<iced_core::Color>`

This means persisted application state (`saved-state.json`) is structurally tied to Iced's internal type system. A future upgrade of Iced that changes `Color`'s serialization would silently corrupt state files.

**Recommended fix**: Define a crate-local `Rgba` or `SerializableColor` (without `iced_core` dependency), and perform the conversion at the GUI boundary only.

**Finding 1.3 — `kairos-data` Uses `open` and `dirs-next` Crates** [SEVERITY: Medium]
The `data/Cargo.toml` includes `open` (opens OS file browser) and `dirs-next` (platform data directory resolution). Both are platform I/O concerns, not business logic. The `open_data_folder()` and `data_path()` functions in `data/src/lib.rs` depend on these. These utility functions should live in the GUI crate, not the data crate.

**Finding 1.4 — `kairos-data` Depends on `keyring` and `base64`** [SEVERITY: Medium]
The secrets management module (`data/src/secrets/mod.rs`) is a full 436-line implementation of OS keyring access, file-based API key storage with base64 encoding, and environment variable fallback. While secrets management relates to "what API key to use", the actual OS keyring I/O is infrastructure code that belongs in the application layer or a dedicated `kairos-secrets` crate.

**Finding 1.5 — `kairos-exchange` Package Description is Stale** [SEVERITY: Low]
`exchange/Cargo.toml` still reads: "Databento adapter for futures market data in Kairos" — but the crate now also implements Rithmic and Massive (Polygon) adapters. The description is misleading to new contributors.

---

## 2. Module Organization

### 2.1 kairos (GUI Crate — `src/`)

**Finding 2.1 — `src/app/mod.rs` is a God Module** [SEVERITY: Critical]
At 1,063 lines, `src/app/mod.rs` handles:
- 4 global `OnceLock<Arc<Mutex<>>>` statics and their getter functions
- The full `Kairos` struct definition (25+ fields)
- Three message sub-enums (`ChartMessage`, `OptionsMessage`, `DownloadMessage`)
- The top-level `Message` enum (25+ variants)
- Full `Kairos::new()` constructor with service wiring and state loading (~160 lines)
- All sidebar modal rendering in `view_with_modal()` (~370 lines)
- The `FUTURES_PRODUCTS` lookup table (static data)
- `build_tickers_info()` business logic function

The `view()` function is ~230 lines by itself. `view_with_modal()` is another ~370 lines, all in `mod.rs`.

**Recommended fix**:
- Extract global staging singletons into `src/app/globals.rs`
- Extract all `Message` enum definitions into `src/app/messages.rs`
- Extract `view_with_modal()` into `src/app/sidebar_modals.rs`
- Extract `FUTURES_PRODUCTS` and `build_tickers_info` into `src/app/ticker_registry.rs`
- Keep `mod.rs` as pure re-export + `Kairos` struct definition

**Finding 2.2 — `src/chart/mod.rs` Mixes Chart Logic and Utility Functions** [SEVERITY: Medium]
`src/chart/mod.rs` (499 lines) contains:
- The `Message` enum for chart interactions
- The `Action` enum
- Generic `update<T: Chart>()` handler (~300 lines, handling all 20+ message variants)
- Generic `view<T>()` rendering function
- `draw_volume_bar()` rendering utility

The `update()` function in `mod.rs` is particularly large and handles all chart message variants including both axis interaction and drawing state. The drawing-related messages at the bottom are listed as "handled by the pane/dashboard, not the chart itself" — yet they're dispatched in the same match arm. These could be handled by a separate `drawing_update` function or moved to the pane level entirely.

**Finding 2.3 — `src/chart/study/` is a Dead Module** [SEVERITY: High]
`src/chart/mod.rs` declares:
```rust
#[allow(dead_code)]
pub(crate) mod study;
```
The `src/chart/study/` directory contains 5 files (imbalance, poc, value_area, volume_profile, npoc) implementing the same concepts as the `kairos-study` crate. Only `TradeGroup` and `Footprint` type aliases from this module are actively used (re-exported in `candlestick/mod.rs`). All other study logic here is dead code.

**Recommended fix**: Extract `TradeGroup` and `Footprint` into `src/chart/candlestick/footprint.rs` where they are used, and delete `src/chart/study/`.

**Finding 2.4 — `src/chart/study_renderer/` is also `#[allow(dead_code)]`** [SEVERITY: Medium]
```rust
#[allow(dead_code)]
pub mod study_renderer;
```
The study renderer is intended to convert `StudyOutput` primitives to canvas draw calls, but it is currently not integrated with the active rendering pipeline (as shown by the `dead_code` suppression). This is an incomplete feature that adds module complexity without active functionality.

**Finding 2.5 — Inconsistent Modal Nesting Depth** [SEVERITY: Low]
Modal modules show inconsistent depth:
- `src/modals/download/` — 5 files, flat
- `src/modals/pane/settings/` — has a `mod.rs` + separate settings files
- `src/modals/drawing_properties/mod.rs` — 785-line monolith with no submodules

The `drawing_properties` module is large enough to benefit from splitting into pane-specific drawing type handlers.

### 2.2 kairos-data (`data/src/`)

**Finding 2.6 — `data/src/domain/chart.rs` is 723 Lines and Mixed Responsibility** [SEVERITY: Medium]
This file defines: `ChartConfig`, `ChartBasis`, `ChartType`, `DataGapKind`, `DataGap`, `DataSegment`, `MergeResult`, `ChartData`, `LoadingStatus`, `ViewConfig`, `Autoscale`, `DataSchema`, `KlineDataPoint`, `KlineTrades`, `FootprintMode`, `FootprintType`, `FootprintStudyConfig`, `HeatmapIndicator`, plus a submodule `pub mod heatmap` (CoalesceKind, HeatmapStudy, etc.) and `pub mod comparison`.

This is too many concerns in one file. Chart configuration, chart data structures, chart view state, heatmap-specific types, and comparison-specific types should be in separate files.

**Finding 2.7 — `data/src/lib.rs` has Too-Broad Re-exports** [SEVERITY: Medium]
The `data/src/lib.rs` re-exports 50+ symbols from across all submodules via 23 `pub use` lines. While this provides a convenient flat API, it means any addition to any submodule can silently extend the public surface. Some re-exports are very granular (e.g., `aggregate_trades_to_ticks`, `aggregate_trades_to_candles`) which are implementation details that don't need to be at the top-level namespace.

**Finding 2.8 — `data/src/lib.rs` Defines `DataError` and Utility Functions** [SEVERITY: Low]
The `lib.rs` defines `DataError` enum, `data_path()`, `open_data_folder()`, and `lock_or_recover()` directly. None of these belong in a `lib.rs` — they should be in their respective modules (`error.rs`, `util/mod.rs`).

### 2.3 kairos-exchange (`exchange/src/`)

**Finding 2.9 — `exchange/src/adapter/databento/fetcher.rs` is 1,411 Lines** [SEVERITY: High]
The largest file in the project by a significant margin. `fetcher.rs` contains the `HistoricalDataManager` struct which appears to handle: fetching orchestration, date range gap detection, cache validation, API cost estimation, concurrent download management, and trade data assembly. This is at minimum 4-5 distinct concerns that could be split into: `gap_detector.rs`, `download_manager.rs`, `cost_estimator.rs`, `trade_assembler.rs`.

**Finding 2.10 — `exchange/src/lib.rs` Defines `PushFrequency` Domain Type** [SEVERITY: Medium]
`PushFrequency` (an enum for orderbook update frequency) is defined directly in `exchange/src/lib.rs` rather than in `exchange/src/types.rs` or `exchange/src/adapter/stream.rs` where it's primarily used. Types should not live in `lib.rs` files.

**Finding 2.11 — Duplicate `TickerInfo` Type in Exchange** [SEVERITY: Medium]
`exchange/src/types.rs` defines `TickerInfo` (with fields `ticker`, `min_tick`, `min_qty`), while the domain layer (`data`) defines `FuturesTickerInfo` (with the same semantic meaning). The exchange `TickerInfo` is used for conversion purposes but adds confusion about which type is canonical.

### 2.4 kairos-study (`study/src/`)

**Finding 2.12 — Study Crate Structure is Clean** [SEVERITY: None]
The study crate has the cleanest structure of all crates. Module nesting is at most 2 levels (category/study), all files are reasonably sized (under 600 lines), and the public API is minimal and intentional. The `StudyRegistry` acts as a clean factory.

**Finding 2.13 — `study/src/orderflow/big_trades.rs` is 1,229 Lines** [SEVERITY: Medium]
The `BigTradesStudy` file is disproportionately large compared to other study implementations (next largest is 514 lines). It likely contains rendering configuration, aggregation logic, and visual style settings that could be split into separate concerns.

---

## 3. File Structure Consistency

**Finding 3.1 — Inconsistent `mod.rs` vs Named Files Pattern** [SEVERITY: Low]
Some modules use `module_name/mod.rs` while others use `module_name.rs`. This is standard Rust, but the project shows an inconsistent preference:
- `src/modals/replay/mod.rs` (1,009 lines — should be split)
- `data/src/secrets/mod.rs` (436 lines — the module's only file)
- `src/screen/dashboard/pane/view/` directory with a `mod.rs` (correct pattern)

Large `mod.rs` files (replay, secrets) are a code smell — the pattern suggests the module grew without being subdivided.

**Finding 3.2 — Dead Deleted Files in Git Status** [SEVERITY: Low]
Git status shows deleted files (`D src/chart/indicator/`, `D src/modals/pane/indicators.rs`) that were part of the old indicator system before the study crate was introduced. This confirms the refactoring is partially complete — the old code was removed but the new integration paths (study_renderer) are not yet fully wired.

**Finding 3.3 — Old Branding Remnants** [SEVERITY: Low]
Found references to the old project name "FlowSurface":
- `FLOWSURFACE_DATA_PATH` env var referenced in `data/src/lib.rs:122` and `data/src/state/persistence.rs:299`
- `//! Comprehensive error handling for FlowSurface Exchange layer` in `exchange/src/error.rs:1`

Also found placeholder branding ("XX & Company", "XXNCO") in `src/app/mod.rs:383-422` suggesting client-specific customization is hardcoded rather than configurable.

---

## 4. Dependency Flow — Layer Violations

**Finding 4.1 — GUI Concepts Persist in Data Layer** [SEVERITY: Critical]
As detailed in §1.2: `iced_core::Color` and `iced_core::Theme` are embedded in the data layer's persisted state and domain types. This is the most significant layer violation. The data layer should only know about colors as raw `(f32, f32, f32, f32)` tuples or a locally-defined `Rgba` type.

**Finding 4.2 — `data/src/domain/chart.rs` References `FeedId`** [SEVERITY: Medium]
`DataGapKind::PartialCoverage` contains `available_feeds: Vec<FeedId>` and `DataSegment` contains `feed_id: FeedId`. The `FeedId` type comes from `data::feed::FeedId`. While this is within the same crate, it means the domain model (pure business logic) depends on the feed management model (infrastructure config). Domain types like `DataGap` and `DataSegment` should not reference feed infrastructure types — they should use abstract identifiers or separate the feed-specific information from the pure domain.

**Finding 4.3 — `src/app/services.rs` Creates Tokio Runtimes Synchronously** [SEVERITY: High]
`initialize_market_data_service()` and `initialize_options_services()` create `tokio::runtime::Runtime::new()` and call `block_on()` during application startup inside `Kairos::new()`. Since Iced itself uses Tokio, creating a new runtime here is at minimum wasteful (two runtimes running simultaneously) and at worst causes runtime conflicts. The correct pattern is to use `Task::perform()` with async futures.

**Finding 4.4 — `block_on()` Inside Update Handlers** [SEVERITY: High]
Multiple places in `src/app/update/` call `Handle::current().block_on()`:
- `update/chart.rs:191` — blocks to get rebuild trades during chart data loading
- `update/replay.rs:17,22,37,50,58,63,241,296,360` — blocks repeatedly on replay engine async operations (play, pause, stop, speed, seek, jump, compute histogram)

These calls block the Iced event loop thread. Any operation that takes more than a few milliseconds will cause visible UI stuttering or freezing. All async operations must go through `Task::perform()`.

---

## 5. Public API Surface

**Finding 5.1 — `data/src/lib.rs` Re-exports Too Much** [SEVERITY: Medium]
The crate-level `lib.rs` re-exports 50+ symbols, including internal implementation details like `aggregate_trades_to_candles`, `aggregate_trades_to_ticks`, `merge_segments`, `lock_or_recover`. These are not meaningful public API — they are called by the GUI layer which has better-scoped alternatives. The surface should be reduced to the core types and services that external crates genuinely need.

**Finding 5.2 — `exchange/src/lib.rs` Re-exports Third-Party Types** [SEVERITY: Medium]
`exchange/src/lib.rs` re-exports:
- `pub use databento::dbn::Schema as DatabentoSchema;`
- `pub use rithmic_rs::{self, RithmicEnv};`

This leaks third-party library types into the exchange crate's public API, making the GUI layer directly dependent on `databento` and `rithmic-rs` crate types. If these dependencies change their API, it breaks the GUI. These should be wrapped in exchange-specific types.

**Finding 5.3 — Pervasive `#[allow(dead_code)]` in Components** [SEVERITY: Medium]
The `src/components/` submodules all start with `#![allow(dead_code)]`:
- `src/components/display/mod.rs`
- `src/components/form/mod.rs`
- `src/components/input/mod.rs`
- `src/components/layout/mod.rs`
- `src/components/overlay/mod.rs`
- `src/components/primitives/mod.rs`

This suggests the component library was built speculatively with many functions that are not yet consumed. Dead code in a component library is tolerable, but suppressing the warning crate-wide hides real dead code that could be removed.

**Finding 5.4 — Visibility Modifiers Reasonably Used** [SEVERITY: None]
The codebase uses `pub(crate)` in 200 places and `pub(super)` in some submodules, which is good practice. The main crate (`kairos`) makes most things `pub(crate)` which is appropriate since it's an application binary. The library crates use `pub` more broadly, which is necessary but could be tightened.

---

## 6. Re-export Patterns

**Finding 6.1 — Double-Layer Re-export Confusion** [SEVERITY: Medium]
The `exchange` crate re-exports domain types from `kairos-data`:
```rust
pub use kairos_data::domain::{
    ContractSpec, ContractType, FuturesTicker, FuturesTickerInfo, FuturesVenue, TickerStats, Timeframe,
};
```
This means `FuturesTicker` is available as both `data::FuturesTicker` and `exchange::FuturesTicker`. In `src/app/mod.rs`, the GUI imports from both:
```rust
use data::FeedId;
use exchange::{FuturesTicker, FuturesTickerInfo, FuturesVenue};
```
The same type exists under two paths, creating confusion about which to use. The GUI should consistently import domain types from `data`, not from `exchange`.

**Finding 6.2 — `exchange/src/lib.rs` Re-exports AppError from Data Layer** [SEVERITY: Low]
```rust
pub use kairos_data::domain::error::{AppError, ErrorSeverity};
```
The exchange crate re-exports the error traits from the data layer. While not incorrect, it creates an additional path to these types. Consumers of `exchange` may not realize `AppError` is actually from `kairos_data`.

---

## 7. Feature Flags & Conditional Compilation

**Finding 7.1 — The `debug` Feature is Minimal** [SEVERITY: Low]
`Cargo.toml` defines:
```toml
[features]
debug = ["iced/hot"]
```
This enables Iced's hot-reload feature. There is no other conditional compilation in the source files based on this feature flag (no `#[cfg(feature = "debug")]` in source). Platform-specific code uses `#[cfg(target_os = "macos")]` / `#[cfg(not(target_os = "macos"))]` throughout `mod.rs` and window rendering.

**Finding 7.2 — Platform Conditionals are Inline, Not Abstracted** [SEVERITY: Low]
macOS vs. non-macOS rendering differences appear inline in `src/app/mod.rs:419-443` and `src/app/mod.rs:586-597`. These could be extracted into platform-specific view functions for cleaner code.

---

## 8. Large File Summary

| File | Lines | Issues |
|------|-------|--------|
| `exchange/src/adapter/databento/fetcher.rs` | 1,411 | Too many responsibilities |
| `src/screen/dashboard/panel/ladder.rs` | 1,283 | Monolith panel |
| `study/src/orderflow/big_trades.rs` | 1,229 | Oversized |
| `src/modals/pane/indicator_manager.rs` | 1,114 | UI logic mixed with state |
| `src/app/mod.rs` | 1,063 | God object |
| `src/modals/replay/mod.rs` | 1,009 | Single-file monolith |
| `src/chart/candlestick/mod.rs` | 869 | KlineChart state + rendering |
| `data/src/services/replay_engine.rs` | 930 | Large service |
| `data/src/domain/gex.rs` | 857 | GEX domain is complex |
| `data/src/services/market_data.rs` | 747 | Service orchestration |
| `data/src/domain/chart.rs` | 723 | Mixed chart domain types |

---

## 9. Proposed Ideal Module Structure

### kairos (GUI crate)

```
src/
├── main.rs
├── error.rs
├── window.rs
├── layout.rs
├── logger.rs
├── app/
│   ├── mod.rs              # Kairos struct + new() + title/theme/scale_factor
│   ├── globals.rs          # OnceLock statics and getters (EXTRACT FROM mod.rs)
│   ├── messages.rs         # Message, ChartMessage, DownloadMessage enums (EXTRACT)
│   ├── services.rs         # Service initialization functions
│   ├── subscriptions.rs    # Subscription building
│   ├── sidebar_view.rs     # view_with_modal() and all sidebar modal rendering (EXTRACT)
│   ├── ticker_registry.rs  # FUTURES_PRODUCTS, build_tickers_info (EXTRACT)
│   └── update/
│       ├── mod.rs          # Kairos::update() dispatch
│       ├── chart.rs
│       ├── download.rs
│       ├── feeds.rs
│       ├── navigation.rs
│       ├── options.rs
│       ├── preferences.rs
│       └── replay.rs
├── chart/
│   ├── mod.rs              # Re-exports + draw_volume_bar utility
│   ├── messages.rs         # Message + Action enums (EXTRACT FROM mod.rs)
│   ├── update.rs           # Generic update<T: Chart>() (EXTRACT FROM mod.rs)
│   ├── view.rs             # Generic view<T>() (EXTRACT FROM mod.rs)
│   ├── candlestick/
│   ├── comparison/
│   ├── core/
│   ├── drawing/
│   ├── heatmap/
│   ├── overlay/
│   ├── perf/
│   ├── scale/
│   └── study_renderer/     # Keep when wired; remove dead_code suppression
│   # REMOVE: src/chart/study/ — dead code, TradeGroup moves to candlestick/
├── components/
├── modals/
├── screen/
└── style/
```

### kairos-data (data crate)

```
data/src/
├── lib.rs                  # Thin re-exports only — reduce to ~20 lines
├── domain/
│   ├── mod.rs
│   ├── error.rs
│   ├── types.rs            # Price, Volume, Timestamp, Side, DateRange, etc.
│   ├── entities.rs         # Trade, Candle, DepthSnapshot
│   ├── aggregation.rs
│   ├── chart/
│   │   ├── mod.rs          # ChartConfig, ChartData, ChartBasis, ChartType
│   │   ├── config.rs       # ChartConfig, ChartBasis, ChartType (SPLIT FROM chart.rs)
│   │   ├── data.rs         # ChartData, DataSegment, DataGap, MergeResult (SPLIT)
│   │   ├── view.rs         # ViewConfig, Autoscale, LoadingStatus (SPLIT)
│   │   ├── kline.rs        # KlineDataPoint, KlineTrades, FootprintMode/Type (SPLIT)
│   │   └── heatmap.rs      # HeatmapIndicator, CoalesceKind, HeatmapStudy (SPLIT)
│   ├── futures.rs
│   ├── gex.rs
│   ├── options.rs
│   └── panel/
├── repository/
├── services/
├── state/
├── config/
│   ├── mod.rs
│   ├── panel.rs
│   ├── sidebar.rs
│   ├── timezone.rs
│   # REMOVE iced_core dependency — replace Theme(iced_core::Theme) with ThemeConfig(String, Rgba)
├── feed/
├── drawing/
│   # Define local Rgba type, remove iced_core::Color From impls
├── secrets/
│   # Consider moving to app layer or kairos-secrets crate
└── util/
    ├── mod.rs
    ├── logging.rs
    ├── formatting.rs
    └── math.rs
```

### kairos-exchange (exchange crate)

```
exchange/src/
├── lib.rs                  # Thin re-exports; NO type definitions here; remove PushFrequency
├── error.rs
├── types.rs                # Exchange-specific types; consolidate/remove TickerInfo duplicate
├── util.rs                 # Power10 types
├── adapter/
│   ├── mod.rs
│   ├── event.rs
│   ├── stream.rs           # Move PushFrequency here from lib.rs
│   ├── error.rs
│   ├── databento/
│   │   ├── mod.rs
│   │   ├── cache.rs
│   │   ├── client.rs
│   │   ├── decoder.rs
│   │   ├── mapper.rs
│   │   └── fetcher/        # SPLIT fetcher.rs (1,411 lines) into:
│   │       ├── mod.rs      #   re-exports
│   │       ├── manager.rs  #   HistoricalDataManager struct + lifecycle
│   │       ├── gaps.rs     #   date range gap detection
│   │       ├── cost.rs     #   cost estimation
│   │       └── download.rs #   concurrent download orchestration
│   ├── massive/
│   └── rithmic/
└── repository/
```

---

## 10. Summary of Recommended Changes by Priority

### Critical (Immediate Action)
1. Remove `iced_core` from `kairos-data` — define local `Rgba(f32,f32,f32,f32)` in `data/src/drawing/types.rs` and update `CandleStyle`, `ComparisonConfig`, `Theme` to use it
2. Replace all `block_on()` in `update/replay.rs` and `update/chart.rs` with `Task::perform()` async operations
3. Break up `src/app/mod.rs` — extract globals, messages, and sidebar view into separate files

### High Priority
4. Delete `src/chart/study/` — move `TradeGroup`/`Footprint` to `src/chart/candlestick/footprint.rs`
5. Split `exchange/src/adapter/databento/fetcher.rs` (1,411 lines) into 4+ focused files
6. Fix the two `tokio::runtime::Runtime::new().block_on()` calls in `services.rs` — use `Task::future()` during startup

### Medium Priority
7. Stop re-exporting third-party types (`databento::dbn::Schema`, `rithmic_rs::RithmicEnv`) from `exchange/src/lib.rs`
8. Stop re-exporting domain types from `exchange` — GUI should import them from `data` only
9. Split `data/src/domain/chart.rs` (723 lines) into focused submodules
10. Reduce `data/src/lib.rs` re-export surface
11. Move `PushFrequency` out of `exchange/src/lib.rs` into `exchange/src/adapter/stream.rs`

### Low Priority
12. Update `exchange/Cargo.toml` description to reflect all three adapters
13. Rename `FLOWSURFACE_DATA_PATH` env var to `KAIROS_DATA_PATH` (and update docs)
14. Move `DataError`, `data_path()`, `open_data_folder()`, `lock_or_recover()` out of `lib.rs` into appropriate submodules
15. Address dead study_renderer by completing integration or removing until ready
