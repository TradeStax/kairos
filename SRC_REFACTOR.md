# Comprehensive src/ Refactor Plan - Modular Architecture

## Executive Summary

Complete refactor of the `src/` directory with a focus on:
- **Clean modular architecture** - Pluggable components (indicators, overlays, studies) stay separate
- **Consolidated infrastructure** - Shared code (ViewState, Caches, Interaction) unified
- **Subdirectories for separation of concerns** - Clear module boundaries
- **Perfect maintainability** - Easy to add new indicators, overlays, chart types
- **100% functional and visual parity** - No user-facing changes

---

## Architecture Principles

### CONSOLIDATE: Shared Infrastructure
- Core chart state (ViewState, Caches, Interaction)
- Message handling and update logic
- Coordinate transformation utilities
- Common rendering helpers

### KEEP SEPARATE: Pluggable Components
- **Each indicator** = separate file (easy to add new ones)
- **Each overlay** = separate file (depth, trades, imbalance)
- **Each study** = separate file (POC, nPOC, VAH/VAL)
- **Each chart type** = separate module (candlestick, heatmap, comparison)
- **Each panel** = separate file (ladder, timeandsales)

---

## Complete Directory Structure

```
src/
├── main.rs                        # Entry point only (~150 lines)
│
├── app/                           # Application core (extracted from main.rs)
│   ├── mod.rs                     # Flowsurface struct, Message enum
│   ├── state.rs                   # Application state management
│   ├── subscriptions.rs           # Event subscriptions
│   └── services.rs                # Service initialization
│
├── chart/                         # CHART MODULE
│   ├── mod.rs                     # Public API: Chart trait, Message, update(), view()
│   │
│   ├── core/                      # CONSOLIDATED: Shared infrastructure
│   │   ├── mod.rs                 # Re-exports
│   │   ├── view_state.rs          # ViewState struct + coordinate transforms (~400 lines)
│   │   ├── caches.rs              # Cache management (~50 lines)
│   │   ├── interaction.rs         # Interaction enum, mouse/keyboard handling (~200 lines)
│   │   ├── autoscale.rs           # Autoscaling logic (~150 lines)
│   │   └── traits.rs              # Chart, PlotConstants traits (~50 lines)
│   │
│   ├── candlestick/               # Candlestick chart type
│   │   ├── mod.rs                 # KlineChart struct, Chart trait impl (~400 lines)
│   │   ├── render.rs              # Canvas rendering dispatch (~300 lines)
│   │   ├── candle.rs              # OHLC candle rendering (~100 lines)
│   │   ├── footprint.rs           # Footprint cluster rendering (~400 lines)
│   │   └── config.rs              # KlineChartKind configuration (~50 lines)
│   │
│   ├── heatmap/                   # Heatmap chart type
│   │   ├── mod.rs                 # HeatmapChart struct (~400 lines)
│   │   ├── render.rs              # Depth heatmap rendering (~400 lines)
│   │   ├── trades.rs              # Trade circle/rectangle rendering (~200 lines)
│   │   └── data.rs                # HeatmapData structure (~200 lines)
│   │
│   ├── comparison/                # Comparison chart type
│   │   ├── mod.rs                 # ComparisonChart struct (~400 lines)
│   │   └── series.rs              # Series management (~200 lines)
│   │
│   ├── indicator/                 # MODULAR: Each indicator separate file
│   │   ├── mod.rs                 # Indicator trait, indicator_row helper
│   │   ├── plot/                  # Plot rendering infrastructure
│   │   │   ├── mod.rs             # Plot trait, Series trait, ChartCanvas
│   │   │   ├── line.rs            # LinePlot implementation
│   │   │   └── bar.rs             # BarPlot implementation
│   │   └── kline/                 # Kline-specific indicators
│   │       ├── mod.rs             # KlineIndicatorImpl trait, factory
│   │       ├── volume.rs          # Volume indicator
│   │       ├── delta.rs           # Delta indicator
│   │       ├── open_interest.rs   # Open interest indicator
│   │       ├── sma.rs             # Simple moving average
│   │       ├── ema.rs             # Exponential moving average
│   │       ├── rsi.rs             # RSI indicator
│   │       ├── macd.rs            # MACD indicator
│   │       └── bollinger.rs       # Bollinger bands
│   │
│   ├── overlay/                   # MODULAR: Each overlay separate file
│   │   ├── mod.rs                 # Overlay trait + registry
│   │   ├── crosshair.rs           # Crosshair drawing
│   │   ├── ruler.rs               # Ruler measurement tool
│   │   ├── last_price.rs          # Last price line
│   │   └── grid.rs                # Grid rendering
│   │
│   ├── study/                     # MODULAR: Each study separate file
│   │   ├── mod.rs                 # Study trait + registry
│   │   ├── poc.rs                 # Point of Control
│   │   ├── npoc.rs                # Naked POC
│   │   ├── value_area.rs          # VAH, VAL
│   │   ├── imbalance.rs           # Imbalance markers
│   │   └── volume_profile.rs      # Volume profile
│   │
│   ├── display_data/              # Cached display structures
│   │   ├── mod.rs                 # DisplayData trait, DisplayDataCache
│   │   ├── footprint.rs           # FootprintDisplayData
│   │   └── heatmap.rs             # HeatmapDisplayData
│   │
│   ├── scale/                     # Axis rendering
│   │   ├── mod.rs                 # AxisLabel, scale utilities
│   │   ├── linear.rs              # Linear Y-axis (price)
│   │   └── timeseries.rs          # Time X-axis
│   │
│   └── perf/                      # Performance systems
│       ├── mod.rs                 # RenderBudget, FrameMetrics, presets
│       ├── lod.rs                 # Level-of-detail management
│       ├── viewport.rs            # ViewportBounds calculations
│       └── progressive.rs         # Progressive rendering
│
├── screen/                        # SCREENS
│   ├── mod.rs                     # Screen exports, ConfirmDialog
│   │
│   └── dashboard/                 # Dashboard screen
│       ├── mod.rs                 # Dashboard struct, Message, update, view
│       ├── state.rs               # Dashboard state (panes, focus, popout)
│       │
│       ├── pane/                  # MODULAR: Pane system
│       │   ├── mod.rs             # State struct, Message, update
│       │   ├── content.rs         # Content enum (Kline, Heatmap, Panel, etc.)
│       │   ├── view.rs            # Pane view rendering
│       │   ├── header.rs          # Pane header/title bar
│       │   └── effects.rs         # Pane effects (LoadChart, etc.)
│       │
│       ├── panel/                 # MODULAR: Panel types (non-chart content)
│       │   ├── mod.rs             # Panel trait
│       │   ├── ladder.rs          # DOM ladder panel
│       │   └── timeandsales.rs    # Time & sales panel
│       │
│       ├── sidebar/               # Sidebar component
│       │   ├── mod.rs             # Sidebar struct, Message, view
│       │   └── menu.rs            # Menu items
│       │
│       └── tickers_table.rs       # Ticker selection table
│
├── modal/                         # MODALS
│   ├── mod.rs                     # Modal helpers (dashboard_modal, main_dialog_modal)
│   │
│   ├── layout_manager.rs          # Layout management modal
│   ├── theme_editor.rs            # Theme customization modal
│   ├── audio.rs                   # Audio settings modal
│   │
│   └── pane/                      # Pane-specific modals
│       ├── mod.rs                 # Modal enum, stack_modal helper
│       ├── settings/              # MODULAR: Chart settings by type
│       │   ├── mod.rs             # Settings view dispatch
│       │   ├── kline.rs           # Kline chart settings
│       │   ├── heatmap.rs         # Heatmap chart settings
│       │   ├── comparison.rs      # Comparison chart settings
│       │   └── study.rs           # Study configurator
│       ├── indicators.rs          # Indicator selection modal
│       ├── data_management.rs     # Data download modal
│       ├── stream.rs              # Stream modifier modal
│       └── mini_tickers_list.rs   # Quick ticker selector
│
├── widget/                        # REUSABLE WIDGETS
│   ├── mod.rs                     # Widget helpers (tooltip, confirm_dialog, etc.)
│   ├── toast.rs                   # Toast notifications
│   ├── color_picker.rs            # Color picker widget
│   ├── column_drag.rs             # Draggable column widget
│   ├── multi_split.rs             # Multi-split container
│   ├── decorate.rs                # Decoration helpers
│   └── chart/                     # Chart widgets
│       ├── mod.rs                 # Chart widget helpers
│       └── comparison.rs          # LineComparison widget
│
├── style/                         # STYLING (extracted from style.rs)
│   ├── mod.rs                     # Re-exports, Icon enum
│   ├── button.rs                  # Button styles
│   ├── container.rs               # Container styles
│   └── colors.rs                  # Color utilities
│
├── layout.rs                      # Layout persistence
├── window.rs                      # Window management
├── audio.rs                       # Audio system
└── logger.rs                      # Logging
```

