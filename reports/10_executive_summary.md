# Executive Summary — Kairos Codebase Review

**Date**: 2026-02-20
**Scope**: ~76K LOC, 328 .rs files, 5 workspace crates
**Reports**: 10 documents produced by 8 specialized agents across 4 stages
**Total findings**: 101 (7 Critical, 23 High, 37 Medium, 34 Low)
**Implementation plan**: 80 items across 7 phases, ~202 hours (~5 dev-weeks)

---

## Project Health Assessment

| Concern Area | Grade | Rationale |
|-------------|-------|-----------|
| **Architecture** | B- | Clean 4-crate dependency graph (no cycles), but `kairos-data` violates its own "pure business logic" contract by depending on `iced_core`, `keyring`, and platform I/O. The exchange layer leaks Databento-specific methods into domain traits. |
| **Code Quality** | C+ | Several god objects (1,063-line `mod.rs`, 1,411-line `fetcher.rs`), 20+ sites with `#[allow(dead_code)]` masking real dead code, FUTURES_PRODUCTS defined 3x, placeholder company name in production. The study and data crates are cleaner than the GUI crate. |
| **Robustness** | C | 2 Critical crash paths (`active_dashboard().expect()` on every UI tick, `layouts.first().unwrap()`), `Price` arithmetic panics on overflow in rendering paths, non-atomic state persistence, mixed mutex poison handling. The app cannot recover from a corrupt state file. |
| **Performance** | B | Solid 5-layer cache architecture, good LOD system, efficient QtyScale caching. But: `block_on()` in replay freezes UI (Critical), heatmap has duplicate per-frame computation, volume profile rebuilds every frame, 6 O(N) scans should be O(log N). |
| **Consistency** | C+ | Dual Price types (the #1 cross-cutting issue), parallel Trade/Candle/Depth types across crates, `f32`/`f64` price leakage in study outputs, `Side` enum conflates trade and book semantics, `Result<T, String>` in all message enums erases structured error info. |
| **Completeness** | B- | Core charting features work well. But: options pipeline is built and dead, link-group switching is a stub, replay skip controls unimplemented, JS scripting has no UI, `kairos` GUI crate has ~11 tests total. |
| **Documentation** | D+ | `CLAUDE.md` is excellent but partially outdated (references deleted modules). Root `README.md` is empty. Script crate has no docs. Public APIs lack doc comments. |

**Overall Grade: C+** — A capable application with solid domain design in the data/study layers, but the GUI crate has accumulated significant technical debt, the type system is fragmented across crate boundaries, and critical crash paths exist that can make the app unlaunchable.

---

## Top 20 Action Items (Priority Order)

| # | ID | Title | Effort | Risk | Phase |
|---|-----|-------|--------|------|-------|
| 1 | P0-001 | Fix `active_dashboard().expect()` — app crashes on corrupt state | M | Med | Day 1 |
| 2 | P0-002 | Replace `block_on()` in replay with `Task::perform` — UI freezes | L | Med | Day 1 |
| 3 | P0-004 | Fix `layouts.first().unwrap()` in download handler | S | Low | Day 1 |
| 4 | P0-006 | Replace `Price +/-` panic with saturating arithmetic | S | Low | Day 1 |
| 5 | P0-005 | Atomic state persistence (write-temp-rename) | S | Low | Day 1 |
| 6 | P0-007 | Replace `from_utf8().unwrap()` with lossy conversion | S | Low | Day 1 |
| 7 | P0-008 | Change production `assert!` to `debug_assert!` in aggregation | S | Low | Day 1 |
| 8 | P2-001 | Remove `iced_core` from `kairos-data` (define local Rgba) | L | High | Wk 2 |
| 9 | P2-002 | Unify dual `Price` types (data::Price canonical) | XL | High | Wk 2 |
| 10 | P1-001 | Extract globals from `src/app/mod.rs` | S | Low | Wk 1 |
| 11 | P1-002 | Extract Message enums from `src/app/mod.rs` | S | Low | Wk 1 |
| 12 | P1-003 | Extract sidebar view from `src/app/mod.rs` | M | Low | Wk 1 |
| 13 | P4-001 | Heatmap: compute `visible_data_count` once per frame | S | Low | Wk 4 |
| 14 | P4-002 | Candlestick: binary search for visible candle count | S | Low | Wk 4 |
| 15 | P4-004 | Precompute volume profile outside draw closure | M | Med | Wk 4 |
| 16 | P3-014 | Extract Databento methods from TradeRepository trait | M | Med | Wk 3 |
| 17 | P2-004 | Delete duplicate `TickerInfo`, use `FuturesTickerInfo` only | M | Med | Wk 2 |
| 18 | P5-001 | Implement link group ticker switching | M | Med | Wk 5 |
| 19 | P1-009 | Delete dead `src/chart/study/` module | S | Low | Wk 1 |
| 20 | P5-004 | Add unit tests for GUI crate message handlers | XL | Low | Wk 5+ |

---

## Quick Wins (High Impact, Low Effort)

These items can be completed in 1-2 hours each and have immediate, measurable impact:

| ID | Title | Impact | Effort |
|----|-------|--------|--------|
| P0-004 | Fix `layouts.first().unwrap()` | Prevents crash | S |
| P0-006 | Price saturating arithmetic | Prevents crash | S |
| P0-005 | Atomic state persistence | Prevents data corruption | S |
| P0-007 | `from_utf8_lossy` for exchange symbols | Prevents crash | S |
| P0-008 | `assert!` → `debug_assert!` in aggregation | Prevents crash | S |
| P4-001 | Compute `visible_data_count` once | ~10-50ms/frame saved | S |
| P4-002 | Binary search for candle count | O(N)→O(log N) | S |
| P4-003 | Binary search for crosshair lookup | O(N)→O(log N) per mouse move | S |
| P4-006 | Binary search in comparison chart | O(N)→O(log N) | S |
| P1-009 | Delete dead `src/chart/study/` | Removes confusion | S |
| P6-007 | Log dropped replay events | Debugging aid | S |

**Total quick wins: 11 items, ~16 hours, addresses 5 crash paths + 4 perf wins.**

---

## Estimated Total Refactoring Scope

| Metric | Value |
|--------|-------|
| Total items | 80 |
| Estimated hours | ~202 |
| Developer-weeks | ~5 |
| Critical path | ~3-4 days (Price unification chain) |
| Files touched | ~120 (of 328 total .rs files) |
| New files created | ~10 (extracted modules + extension traits) |
| Files deleted | ~6 (dead study module, old TickerInfo shim) |
| Crates modified | All 5 |
| State migration needed | Yes (v2→v3 for Rgba replacement of iced_core::Color) |

### Effort by Phase

| Phase | Items | Hours | Calendar |
|-------|-------|-------|----------|
| P0: Critical Fixes | 8 | ~12 | Day 1 |
| P1: Structure | 13 | ~25 | Week 1 |
| P2: Type System | 18 | ~44 | Week 2 |
| P3: Quality | 14 | ~28 | Week 3 |
| P4: Performance | 9 | ~18 | Week 4 |
| P5: Features | 9 | ~47 | Week 5+ |
| P6: Polish | 9 | ~18 | Ongoing |

---

## Risk Areas Requiring Careful Handling

### 1. State File Migration (P2-001)
Removing `iced_core::Color` from `kairos-data` requires a state migration (v2→v3). Existing `app-state.json` files contain serialized `iced_core::Color { r, g, b, a }` values. The migration must:
- Parse old format correctly
- Convert to new `Rgba(f32, f32, f32, f32)` format
- Handle partial/corrupt files gracefully
- Be tested with real user state files

### 2. Price Type Unification (P2-002)
This is the single largest change, touching ~50 files across all crates. It must be done in a single coordinated PR to avoid intermediate compilation failures. The risk is mitigated by the compiler — every usage site that needs updating will produce a type error.

### 3. Replay `block_on` → `Task::perform` (P0-002)
This changes the concurrency model for replay operations. Subtle timing differences may emerge. The replay system should be tested across all operations (play, pause, stop, seek, speed change) with both ES and NQ data before and after.

### 4. `active_dashboard` Return Type Change (P0-001)
Changing from `&Dashboard` to `Option<&Dashboard>` affects every message handler. While mechanical, the sheer number of call sites (~20+) means careful review is needed.

---

## Recommended Approach for Implementation

### Solo Developer Path (~5 weeks)
1. **Day 1**: Land all P0 items (critical fixes) as individual commits
2. **Week 1**: P1 structural extractions — one PR per extraction
3. **Week 2**: P2 type system — Price unification as one large PR, the rest as smaller PRs
4. **Week 3-4**: P3 quality + P4 performance — can be interleaved
5. **Week 5+**: P5 features + P6 polish — prioritize by user impact

### Two-Developer Path (~3 weeks)
- **Dev A**: P0 → P2 (crash fixes, type system — serialized due to dependencies)
- **Dev B**: P1 → P3 → P4 (structure, quality, performance — largely independent)
- **Both**: P5, P6 (features, polish — parallelizable)

### Recommended PR Strategy
- P0 items: Individual PRs, merge same day
- P1 items: 3-4 PRs (globals extraction, sidebar extraction, dead code deletion, options feature-gate)
- P2 items: 2-3 PRs (Price unification, TickerInfo cleanup, error handling)
- P3-P6: One PR per item or group of related items

---

## Key Metrics to Track Post-Refactoring

| Metric | Current | Target |
|--------|---------|--------|
| `unwrap()` in production code (non-test) | ~40 | <10 |
| `#[allow(dead_code)]` sites | 20+ | 0 |
| Files >1,000 lines | 6 | 0 |
| Functions >50 lines | ~15 | <5 |
| Test count in `kairos` (GUI) crate | 11 | 50+ |
| `block_on()` calls in update handlers | 12 | 0 |
| Duplicate type definitions cross-crate | 4 | 0 |
| FUTURES_PRODUCTS definitions | 3 | 1 |
