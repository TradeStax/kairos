# Code Quality & Smells Audit — Kairos

**Date:** 2026-02-20
**Codebase:** ~76K LOC (365 Rust source files across 4 workspace crates)
**Auditor:** Code Quality Agent (Task #2)

---

## Executive Summary — Top 10 Worst Offenders

| Rank | Issue | File(s) | Severity |
|------|-------|---------|---------|
| 1 | FUTURES_PRODUCTS lookup table duplicated in 3+ places with different shapes | `src/app/mod.rs:1018`, `src/modals/download/mod.rs:16`, `exchange/src/adapter/databento/mapper.rs:76` | HIGH |
| 2 | Broad `#![allow(dead_code)]` silencing entire component modules | `src/components/{display,form,input,layout,overlay,primitives}/mod.rs` (6 files) | HIGH |
| 3 | Unfinished options data pipeline silenced with `#[allow(dead_code)]` instead of being feature-gated or removed | `src/app/mod.rs:122,191` | HIGH |
| 4 | Panic-in-test anti-pattern: `panic!("Expected Markers, got {:?}", other)` used as assertions in 11+ test cases | `study/src/orderflow/big_trades.rs:689–1079` | HIGH |
| 5 | `dashboard_modal` function defined twice with divergent signatures and semantics | `src/modals/mod.rs:78` vs `src/components/overlay/modal_shell.rs:159` | MEDIUM |
| 6 | Magic number `9.0` for chart label text size repeated 3× in same file | `src/chart/heatmap/render.rs:397,545,723` | MEDIUM |
| 7 | `view_with_modal` in `src/app/mod.rs` is 375 lines with deeply nested match arms | `src/app/mod.rs:640–1014` | MEDIUM |
| 8 | Placeholder company name strings ("XX & Company", "XXNCO") hardcoded in production UI | `src/app/mod.rs:383–422` | MEDIUM |
| 9 | `TickerInfo` and `FuturesTickerInfo` are near-duplicate structs with manual bidirectional `From` impls | `exchange/src/types.rs:150`, `data/src/domain/futures.rs:403` | MEDIUM |
| 10 | `draw_volume_bar()` takes 11 parameters — far exceeds the clippy limit of 5 | `src/chart/mod.rs:431` | MEDIUM |

---

## Metrics

| Metric | Value |
|--------|-------|
| Total Rust source files | 365 |
| Total LOC (project) | ~76K (excluding build artifacts) |
| Estimated dead/suppressed LOC | ~600–900 (options pipeline, drawing_drawings, etc.) |
| `#[allow(dead_code)]` instances | 18 in `src/`, 0 in `data/`, 0 in `exchange/` |
| `#![allow(dead_code)]` module-level | 6 full component sub-modules |
| `#![allow(unused_imports)]` module-level | 1 (`src/components/mod.rs`) |
| `#[allow(clippy::too_many_arguments)]` | 10 in `src/` |
| `.clone()` calls | 257 in `src/`, 21 in `data/`, 18 in `exchange/` |
| `.unwrap()` calls | 49 in `src/`, 79 in `data/`, 78 in `exchange/` |
| `panic!()` in non-test code | 0 |
| `panic!()` in test code | 11+ (`study/src/orderflow/big_trades.rs`) |
| TODO/FIXME comments | 3 |
| Test functions | ~250 across all crates |
| Doc comment lines (`///`/`//!`) in `src/` | 1,467 |
| Public functions in `src/` | 770 |
| Duplication percentage (FUTURES_PRODUCTS) | 3 separate definitions |

---

## 1. Dead Code

### 1.1 Entire Component Modules Suppressed
**[HIGH]** `src/components/display/mod.rs:1`, `src/components/form/mod.rs:1`, `src/components/input/mod.rs:1`, `src/components/layout/mod.rs:1`, `src/components/overlay/mod.rs:1`, `src/components/primitives/mod.rs:1`

Every one of the six component sub-modules carries `#![allow(dead_code)]` as its first line. This is a crate-wide suppression that hides genuinely unused exports. The `src/components/mod.rs` additionally carries `#![allow(unused_imports)]`.

```rust
// src/components/display/mod.rs:1
#![allow(dead_code)]
```

**Impact:** Unknown quantity of unused component functions are invisible to the compiler warning system.
**Fix:** Remove the module-level suppressor; fix each individual unused item or delete it.

---

### 1.2 Options Data Pipeline (Dead Feature)
**[HIGH]** `src/app/mod.rs:122–191`

```rust
#[allow(dead_code)] // Options data pipeline not yet wired
pub enum OptionsMessage {
    OptionChainLoaded { ... },
    GexProfileLoaded { ... },
}

#[allow(dead_code)] // Options data pipeline not yet wired
Options(OptionsMessage),
```

`OptionsMessage` and the `Options(OptionsMessage)` variant on the top-level `Message` enum are both silenced. The `_gex_service` from `services::initialize_options_services()` is also discarded:

```rust
// src/app/mod.rs:231
let (options_service, _gex_service) = services::initialize_options_services();
```

**Impact:** Dead initialisation code runs on startup; type system feedback suppressed.
**Fix:** Gate behind a Cargo feature flag (`--features options`) or remove until the pipeline is implemented.

---

### 1.3 `draw_drawings` Function
**[MEDIUM]** `src/chart/drawing/render.rs:15–25`

```rust
#[allow(dead_code)]
pub fn draw_drawings(...) {
    draw_completed_drawings(...);
    draw_overlay_drawings(...);
}
```

This thin wrapper is never called (callers use the two inner functions directly) but is suppressed rather than removed.

**Fix:** Delete `draw_drawings`; the two separate functions are the canonical API.

---

### 1.4 Performance Module Suppression
**[LOW]** `src/chart/perf/mod.rs:1`

```rust
#[allow(dead_code)]
pub mod lod;
```

`lod` is actively used throughout the heatmap renderer. This suppression is stale.

**Fix:** Remove the attribute.

---

### 1.5 Study Module Suppression
**[LOW]** `src/chart/mod.rs:14,16`

```rust
#[allow(dead_code)]
pub(crate) mod study;
#[allow(dead_code)]
pub mod study_renderer;
```

Both are used, but the legacy `chart::study` module (POC, value area, volume profile, imbalance) may be partially dead if the `kairos-study` crate now handles those computations.

---

### 1.6 Drawing Persistence Module
**[LOW]** `src/chart/drawing/mod.rs:10`

```rust
#[allow(dead_code)]
pub mod persistence;
```

Drawing persistence is suppressed. If drawings are not persisted this is dead infrastructure.

---

### 1.7 Replay UI Controls
**[LOW]** `src/modals/replay/mod.rs:97,99`

```rust
#[allow(dead_code)] // Planned: skip forward in replay
SkipForward,
#[allow(dead_code)] // Planned: skip backward in replay
SkipBackward,
```

Two message variants planned but unimplemented. Acceptable if the feature is imminent, but should be tracked as a ticket rather than a comment.

---

### 1.8 Sidebar Dead Field
**[LOW]** `src/screen/dashboard/sidebar.rs:326`

```rust
#[allow(dead_code)]
```

A field in the sidebar is suppressed. Without reading the full file it is unclear what it is, but it warrants investigation.

---

## 2. Code Duplication

### 2.1 FUTURES_PRODUCTS — Three Distinct Definitions
**[HIGH]**

The CME futures product catalogue is defined in three places with different shapes and partially overlapping content:

| Location | Type | Fields | Products |
|----------|------|--------|---------|
| `src/app/mod.rs:1018` | `const` `&[(&str, &str, f32, f32, f32)]` | sym, name, tick, min_qty, contract | 12 |
| `src/modals/download/mod.rs:16` | `const` `&[(&str, &str)]` | sym, name | 12 |
| `exchange/src/adapter/databento/mapper.rs:76` | local `vec!` inside `get_continuous_ticker_info()` | sym, tick, min_qty, contract | 12 (+ZT.c.0) |

The exchange mapper has an extra product (`ZT.c.0` 2-Year T-Note) not present in the UI lists. There are also differences in tick sizes for `ZF.c.0` (0.0078125 in exchange, not listed in the same way in app). Product specs are fragmented and changes must be applied in three places.

**Fix:** Define a canonical `const FUTURES_PRODUCTS_FULL: &[FuturesProduct]` in `data/src/domain/futures.rs` (or a dedicated constants module) and derive the UI-only views from it.

---

### 2.2 `dashboard_modal` Defined Twice with Different Signatures
**[MEDIUM]** `src/modals/mod.rs:77–90` vs `src/components/overlay/modal_shell.rs:158–164`

```rust
// src/modals/mod.rs:77 — backward-compat wrapper around positioned_overlay
pub fn dashboard_modal<'a, Message>(
    base, content, on_blur, padding, align_y, align_x
) -> Element<'a, Message>

// src/components/overlay/modal_shell.rs:158 — ModalShell constructor
pub fn dashboard_modal<'a, Message: Clone + 'a>(
    body, on_close
) -> ModalShell<'a, Message>
```

Both symbols are named `dashboard_modal` and both are in scope depending on what's imported. The one in `src/modals/mod.rs` is marked "backward-compatible wrapper" in its doc comment, confirming it is transitional debt.

**Fix:** Migrate all callers of the old 6-argument signature to the new `ModalShell` API and delete the wrapper.

---

### 2.3 Text Size `9.0 / chart.scaling` Repeated Three Times
**[MEDIUM]** `src/chart/heatmap/render.rs:397,545,723`

```rust
let text_size = 9.0 / chart.scaling;  // line 397
...
let text_size = 9.0 / chart.scaling;  // line 545
...
let text_size = 9.0 / chart.scaling;  // line 723
```

The same formula appears in three separate rendering functions within the same file.

**Fix:** Extract `const CHART_LABEL_BASE_PX: f32 = 9.0;` and define a helper `fn scaled_label_size(scaling: f32) -> f32 { CHART_LABEL_BASE_PX / scaling }`.

---

### 2.4 `TickerInfo` vs `FuturesTickerInfo` Near-Duplicate Structs
**[MEDIUM]** `exchange/src/types.rs:150` and `data/src/domain/futures.rs:403`

```rust
// exchange/src/types.rs:150
pub struct TickerInfo {
    pub ticker: FuturesTicker,
    pub min_ticksize: PriceStep,   // PriceStep (fixed-point)
    pub min_qty: f32,
    pub contract_size: f32,
}

// data/src/domain/futures.rs:403
pub struct FuturesTickerInfo {
    pub ticker: FuturesTicker,
    pub tick_size: f32,            // f32 — different precision!
    pub min_qty: f32,
    pub contract_size: f32,
}
```

These two structs carry the same information. Bidirectional `From` conversions, a `to_domain()` method, and a `from_domain()` method are all written manually. The field name differs (`min_ticksize` vs `tick_size`) and the type differs (`PriceStep` vs `f32`).

**Fix:** Unify to a single type in the `data` crate using `PriceStep` for precision. The exchange adapter can keep an internal type alias if needed for adapter-specific logic.

---

### 2.5 Confirm-Dialog Rendering Duplicated in `view_with_modal`
**[MEDIUM]** `src/app/mod.rs:772–786` and `src/app/mod.rs:957–971`

The same ~14-line block for layering a `ConfirmDialogBuilder` appears twice in `view_with_modal` (once in the `Settings` arm, once in the `DataFeeds` arm):

```rust
if let Some(dialog) = &self.confirm_dialog {
    let on_cancel = Message::ToggleDialogModal(None);
    let mut builder = components::overlay::confirm_dialog::ConfirmDialogBuilder::new(
        dialog.message.clone(),
        *dialog.on_confirm.clone(),
        on_cancel,
    );
    if let Some(text) = &dialog.on_confirm_btn_text {
        builder = builder.confirm_text(text.clone());
    }
    builder.view(base_content)
} else {
    base_content
}
```

**Fix:** Extract to a private `fn overlay_confirm_dialog<'a>(base, dialog) -> Element<'a, Message>` helper.

---

### 2.6 Sidebar Positioning Pattern Repeated in Every Menu Arm
**[LOW]** `src/app/mod.rs:640–1014`

In `view_with_modal`, every match arm does:
```rust
let (align_x, padding) = match sidebar_pos {
    sidebar::Position::Left => (Alignment::Start, padding::left(44).bottom(4)),
    sidebar::Position::Right => (Alignment::End, padding::right(44).bottom(4)),
};
```

with slightly different offset values. This is repeated 6 times.

**Fix:** Create a helper `fn sidebar_modal_position(pos: sidebar::Position, offset: SidebarOffset) -> (Alignment, Padding)`.

---

## 3. Complexity Hotspots

### 3.1 `view_with_modal` — 375-Line God Function
**[MEDIUM]** `src/app/mod.rs:640–1014`

The `view_with_modal` method is 375 lines long and contains a `match menu { ... }` with 7 arms, each building significant UI trees inline. Some arms are themselves 80–100 lines:

- `sidebar::Menu::Settings` arm: ~120 lines
- `sidebar::Menu::Layout` arm: ~100 lines
- `sidebar::Menu::DataFeeds` arm: ~60 lines

**Fix:** Extract each match arm into a dedicated `fn view_settings_modal(...)`, `fn view_layout_modal(...)`, etc.

---

### 3.2 `view` Function — 232 Lines with Nested `if let` Chains
**[MEDIUM]** `src/app/mod.rs:401–631`

The top-level `view()` method is 231 lines, building the entire main window layout including conditional platform-specific code, the drawing tool flyout, menu bar dropdown, save dialog, replay controller, and toast manager. Each layer wraps the previous in a `stack![]` macro.

**Fix:** Extract sub-sections to named helpers: `fn view_main_content(...)`, `fn view_with_flyout(...)`, `fn view_with_menu_dropdown(...)`.

---

### 3.3 `draw_volume_bar` — 11 Parameters
**[MEDIUM]** `src/chart/mod.rs:431–498`

```rust
pub fn draw_volume_bar(
    frame: &mut Frame,
    start_x: f32,
    start_y: f32,
    buy_qty: f32,
    sell_qty: f32,
    max_qty: f32,
    bar_length: f32,
    thickness: f32,
    buy_color: iced::Color,
    sell_color: iced::Color,
    bar_color_alpha: f32,
    horizontal: bool,   // 11 params — clippy limit is 5
)
```

Called from at least 3 sites in `heatmap/render.rs` (lines 527, 704) and `candlestick`. The function itself is `#[allow(clippy::too_many_arguments)]`-exempt.

**Fix:** Introduce a `VolumeBarSpec { buy_qty, sell_qty, max_qty, buy_color, sell_color, alpha }` struct. The positional and sizing args can remain as parameters.

---

### 3.4 `draw_clusters` — 17 Parameters
**[MEDIUM]** `src/chart/candlestick/footprint.rs:392–411`

```rust
pub fn draw_clusters(
    frame, price_to_y, x_position, cell_width, cell_height,
    candle_width, max_cluster_qty, palette, text_size,
    _tick_size, show_text, candle, footprint, study_type,
    scaling, candle_position, mode, spacing   // 17 params
)
```

Note `_tick_size` is prefixed with `_` indicating it is currently unused (another dead code smell).

**Fix:** Group related parameters into structs: `ClusterLayout { x_position, cell_width, cell_height, ... }`, `ClusterStyle { palette, text_size, ... }`.

---

### 3.5 `study/src/orderflow/big_trades.rs` — 1,229 Lines
**[MEDIUM]** `study/src/orderflow/big_trades.rs`

The file is 1,229 lines, of which approximately 620 lines (50%) are test code. The test code contains highly repetitive patterns — each test calls the same 3-4 helper functions and the same `match output { StudyOutput::Markers(m) => { ... }, other => panic!(...) }` idiom.

---

### 3.6 `src/modals/pane/indicator_manager.rs` — 1,114 Lines
**[MEDIUM]** `src/modals/pane/indicator_manager.rs`

Monolithic modal file without sub-module breakdown.

---

### 3.7 `src/screen/dashboard/panel/ladder.rs` — 1,283 Lines
**[MEDIUM]** `src/screen/dashboard/panel/ladder.rs`

Canvas-based DOM panel with all rendering, state, and logic in a single file. 42 functions defined. The file mixes: configuration management, orderbook data structures (`GroupedDepth`, `TradeStore`), animation state, and drawing primitives.

**Fix:** Split into `state.rs`, `render.rs`, and `layout.rs` sub-modules.

---

## 4. Naming Inconsistencies

### 4.1 `TickerInfo` vs `FuturesTickerInfo` — Same Concept, Two Names
**[MEDIUM]** Cross-crate naming inconsistency between `exchange::TickerInfo` and `data::domain::FuturesTickerInfo` (already covered under Duplication §2.4, also a naming issue).

---

### 4.2 `dashboard_modal` as Both a Function and a Style Constructor
**[LOW]**

`dashboard_modal` is used as: (a) a free function creating a positioned overlay (`src/modals/mod.rs`), (b) a `ModalShell` constructor (`src/components/overlay/modal_shell.rs`), and (c) a container style function (`src/style/container.rs:124`). Three different things, one name.

---

### 4.3 Inconsistent `new()` vs `with_*` Patterns
**[LOW]**

Some structs expose `new()` taking all required parameters (e.g., `Ladder::new(config, ticker_info, tick_size)`), while others use the builder pattern. No consistent convention is enforced.

---

### 4.4 `_tick_size` Unused Parameter in Public Function
**[LOW]** `src/chart/candlestick/footprint.rs:402`

```rust
_tick_size: f32,
```

A public function has an unused parameter named with underscore prefix. This is acceptable in trait implementations but should be removed if the function is free-standing.

---

## 5. Magic Numbers & Strings

### 5.1 Hardcoded Company Name (Placeholder)
**[MEDIUM]** `src/app/mod.rs:383–422`

```rust
// src/app/mod.rs:383
format!("XX & Company [{}]", id.name)
// src/app/mod.rs:385
"XX & Company".to_string()
// src/app/mod.rs:422
text("XXNCO")
```

Placeholder company names are hardcoded in the production application title bar. These are not constants and cannot be changed without recompilation.

**Fix:** Define `const APP_NAME: &str = "Kairos";` in a central location (`src/lib.rs` or `data/src/config`).

---

### 5.2 Chart Label Base Size: 9.0
**[MEDIUM]** `src/chart/heatmap/render.rs:397,545,723`

(Already covered under Duplication §2.3)

---

### 5.3 Menu Geometry Hardcoded
**[LOW]** `src/app/mod.rs:519–521`

```rust
Some(menu_bar::Menu::File) => tokens::spacing::SM,
Some(menu_bar::Menu::Layout) => tokens::spacing::SM + 46.0,
```

The `46.0` pixel offset for the Layout menu is hardcoded. This offset is tied to the rendered width of the "File" menu button and will silently break if the button text changes.

**Fix:** Calculate dynamically from the rendered menu item width, or extract as a named layout constant.

---

### 5.4 Sidebar Icon Padding
**[LOW]** `src/app/mod.rs:759,760`

```rust
(Alignment::Start, padding::left(44).bottom(4))
```

The value `44` (sidebar icon column width) appears in multiple match arms as a literal. It is also present as `tokens::layout::SIDEBAR_WIDTH` elsewhere. These should be unified.

---

### 5.5 LOD Decision Thresholds Embedded in Logic
**[LOW]** `src/chart/perf/lod.rs:111–120`

```rust
if self.scaling < 0.5 || items_per_pixel > 5.0 || item_count > 10_000 {
    LodLevel::Low
} else if self.scaling < 1.0 || items_per_pixel > 2.0 || item_count > 5_000 {
    LodLevel::Medium
} else {
    LodLevel::High
}
```

All six threshold values are magic numbers with no named constants. Performance tuning requires reading the source.

**Fix:** Define `const LOD_LOW_SCALE_THRESHOLD: f32 = 0.5;` etc.

---

### 5.6 Volume Profile Hard Limits
**[LOW]** `src/chart/heatmap/render.rs:623–629`

```rust
let min_segment_width = 2.0;
let segments = ((area_width / min_segment_width).floor() as usize).clamp(10, 40);
let alpha = 0.95 - (0.85 * (i as f32 / (segments - 1) as f32).powf(2.0));
```

Gradient segment count limits (10–40), alpha values (0.95, 0.85), and minimum segment width (2.0) are all inline magic numbers.

---

## 6. Commented-Out Code

### 6.1 TODO: Migration Deferred
**[MEDIUM]** `src/chart/comparison/mod.rs:512`

```rust
// TODO: Remove once TickerInfo is fully migrated to FuturesTickerInfo
```

This TODo marks an incomplete migration between two ticker info types. The dual type system (§2.4) remains as a result.

---

### 6.2 Unimplemented Replay Controls
**[LOW]** `src/modals/replay/mod.rs:97,99`

```rust
#[allow(dead_code)] // Planned: skip forward in replay
SkipForward,
#[allow(dead_code)] // Planned: skip backward in replay
SkipBackward,
```

---

### 6.3 TODO: Widget Focusing
**[LOW]** `src/screen/dashboard/update.rs:207`

```rust
// TODO: Implement widget focusing with the specific ID
```

---

### 6.4 TODO: Ticker Switching in Link Groups
**[LOW]** `src/screen/dashboard/update.rs:166`

```rust
// TODO: Handle ticker switching in link groups
```

---

## 7. Anti-Patterns in Rust

### 7.1 `panic!()` as Assertions in Unit Tests
**[HIGH]** `study/src/orderflow/big_trades.rs:689–1079`

```rust
// study/src/orderflow/big_trades.rs:689
other => panic!("Expected Markers, got {:?}", other),
```

This pattern appears 11+ times in test code. The correct idiom is:

```rust
other => panic!("Expected Markers, got {:?}", other),
// Should be:
_ => panic!("...")  // or use assert_matches! macro
```

More importantly, the entire `match` block should use `assert_matches!` from `std::assert_matches` (stable since 1.82) or the popular `claims` crate:

```rust
assert_matches!(study.output(), StudyOutput::Markers(m) if m.len() == 1);
```

**Fix:** Replace `match output { ... other => panic!(...) }` with `assert_matches!()`.

---

### 7.2 Mutex Poisoning Ignored via `unwrap()`
**[MEDIUM]** `src/app/mod.rs:365`, `src/app/update/download.rs:138,220,255,387`, `src/layout.rs:75`

```rust
// src/app/mod.rs:365
self.data_feed_manager.lock().unwrap_or_else(|e| e.into_inner());
// (inconsistent: some use unwrap_or_else, some use unwrap)
let mut progress = get_download_progress().lock().unwrap();
```

Most Mutex lock sites use `.unwrap()` which will panic on poison. A few use `.unwrap_or_else(|e| e.into_inner())` (poison recovery), but the pattern is inconsistent.

**Fix:** Define a helper or use `unwrap_or_else` consistently across all lock sites.

---

### 7.3 `*dialog.on_confirm.clone()` — Clone then Deref
**[LOW]** `src/app/mod.rs:777,962`

```rust
*dialog.on_confirm.clone(),
```

This clones a `Box<Message>` and immediately derefs the clone to get a `Message`. This is equivalent to `(*dialog.on_confirm).clone()` but less clear. Since `Message: Clone`, the clone could be on the inner value directly.

---

### 7.4 `text().to_string()` Where `&str` Suffices
**[LOW]** `src/app/mod.rs:385,800`

```rust
"XX & Company".to_string()
// and
"".to_string()
```

In `title()`, which returns `String`, this is unavoidable. In `selected_pane_str` construction (line 800), an owned String is needed for concatenation, so it is reasonable. However, there are 163 `.to_string()` calls in `src/` that should each be reviewed for cases where `&str` would suffice in function signatures.

---

### 7.5 `unwrap()` on Infallible Date Construction
**[LOW]** `src/modals/pane/calendar.rs:48,62,67,96,98–100,161` and `src/modals/replay/mod.rs:656,669,997,1006,1007`

```rust
NaiveDate::from_ymd_opt(yesterday.year(), yesterday.month(), 1).unwrap()
```

`from_ymd_opt` is used instead of the panicking `from_ymd` (which is deprecated) — this is correct behavior. However, since the day is always `1` and year/month come from an existing valid date, this is infallible in practice. Using `.expect("valid date: first of month")` would be cleaner than `.unwrap()`.

---

### 7.6 Discarded GEX Service on Startup
**[LOW]** `src/app/mod.rs:231`

```rust
let (options_service, _gex_service) = services::initialize_options_services();
```

`_gex_service` is initialized and then immediately discarded. The initialization likely creates network connections, allocates memory, or registers services — all wasted.

---

## 8. Documentation Gaps

### 8.1 `src/chart/mod.rs` — `draw_volume_bar` Undocumented Parameters
**[LOW]** `src/chart/mod.rs:431`

The 11-parameter `draw_volume_bar` function has no doc comment. Each parameter's purpose (e.g., `horizontal: bool` — orientation of the bar, `bar_color_alpha: f32` — alpha multiplier) is non-obvious.

---

### 8.2 `FUTURES_PRODUCTS` Constants Undocumented
**[LOW]**

None of the three FUTURES_PRODUCTS definitions carry a doc comment explaining what the fields mean:

```rust
pub(crate) const FUTURES_PRODUCTS: &[(&str, &str, f32, f32, f32)] = &[
    ("ES.c.0", "E-mini S&P 500", 0.25, 1.0, 50.0),
    //                            ^     ^     ^
    //                         tick  min  contract
    //                         size  qty  size
```

Column semantics are discoverable from usage context but not from the definition.

---

### 8.3 `view_with_modal` — No Function-Level Doc
**[LOW]** `src/app/mod.rs:640`

The 375-line `view_with_modal` function has a 2-line doc comment that doesn't explain the relationship between `base`, `dashboard`, and `menu`.

---

### 8.4 Chart Module Naming: `perf`
**[LOW]** `src/chart/perf/mod.rs`

The module name `perf` is ambiguous — it could mean "performance metrics", "performance profiling", or "LOD rendering". The actual purpose (LOD-based render decimation) is not in the module name.

---

## Summary of Recommendations (Priority Order)

1. **[P0 - Immediate]** Remove `#![allow(dead_code)]` from all 6 component sub-modules; address each actual dead item.
2. **[P0 - Immediate]** Consolidate `FUTURES_PRODUCTS` into a single canonical constant in `data/` crate.
3. **[P1 - Short-term]** Delete or feature-gate the options pipeline dead code; remove `_gex_service` initialization.
4. **[P1 - Short-term]** Replace `panic!("Expected Markers")` in `big_trades.rs` tests with `assert_matches!`.
5. **[P1 - Short-term]** Merge `dashboard_modal` to a single function, delete the backward-compat wrapper.
6. **[P2 - Medium-term]** Unify `TickerInfo` / `FuturesTickerInfo` into a single type.
7. **[P2 - Medium-term]** Extract the 3× `text_size = 9.0 / chart.scaling` into a named constant + helper.
8. **[P2 - Medium-term]** Replace hardcoded "XX & Company" strings with a `const APP_NAME`.
9. **[P2 - Medium-term]** Extract the confirm-dialog overlay block (duplicated in `view_with_modal`) to a helper.
10. **[P3 - Long-term]** Refactor `draw_volume_bar` (11 params), `draw_clusters` (17 params) using builder/spec structs.
11. **[P3 - Long-term]** Split `ladder.rs` (1,283 lines) and `indicator_manager.rs` (1,114 lines) into sub-modules.
12. **[P3 - Long-term]** Add LOD threshold constants; document `draw_volume_bar` parameters.