---

## Key Trait Definitions

### 1. Overlay Trait (`chart/overlay/mod.rs`)

```rust
/// Overlay that renders on top of chart content
pub trait Overlay: Send + Sync {
    /// Unique identifier
    fn id(&self) -> &'static str;

    /// Human-readable name for UI
    fn name(&self) -> &str;

    /// Whether currently enabled
    fn enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);

    /// Draw the overlay
    fn draw(
        &self,
        frame: &mut canvas::Frame,
        ctx: &ViewState,
        theme: &Theme,
        viewport: &ViewportBounds,
    );

    /// Invalidate cached geometry
    fn invalidate(&mut self);
}

/// Registry for managing overlays
pub struct OverlayRegistry {
    overlays: Vec<Box<dyn Overlay>>,
}

impl OverlayRegistry {
    pub fn register(&mut self, overlay: Box<dyn Overlay>);
    pub fn get(&self, id: &str) -> Option<&dyn Overlay>;
    pub fn get_mut(&mut self, id: &str) -> Option<&mut dyn Overlay>;
    pub fn enabled_iter(&self) -> impl Iterator<Item = &dyn Overlay>;
    pub fn draw_all(&self, frame: &mut canvas::Frame, ctx: &ViewState, ...);
}
```

### 2. Study Trait (`chart/study/mod.rs`)

