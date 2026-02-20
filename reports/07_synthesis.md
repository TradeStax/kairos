# Cross-Cutting Synthesis Report

**Date**: 2026-02-20
**Author**: Team Lead — synthesized from Reports 01–06
**Reviewers**: architecture, quality, robustness, performance, consistency, completeness

---

## 1. Cross-Cutting Themes

Six independent reviewers converged on the same core issues from different angles. The following themes appeared in 3+ reports:

### Theme A: Dual/Parallel Type System (Reports 01, 02, 05)

The single most pervasive architectural issue. Three independent reviewers identified this:

- **Architecture (01)**: "Duplicate / Parallel Type System for Market Data" — `exchange::Trade` vs `data::Trade`, `exchange::Kline` vs `data::Candle`, constant mapping at boundaries.
- **Quality (02)**: "`TickerInfo` and `FuturesTickerInfo` are near-duplicate structs with manual bidirectional `From` impls" — tick_size field even differs in type (`PriceStep` vs `f32`).
- **Consistency (05)**: "Dual Price types" as the #1 finding — two separate `Price` structs at same precision. Also: `exchange::Trade` uses `f32` prices while `data::Trade` uses `Price` newtypes.

**Impact**: Every data layer boundary requires manual conversion. Type safety is weakened. Bugs can silently introduce precision loss (f32 → Price → f32 round-trips). The inconsistency propagates into study outputs where `f64` prices appear.

**Root Cause**: The exchange crate was designed to work with raw wire-format types, and the conversion to domain types was never unified into a single canonical boundary.

### Theme B: `block_on()` / Blocking Async on UI Thread (Reports 01, 03, 04)

All three reviewers who touched async code flagged this:

- **Architecture (01)**: "Blocking Async on the UI Thread" — `services.rs` spawns separate Tokio runtimes; `replay.rs` calls `Handle::current().block_on()` inside Iced's `update()`.
- **Robustness (03)**: 9 `block_on` calls in `replay.rs` identified as High risk crash/deadlock paths.
- **Performance (04)**: Ranked as the #1 Critical bottleneck — "`block_on()` inside `spawn_blocking` closures can freeze the UI thread and starve the Tokio runtime."

**Impact**: UI freezes during replay operations (play/pause/stop/seek). Potential deadlock under thread pool exhaustion. Two separate Tokio runtimes running simultaneously waste resources.

### Theme C: `src/app/mod.rs` God Object (Reports 01, 02)

- **Architecture (01)**: 1,063-line god module with 25+ fields, 4 global singletons, 600-line view function.
- **Quality (02)**: `view_with_modal` is 375 lines with deeply nested match arms, duplicated sidebar positioning, duplicated confirm-dialog blocks.

**Impact**: Difficulty onboarding, high merge conflict probability, mixing of concerns (state, rendering, service wiring, product data).

### Theme D: Dead Code Suppressed With `#[allow(dead_code)]` (Reports 01, 02, 06)

- **Architecture (01)**: `src/chart/study/` dead module, `study_renderer` suppressed.
- **Quality (02)**: 6 component modules with `#![allow(dead_code)]`, options pipeline dead, `draw_drawings` dead.
- **Completeness (06)**: Full inventory of 20+ `#[allow(dead_code)]` sites; options pipeline as the #1 gap.

**Impact**: Compiler cannot detect genuine dead code. Unknown quantity of unused code inflates binary size. New contributors cannot distinguish live from dead code.

### Theme E: `active_dashboard().expect()` Crash Risk (Reports 03, 06)

- **Robustness (03)**: #1 Critical crash risk — `.expect("No active layout")` called on every UI update. Corrupt state = crash on launch.
- **Completeness (06)**: Non-atomic state persistence means a power loss during save can corrupt state, triggering this exact panic path.

**Impact**: A single corrupt `app-state.json` file makes the application permanently unlaunchable without manual intervention.

### Theme F: Options Pipeline Dead End (Reports 02, 06)

- **Quality (02)**: "Unfinished options data pipeline silenced with `#[allow(dead_code)]`".
- **Completeness (06)**: #1 gap — "Zero user-visible options functionality despite full exchange layer".

**Impact**: The exchange layer (Massive/Polygon), data service layer (`OptionsDataService`, `GexCalculationService`), and repository implementations are fully built but completely unwired. Initialization runs on every startup, wasting resources.

### Theme G: Inconsistent Error Handling (Reports 03, 05)

- **Robustness (03)**: Mixed mutex poison recovery strategies; `InternalError` lacks `AppError` trait; `AdapterError` and `exchange::Error` not connected.
- **Consistency (05)**: `Result<T, String>` in all message enums erases structured error info; `UserFacingError` trait referenced in CLAUDE.md doesn't exist in code.

**Impact**: Errors lose context as they propagate through the system. Users see generic error strings without actionable recovery information.

---

## 2. Conflict Resolution