```rust
/// Chart study that calculates and renders analytical data
pub trait Study: Send + Sync {
    /// Study identifier
    fn id(&self) -> &'static str;

    /// Display name
    fn name(&self) -> &str;

    /// Whether enabled
    fn enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);

    /// Recalculate study data
    fn calculate(&mut self, data: &ChartData, viewport: &ViewportBounds);

    /// Draw on main chart area
    fn draw_main(&self, frame: &mut canvas::Frame, ctx: &ViewState, theme: &Theme);

    /// Draw on price axis (optional)
    fn draw_y_axis(&self, _frame: &mut canvas::Frame, _ctx: &ViewState, _theme: &Theme) {}

    /// Invalidate cached calculations
    fn invalidate(&mut self);

    /// Configuration UI element (optional)
    fn config_element(&self) -> Option<Element<'_, StudyMessage>> { None }
}

/// Registry for managing studies
pub struct StudyRegistry {
    studies: Vec<Box<dyn Study>>,
}
```

### 3. Indicator Trait (enhanced `chart/indicator/kline/mod.rs`)

```rust
/// Indicator implementation trait (already exists, keep as-is)
pub trait KlineIndicatorImpl: Send + Sync {
    fn clear_all_caches(&mut self);
    fn clear_crosshair_caches(&mut self);
    fn element(&self, chart: &ViewState, range: RangeInclusive<u64>) -> Element<Message>;
    fn rebuild_from_candles(&mut self, candles: &[Candle]);
    fn on_ticksize_change(&mut self, candles: &[Candle]);
    fn on_basis_change(&mut self, candles: &[Candle]);
}

/// Factory for creating indicators (already exists, keep as-is)
pub fn make_empty(which: KlineIndicator) -> Box<dyn KlineIndicatorImpl> {
    match which {
        KlineIndicator::Volume => Box::new(volume::VolumeIndicator::new()),
        KlineIndicator::Delta => Box::new(delta::DeltaIndicator::new()),
        KlineIndicator::Sma20 => Box::new(sma::SmaIndicator::new(20)),
        // ... etc
    }
}
```

### 4. Renderer Context (`chart/core/mod.rs`)

```rust
/// Shared context for rendering operations
pub struct RenderContext<'a> {
    pub view_state: &'a ViewState,
    pub theme: &'a Theme,
    pub palette: &'a Extended,
    pub viewport: ViewportBounds,
    pub lod: LodLevel,
}

impl<'a> RenderContext<'a> {
    /// Create from chart state and theme
    pub fn new(view_state: &'a ViewState, theme: &'a Theme) -> Self {
        let viewport = view_state.calculate_viewport();
        let lod = view_state.calculate_lod();
        Self {
            view_state,
            theme,
            palette: theme.extended_palette(),
            viewport,
            lod,
        }
    }
}
```

---

## Module Extraction Plan

### Phase 1: Extract `chart/core/` from `chart.rs`

**From `chart.rs` (1,217 lines) extract:**

| Component | New Location | Lines |
|-----------|--------------|-------|
| `ViewState` struct + methods | `core/view_state.rs` | ~400 |
| `Interaction` enum | `core/interaction.rs` | ~15 |
| `canvas_interaction()` | `core/interaction.rs` | ~190 |
| `Caches` struct | `core/caches.rs` | ~30 |
| `Chart`, `PlotConstants` traits | `core/traits.rs` | ~50 |
| `draw_crosshair()` | `overlay/crosshair.rs` | ~200 |
| `draw_last_price_line()` | `overlay/last_price.rs` | ~30 |
| `update()`, `view()` | `mod.rs` | ~150 |

**Result:** `chart.rs` → `chart/mod.rs` (~150 lines) + `chart/core/` (~500 lines total)

### Phase 2: Split `chart/candlestick/` from `kline.rs`

**From `kline.rs` (2,090 lines) extract:**

| Component | New Location | Lines |
|-----------|--------------|-------|
| `KlineChart` struct | `candlestick/mod.rs` | ~400 |
| `draw_candle()` | `candlestick/candle.rs` | ~100 |
| `draw_clusters()`, footprint layout | `candlestick/footprint.rs` | ~400 |
| Canvas rendering dispatch | `candlestick/render.rs` | ~300 |
| `KlineChartKind` | `candlestick/config.rs` | ~50 |
| `draw_all_npocs()` | `study/npoc.rs` | ~130 |
| `draw_imbalance_markers()` | `study/imbalance.rs` | ~70 |

**Result:** `kline.rs` (2,090 lines) → `candlestick/` (~1,250 lines total, well-organized)

### Phase 3: Split `chart/heatmap/` from `heatmap.rs`

**From `heatmap.rs` (1,623 lines) extract:**

| Component | New Location | Lines |
|-----------|--------------|-------|
| `HeatmapChart` struct | `heatmap/mod.rs` | ~400 |
| Depth rendering | `heatmap/render.rs` | ~400 |
| Trade circles/rectangles | `heatmap/trades.rs` | ~200 |
| `HeatmapData` | `heatmap/data.rs` | ~200 |
| Volume profile study | `study/volume_profile.rs` | ~150 |

**Result:** `heatmap.rs` (1,623 lines) → `heatmap/` (~1,200 lines total)

### Phase 4: Create `chart/overlay/` module

**New overlay module with existing crosshair + new abstractions:**

| File | Content | Lines |
|------|---------|-------|
| `mod.rs` | Overlay trait, OverlayRegistry | ~100 |
| `crosshair.rs` | Crosshair drawing (from chart.rs) | ~200 |
| `ruler.rs` | Ruler measurement (from chart.rs) | ~150 |
| `last_price.rs` | Last price line | ~50 |
| `grid.rs` | Grid rendering | ~100 |