### Conflict 2.1: Should `exchange::types::Trade` be eliminated or kept?

- **Architecture (01)** implies merging into a single type.
- **Consistency (05)** suggests keeping exchange-specific types for wire-format deserialization but unifying the conversion boundary.

**Resolution**: Keep `exchange::types::Trade` as a wire-format struct (renamed to `RawTrade` or `WireTrade` for clarity), but enforce that conversion to `data::Trade` happens exactly once at the adapter boundary via a single `impl From<WireTrade> for Trade`. The same applies to `Kline`→`Candle` and `Depth`→`DepthSnapshot`.

### Conflict 2.2: Should `Price` arithmetic panic or saturate on overflow?

- **Robustness (03)**: Recommends `saturating_add/sub` or removing `Add/Sub` impl.
- **Performance (04)**: Did not flag Price arithmetic as a performance concern (it's branching, not hot-path allocation).

**Resolution**: Replace `expect()` in `Add`/`Sub` with `saturating_add`/`saturating_sub` for production. Add `checked_add`/`checked_sub` methods for explicit handling when overflow detection is needed (e.g., validation). The `expect` in hot rendering paths is unacceptable — a malformed price from exchange data would crash the entire application.

### Conflict 2.3: Should the options pipeline be completed or removed?

- **Completeness (06)**: Lists as P0 — either wire it or remove it.
- **Quality (02)**: Notes the `_gex_service` is initialized and immediately discarded.

**Resolution**: This is a product decision, not a code quality decision. Recommend: (a) immediately stop initializing services when the API key is not set, (b) feature-gate with `#[cfg(feature = "options")]`, (c) defer full wiring to a dedicated sprint. Mark as P1, not P0, since it wastes startup time but doesn't crash anything.

### Conflict 2.4: `Side` enum — 4 variants vs split into 2 enums?

- **Consistency (05)**: Recommends splitting into `TradeSide { Buy, Sell }` and `BookSide { Bid, Ask }`.
- No other reviewer raised this.

**Resolution**: Agree with splitting. The current `Side::Buy == Side::Bid` equivalence (both map to index 0) is a semantic ambiguity that can cause subtle bugs. This is a medium-priority refactor with moderate blast radius (touches domain, exchange, and rendering code).

---

## 3. Dependency Mapping

The following dependency chain determines the order of fixes:

```
Level 0 (Foundation — must happen first):
├── Remove iced_core from kairos-data (Theme A prerequisite)
├── Fix active_dashboard().expect() crash path (Theme E)
└── Replace block_on() with Task::perform in replay.rs (Theme B)

Level 1 (Type System — enables Level 2):
├── Unify Price types (data::Price as canonical)
│   ├── Merge checked arithmetic from exchange::Price
│   ├── Change FuturesTickerInfo.tick_size from f32 to Price
│   └── Update study output types (f64 → Price or f32)
├── Unify Trade/Candle/Depth types or enforce single conversion boundary
└── Split Side into TradeSide + BookSide

Level 2 (Architecture — enabled by Level 1):
├── Break up src/app/mod.rs (extract globals, messages, sidebar view)
├── Split exchange/src/adapter/databento/fetcher.rs
├── Extract Databento methods from TradeRepository trait
├── Delete src/chart/study/ dead module
└── Remove #[allow(dead_code)] suppressions, fix actual dead code

Level 3 (Error Handling — can parallel Level 2):
├── Implement AppError on InternalError
├── Add From<AdapterError> for exchange::Error
├── Standardize mutex poison recovery
├── Make state persistence atomic (write-temp-rename)
└── Replace unwrap() calls in production paths

Level 4 (Performance — after structural changes):
├── Fix heatmap duplicate visible_data_count
├── O(N) → O(log N) in candlestick render (binary search)
├── Precompute volume profile outside draw closure
├── Replace Vec linear scan in add_trade with BTreeMap
└── Sort validation → debug_assert only

Level 5 (Feature Completion — after stability):
├── Wire options pipeline or feature-gate it
├── Implement link group ticker switching
├── Implement replay JumpForward/JumpBackward
├── Implement FocusWidget effect
└── Add test coverage for src/ crate

Level 6 (Polish):
├── Consolidate FUTURES_PRODUCTS into single definition
├── Replace "XX & Company" placeholder
├── Update CLAUDE.md to reflect current structure
├── Populate root README.md
└── Document JS scripting system
```

---

## 4. Risk Assessment

### What breaks if we change X?

| Change | Risk | Mitigation |
|--------|------|------------|
| Remove `iced_core` from `kairos-data` | High — breaks serialization of existing `app-state.json` files using `iced_core::Color` | Write a migration (v2→v3) that converts `Color { r, g, b, a }` to `Rgba(f32,f32,f32,f32)` before upgrading |
| Unify Price types | High — touches 50+ files across all crates | Do in a single PR with global find-replace; run full test suite |
| Break up `src/app/mod.rs` | Medium — many imports reference `app::Message` | Extract in stages: globals first, then messages, then sidebar view. Each stage is a separately testable PR |
| Delete `src/chart/study/` | Low — only `TradeGroup` alias is used | Move alias to candlestick module first, verify compilation, then delete |
| Replace `block_on` in replay | Medium — replay behavior may change subtly | Test all replay operations (play/pause/stop/seek/speed) manually before and after |
| Atomic state writes | Low — transparent change, no API surface impact | Test by killing process during save; verify state recovery |
| Remove `#[allow(dead_code)]` | Low — only reveals warnings, doesn't change behavior | Fix warnings incrementally; start with component modules |
| Split `Side` into `TradeSide`/`BookSide` | Medium — used in domain, exchange, chart rendering | Requires coordinated update; use compiler errors as guide |

---

## 5. Top 20 Highest-Impact Items (Ranked)

Priority: (a) crash risk, (b) correctness, (c) maintainability, (d) performance

| Rank | ID | Title | Source | Severity | Category |
|------|-----|-------|--------|----------|----------|
| 1 | CR-01 | `active_dashboard().expect()` panics on corrupt state | 03 | Critical | Crash |
| 2 | CR-02 | `block_on()` in replay.rs freezes UI / potential deadlock | 01,03,04 | Critical | Crash+Perf |
| 3 | CR-03 | `layouts.first().unwrap()` in download handler panics on empty layouts | 03 | Critical | Crash |
| 4 | CR-04 | `Price + Price` operator panics on overflow in rendering paths | 03 | High | Crash |
| 5 | CR-05 | `from_utf8().unwrap()` on exchange symbol bytes panics on non-UTF-8 | 03 | High | Crash |
| 6 | CR-06 | `assert!(!trades.is_empty())` in production aggregation | 03 | High | Crash |
| 7 | CR-07 | Non-atomic state persistence — power loss corrupts app-state.json | 03 | High | Correctness |
| 8 | CR-08 | `iced_core` dependency in `kairos-data` couples domain to GUI framework | 01,05 | Critical | Architecture |
| 9 | CR-09 | Dual Price type system — `data::Price` vs `exchange::Price` | 01,05 | High | Correctness |
| 10 | CR-10 | `FuturesTickerInfo.tick_size` is `f32` instead of `Price` | 05 | Medium | Correctness |
| 11 | CR-11 | Replay events silently dropped on full channel | 03 | High | Correctness |
| 12 | CR-12 | `src/app/mod.rs` god object — 1,063 lines, 25+ fields | 01,02 | High | Maintainability |
| 13 | CR-13 | `databento/fetcher.rs` — 1,411-line monolith | 01 | High | Maintainability |
| 14 | CR-14 | TradeRepository trait leaks Databento-specific methods | 05 | High | Architecture |
| 15 | CR-15 | Heatmap duplicate `visible_data_count` computation per frame | 04 | High | Performance |
| 16 | CR-16 | Volume profile rebuilt inside draw closure every frame | 04 | High | Performance |
| 17 | CR-17 | O(N) linear scan in `add_trade()` for heatmap grouping | 04 | High | Performance |
| 18 | CR-18 | `#[allow(dead_code)]` on 6 component modules + options pipeline | 02,06 | High | Maintainability |
| 19 | CR-19 | `kairos` (src/) crate has ~11 tests total — near-zero coverage | 06 | High | Correctness |
| 20 | CR-20 | FUTURES_PRODUCTS defined in 3 places with divergent data | 02 | Medium | Correctness |

---

## 6. Reviewer Agreement Matrix

| Issue | 01 | 02 | 03 | 04 | 05 | 06 | Count |
|-------|----|----|----|----|----|----|-------|
| Dual type system (Price/Trade/TickerInfo) | X | X | | | X | X | 4 |
| block_on() in UI thread | X | | X | X | | | 3 |
| God object mod.rs | X | X | | | | | 2 |
| Dead code suppression | X | X | | | | X | 3 |
| active_dashboard crash | | | X | | | X | 2 |
| Options pipeline dead | | X | | | | X | 2 |
| Error handling inconsistency | | | X | | X | | 2 |
| FUTURES_PRODUCTS duplication | | X | | | | | 1 |
| Heatmap perf bottlenecks | | | | X | | | 1 |
| Test coverage gaps | | | | | | X | 1 |

---

## 7. Categorized Finding Counts

| Category | Critical | High | Medium | Low | Total |
|----------|----------|------|--------|-----|-------|
| Architecture | 2 | 3 | 5 | 4 | 14 |
| Code Quality | 0 | 3 | 7 | 6 | 16 |
| Robustness | 2 | 8 | 9 | 5 | 24 |
| Performance | 1 | 3 | 5 | 5 | 14 |
| Consistency | 0 | 4 | 8 | 10 | 22 |
| Completeness | 2 | 2 | 3 | 4 | 11 |
| **Total** | **7** | **23** | **37** | **34** | **101** |