### Phase 5: Create `chart/study/` module

**Extract studies from kline.rs and heatmap.rs:**

| File | Content | Lines |
|------|---------|-------|
| `mod.rs` | Study trait, StudyRegistry | ~100 |
| `poc.rs` | Point of Control | ~80 |
| `npoc.rs` | Naked POC (from kline.rs) | ~130 |
| `value_area.rs` | VAH, VAL | ~100 |
| `imbalance.rs` | Imbalance markers (from kline.rs) | ~80 |
| `volume_profile.rs` | Volume profile (from heatmap.rs) | ~150 |

### Phase 6: Split `screen/dashboard/pane/` from `pane.rs`

**From `pane.rs` (1,757 lines) extract:**

| Component | New Location | Lines |
|-----------|--------------|-------|
| `State` struct, `update()` | `pane/mod.rs` | ~400 |
| `Content` enum | `pane/content.rs` | ~300 |
| View rendering | `pane/view.rs` | ~400 |
| Header rendering | `pane/header.rs` | ~150 |
| `Effect` enum | `pane/effects.rs` | ~100 |

### Phase 7: Split `modal/pane/settings/`

**From `settings.rs` (1,088 lines) extract:**

| Component | New Location | Lines |
|-----------|--------------|-------|
| Settings dispatch | `settings/mod.rs` | ~100 |
| `kline_cfg_view()` | `settings/kline.rs` | ~300 |
| `heatmap_cfg_view()` | `settings/heatmap.rs` | ~300 |
| `comparison_cfg_view()` | `settings/comparison.rs` | ~200 |
| Study configurator | `settings/study.rs` | ~200 |

### Phase 8: Extract `app/` from `main.rs`

**From `main.rs` (1,624 lines) extract:**

| Component | New Location | Lines |
|-----------|--------------|-------|
| Entry point | `main.rs` | ~50 |
| `Flowsurface` struct, `Message` | `app/mod.rs` | ~400 |
| State management methods | `app/state.rs` | ~400 |
| `subscription()` | `app/subscriptions.rs` | ~150 |
| Service initialization | `app/services.rs` | ~200 |

---

## Dependency Flow

```
                         ┌──────────────────┐
                         │   chart/core/    │
                         │  - ViewState     │
                         │  - Caches        │
                         │  - Interaction   │
                         │  - Traits        │
                         └────────┬─────────┘
                                  │
        ┌─────────────────────────┼─────────────────────────┐
        │                         │                         │
        ▼                         ▼                         ▼
┌───────────────┐        ┌───────────────┐        ┌───────────────┐
│ display_data/ │        │    overlay/   │        │    scale/     │
│ - footprint   │        │ - crosshair   │        │ - linear      │
│ - heatmap     │        │ - ruler       │        │ - timeseries  │
└───────┬───────┘        │ - last_price  │        └───────────────┘
        │                └───────┬───────┘
        │                        │
        └────────────┬───────────┘
                     │
                     ▼
        ┌────────────────────────────────────────┐
        │            Chart Types                  │
        │  ┌────────────┐ ┌────────────┐         │
        │  │candlestick/│ │  heatmap/  │         │
        │  │ - mod.rs   │ │ - mod.rs   │         │
        │  │ - render   │ │ - render   │         │
        │  │ - candle   │ │ - trades   │         │
        │  │ - footprint│ │ - data     │         │
        │  └────────────┘ └────────────┘         │
        └────────────────────────────────────────┘
                     │
        ┌────────────┼────────────┐
        │            │            │
        ▼            ▼            ▼
┌───────────┐  ┌───────────┐  ┌───────────┐
│ indicator/│  │   study/  │  │   perf/   │
│ - volume  │  │ - poc     │  │ - lod     │
│ - delta   │  │ - npoc    │  │ - viewport│
│ - sma/ema │  │ - imbal.  │  │ - progress│
│ - rsi     │  │ - vah/val │  └───────────┘
│ - macd    │  └───────────┘
│ - bolling.│
└───────────┘
```

---

## File Count Summary

| Module | Current Files | New Files | Change |
|--------|--------------|-----------|--------|
| chart/ | 24 | 35 | +11 (better organization) |
| screen/ | 8 | 14 | +6 (split large pane.rs) |
| modal/ | 9 | 12 | +3 (split settings.rs) |
| widget/ | 8 | 9 | +1 |
| app/ (new) | 0 | 4 | +4 (extracted from main.rs) |
| root | 10 | 7 | -3 |
| **Total** | **59** | **~81** | **+22** |

**Note:** More files, but each file is focused and maintainable (100-400 lines typically)

---

## Key Benefits

### 1. Maintainability
- **Add new indicator:** Create `indicator/kline/new_indicator.rs`, add to factory
- **Add new study:** Create `study/new_study.rs`, register in StudyRegistry
- **Add new overlay:** Create `overlay/new_overlay.rs`, register in OverlayRegistry

### 2. Code Navigation
- Clear module boundaries
- 2-3 levels max nesting
- Related code grouped together

### 3. Testing
- Each component testable in isolation
- Traits enable mocking
- Clear dependencies

### 4. Performance
- LOD system in dedicated `perf/lod.rs`
- Viewport culling in `perf/viewport.rs`
- Progressive rendering in `perf/progressive.rs`

---

## Implementation Order

### Week 1: Core Infrastructure
1. Create `chart/core/` directory structure
2. Extract ViewState, Caches, Interaction from chart.rs
3. Update all imports
4. Verify compilation

### Week 2: Chart Type Reorganization
1. Create `chart/candlestick/` from kline.rs
2. Create `chart/heatmap/` from heatmap.rs
3. Create `chart/overlay/` module
4. Create `chart/study/` module

### Week 3: Screen & Modal Splits
1. Split `screen/dashboard/pane/` from pane.rs
2. Split `modal/pane/settings/` from settings.rs
3. Create `screen/dashboard/sidebar/` module

### Week 4: Application Extraction & Cleanup
1. Extract `app/` from main.rs
2. Update all imports throughout codebase
3. Run full test suite
4. Performance validation

---

## Verification Checklist

### Functional Parity
- [ ] All chart types render identically
- [ ] All indicators work
- [ ] All studies work
- [ ] All overlays work
- [ ] Link groups synchronize
- [ ] Modals function correctly
- [ ] Layout persistence works
- [ ] Theme changes apply

### Code Quality
- [ ] No file > 500 lines (except complex rendering)
- [ ] All traits well-documented
- [ ] Clear module boundaries
- [ ] No circular dependencies
- [ ] Clippy warnings resolved

### Build Verification
```bash
cargo build --release
cargo test
cargo clippy -- -D warnings
```

---

## Files to Create

### New Directories
- `src/app/`
- `src/chart/core/`
- `src/chart/candlestick/`
- `src/chart/heatmap/`
- `src/chart/overlay/`
- `src/chart/study/`
- `src/chart/perf/`
- `src/screen/dashboard/pane/`
- `src/screen/dashboard/sidebar/`
- `src/modal/pane/settings/`

### New Files (key ones)
- `src/chart/core/view_state.rs`
- `src/chart/core/interaction.rs`
- `src/chart/overlay/mod.rs` (Overlay trait)
- `src/chart/study/mod.rs` (Study trait)
- `src/chart/candlestick/render.rs`
- `src/chart/heatmap/render.rs`
- `src/screen/dashboard/pane/content.rs`
- `src/screen/dashboard/pane/view.rs`

## Files to Delete (after extraction)

- `src/chart/kline.rs` → split into `candlestick/`
- `src/chart/heatmap.rs` → split into `heatmap/`
- `src/chart/perf.rs` → moved to `perf/mod.rs`
- `src/chart/perf_overlay.rs` → moved to `perf/`
- `src/chart/presets.rs` → moved to `perf/`
