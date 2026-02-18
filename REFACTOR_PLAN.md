# Flowsurface `src/` Comprehensive Refactor Plan

> **Status**: Phase 1 (Review) complete. Phase 2 (Plan) below. No code changes yet.
> **Scope**: 112 Rust files, ~25,000 lines across 8 modules in `src/`
> **Framework**: Iced 0.14.0 — daemon (multi-window), canvas, wgpu, Elm architecture

---

## Table of Contents

1. [Current State Summary](#1-current-state-summary)
2. [Proposed File Structure](#2-proposed-file-structure)
3. [Design Token System](#3-design-token-system)
4. [Reusable Component Library](#4-reusable-component-library)
5. [Message Architecture](#5-message-architecture)
6. [Deduplication Targets](#6-deduplication-targets)
7. [Dead Code Resolution](#7-dead-code-resolution)
8. [Migration Order](#8-migration-order)

---

## 1. Current State Summary

### Key Metrics from Review

| Module | Files | Lines | Largest File | Critical Issues |
|--------|-------|-------|-------------|-----------------|
| `app/` + root | 10 | 3,664 | update.rs (1,530) | Monolithic struct (24 fields), 7-level nesting in DataFeeds handler |
| `screen/` | 12 | 7,142 | tickers_table.rs (1,475) | pane/view.rs 760-line content-type switch, dashboard 1,194 lines |
| `modal/` | 21 | ~5,500 | data_feeds.rs (1,377) | Zero keyboard nav, status colors hardcoded 5x, no ModalShell pattern |
| `chart/` | 56 | ~8,000+ | candlestick/mod.rs (859) | draw_clusters 368 lines with 30-50% duplication, studies unwired |
| `widget/` | 8 | 5,226 | comparison.rs (1,845) | No module-level docs, chart constants in widget code |
| `style/` | 3 | 1,031 | container.rs (486) | No design token system, 16 button variants (4 redundant) |
| **TOTAL** | **112** | **~25,000+** | | |

### Top Problems (Priority Order)

1. **update.rs (1,530 lines)** — DataFeeds handler alone is 300+ lines with 7-level nesting
2. **No design token system** — ~85 hardcoded spacing/sizing values scattered across views
3. **Status colors duplicated 5x** — Same RGB literals in connections_menu.rs and data_feeds.rs
4. **No keyboard navigation** — Zero Escape-to-close, zero Tab navigation in any modal
5. **draw_clusters() (368 lines)** — 30-50% duplication between cluster rendering modes
6. **comparison.rs (1,845 lines)** — Largest widget file, should be 4 modules
7. **Monolithic Flowsurface struct** — 24 fields mixing concerns (services, modals, preferences)
8. **Dead code** — 5 study modules + 5 perf modules implemented but not wired

### What's Already Good

- 100% palette-driven colors in style/ (no hardcoded hex)
- Clean trait-based chart engine (Chart, PlotConstants)
- EnumMap indicator storage (zero-cost indexing)
- Four-tier canvas cache system (main, x_labels, y_labels, crosshair)
- Proper Arc<>/Arc<Mutex<>> service wrapping with async-aware mutexes
- Shared interaction handler (canvas_interaction)
- Functional style system with `|theme, status|` closures
- Well-designed virtual scrolling in tickers_table
- Canvas-based panels (ladder, timeandsales) are self-contained and performant
- Trait-based indicator system with Box<dyn> polymorphism

---

## 2. Proposed File Structure

### Current to Proposed

```
src/
|-- main.rs                          # KEEP (35 lines)
|-- window.rs                        # KEEP (133 lines)
|-- logger.rs                        # KEEP (203 lines)
|-- audio.rs                         # KEEP (222 lines)
|-- layout.rs                        # KEEP (204 lines)
|
|-- app/                             # App shell (restructure)
|   |-- mod.rs                       # Flowsurface struct (slim: fields + new + theme + scale)
|   |-- message.rs                   # NEW -- Message enum + sub-message types
|   |-- update/                      # NEW -- split update.rs by domain
|   |   |-- mod.rs                   #   dispatch to sub-modules
|   |   |-- chart.rs                 #   LoadChartData, ChartDataLoaded, ReplayEvent
|   |   |-- feeds.rs                 #   DataFeeds, ConnectionsMenu, RithmicConnected
|   |   |-- download.rs              #   EstimateDataCost, DownloadData, DataDownloadProgress
|   |   |-- options.rs               #   LoadOptionChain, LoadGexProfile, results
|   |   |-- navigation.rs            #   WindowEvent, GoBack, ExitRequested, Tick
|   |   +-- preferences.rs           #   ThemeSelected, ScaleFactorChanged, SetTimezone
|   |-- services.rs                  # KEEP (202 lines)
|   |-- subscriptions.rs             # KEEP (103 lines)
|   +-- state.rs                     # KEEP (228 lines)
|
|-- theme/                           # NEW -- extracted from style/
|   |-- mod.rs                       #   Re-exports, theme type
|   |-- tokens.rs                    #   Design tokens: spacing, sizing, typography, radii
|   |-- palette.rs                   #   Semantic color helpers, status colors
|   |-- button.rs                    #   MOVE from style/button.rs (consolidated)
|   |-- container.rs                 #   MOVE from style/container.rs
|   +-- icon.rs                      #   MOVE Icon enum + icon_text() from style/mod.rs
|
|-- component/                       # NEW -- production-ready UI component library
|   |-- mod.rs                       #   Top-level re-exports
|   |
|   |-- primitives/                  #   Atomic building blocks
|   |   |-- mod.rs                   #     Re-exports
|   |   |-- label.rs                 #     Themed text: heading(), body(), caption(), mono()
|   |   |-- badge.rs                 #     Small status/count badges (pill-shaped)
|   |   |-- icon.rs                  #     MOVE Icon enum + icon_text() from style/mod.rs
|   |   |-- icon_button.rs           #     Icon + optional tooltip button helper
|   |   |-- separator.rs             #     Themed horizontal/vertical rules + spacers
|   |   +-- truncated_text.rs        #     Text with .wrapping(None) + max_width
|   |
|   |-- input/                       #   Input controls
|   |   |-- mod.rs                   #     Re-exports
|   |   |-- text_field.rs            #     Label + text_input + validation + error msg
|   |   |-- secure_field.rs          #     Password/API key input with visibility toggle
|   |   |-- numeric_field.rs         #     MOVE numeric_input_box from widget/mod.rs
|   |   |-- search_field.rs          #     Search icon + text_input + clear button
|   |   |-- dropdown.rs              #     Themed pick_list wrapper with label
|   |   |-- multi_select.rs          #     Multi-checkbox dropdown panel
|   |   |-- combo_select.rs          #     Searchable dropdown (Iced combo_box)
|   |   |-- checkbox_field.rs        #     Themed checkbox with label + optional tooltip
|   |   |-- radio_group.rs           #     Radio button group in row/column layout
|   |   |-- toggle_button.rs         #     Button that toggles on/off state
|   |   |-- toggle_switch.rs         #     Iced toggler with themed styling
|   |   |-- slider_field.rs          #     MOVE labeled_slider from widget/mod.rs
|   |   |-- stepper.rs               #     [-] value [+] increment control
|   |   +-- color_picker.rs          #     MOVE HSV color picker from widget/color_picker.rs
|   |
|   |-- display/                     #   Data display & feedback
|   |   |-- mod.rs                   #     Re-exports
|   |   |-- status_dot.rs            #     Colored connection status indicator
|   |   |-- status_badge.rs          #     Status dot + label text
|   |   |-- progress_bar.rs          #     Themed progress bar with optional label
|   |   |-- loading_status.rs        #     Downloading/Building/Error state display
|   |   |-- key_value.rs             #     "Label: Value" display pair
|   |   |-- empty_state.rs           #     Centered icon + message + optional CTA
|   |   |-- toast.rs                 #     MOVE toast manager from widget/toast.rs
|   |   +-- tooltip.rs               #     MOVE tooltip/tooltip_with_delay from widget/mod.rs
|   |
|   |-- layout/                      #   Layout composition
|   |   |-- mod.rs                   #     Re-exports
|   |   |-- card.rs                  #     Generic card container (border + padding + bg)
|   |   |-- interactive_card.rs      #     Clickable/selectable card with hover state
|   |   |-- section_header.rs        #     Section heading with optional trailing control
|   |   |-- collapsible.rs           #     Expand/collapse section with arrow toggle
|   |   |-- split_section.rs         #     MOVE split_column! macro from widget/mod.rs
|   |   |-- list_item.rs             #     Selectable list row (feed items, ticker rows)
|   |   |-- reorderable_list.rs      #     Drag-to-reorder (wraps widget/column_drag.rs)
|   |   |-- toolbar.rs               #     Horizontal toolbar with icon buttons
|   |   |-- button_group.rs          #     Grouped buttons (tabs, segmented controls)
|   |   +-- button_grid.rs           #     Grid of buttons (link group modal pattern)
|   |
|   |-- overlay/                     #   Overlays and modals
|   |   |-- mod.rs                   #     Re-exports
|   |   |-- modal_shell.rs           #     Base modal (backdrop + container + Escape)
|   |   |-- confirm_dialog.rs        #     MOVE confirm_dialog_container from widget/mod.rs
|   |   |-- form_modal.rs            #     Modal with form body + Save/Cancel footer
|   |   |-- dropdown_menu.rs         #     Positioned dropdown menu overlay
|   |   +-- context_menu.rs          #     Right-click context menu (pane title bar)
|   |
|   +-- form/                        #   Form composition helpers
|       |-- mod.rs                   #     Re-exports
|       |-- form_field.rs            #     Label + any control + validation wrapper
|       |-- form_row.rs              #     Horizontal label: control layout
|       +-- form_section.rs          #     Grouped fields with section header + divider
|
|-- screen/                          # Views (restructure pane/view.rs)
|   |-- mod.rs                       # KEEP
|   +-- dashboard/
|       |-- mod.rs                   # KEEP (1,194 lines -- large but cohesive orchestration)
|       |-- sidebar.rs               # KEEP (257 lines, clean)
|       |-- tickers_table.rs         # KEEP (1,475 lines -- self-contained virtual scroll)
|       |-- pane/
|       |   |-- mod.rs               # KEEP -- pane state machine
|       |   |-- content.rs           # KEEP -- content enum
|       |   |-- effects.rs           # KEEP (22 lines)
|       |   |-- helpers.rs           # KEEP (76 lines)
|       |   +-- view/                # NEW -- split pane/view.rs by content type
|       |       |-- mod.rs           #   compose_stack_view + title_bar (shared)
|       |       |-- kline.rs         #   Kline view composition
|       |       |-- heatmap.rs       #   Heatmap view composition
|       |       |-- comparison.rs    #   Comparison view composition
|       |       +-- starter.rs       #   Starter placeholder view
|       +-- panel/
|           |-- mod.rs               # KEEP
|           |-- ladder.rs            # KEEP (1,250 lines, canvas-based, self-contained)
|           +-- timeandsales.rs      # KEEP (607 lines, canvas-based, self-contained)
|
|-- modal/                           # Modals (minor restructure)
|   |-- mod.rs                       # KEEP
|   |-- audio.rs                     # KEEP
|   |-- drawing_tools.rs             # KEEP
|   |-- layout_manager.rs            # KEEP
|   |-- theme_editor.rs              # KEEP
|   +-- pane/
|       |-- mod.rs                   # KEEP
|       |-- calendar.rs              # KEEP
|       |-- connections_menu.rs      # KEEP (refactor status colors -> theme/palette.rs)
|       |-- data_feeds/              # NEW -- split 1,377-line file
|       |   |-- mod.rs               #   DataFeedsModal struct + Message + update
|       |   |-- view.rs              #   view_left_panel, view_right_panel, view_edit_form
|       |   +-- preview.rs           #   Historical data preview (price chart + trade table)
|       |-- data_management.rs       # KEEP
|       |-- historical_download.rs   # KEEP
|       |-- indicators.rs            # KEEP
|       |-- mini_tickers_list.rs     # KEEP
|       |-- stream.rs                # KEEP
|       +-- settings/                # KEEP (clean sub-structure)
|           |-- mod.rs
|           |-- common.rs
|           |-- kline.rs
|           |-- heatmap.rs
|           |-- comparison.rs
|           |-- study.rs
|           +-- panel.rs
|
|-- chart/                           # Chart engine (targeted refactors)
|   |-- mod.rs                       # KEEP
|   |-- core/                        # KEEP all (clean: traits, interaction, autoscale, view_state, caches)
|   |-- candlestick/
|   |   |-- mod.rs                   # KEEP
|   |   |-- render.rs                # KEEP
|   |   |-- candle.rs                # KEEP
|   |   |-- config.rs                # KEEP
|   |   |-- footprint.rs             # REFACTOR -- extract cluster rendering template
|   |   +-- cluster.rs              # NEW -- shared cluster rendering (~140 lines)
|   |-- heatmap/                     # KEEP all (mod, render, data, trades)
|   |-- comparison/                  # KEEP all (mod, series)
|   |-- display/                     # KEEP all (mod, footprint, heatmap)
|   |-- drawing/                     # KEEP all (mod, drawing, manager, point, render)
|   |-- indicator/                   # KEEP all (clean trait-based design)
|   |-- overlay/                     # KEEP all (crosshair, ruler, grid, gaps, last_price)
|   |-- scale/                       # KEEP all (mod, linear, timeseries)
|   |-- study/                       # WIRE UP (see S7)
|   +-- perf/                        # KEEP (wire LOD, feature-flag debug, see S7)
|
+-- widget/                          # Custom widgets (split comparison)
    |-- mod.rs                       # KEEP
    |-- chart/
    |   |-- mod.rs                   # KEEP
    |   +-- comparison/              # NEW -- split 1,845-line file
    |       |-- mod.rs               #   LineComparison widget + Widget trait impl
    |       |-- scene.rs             #   compute_scene, PlotContext, domain calculations
    |       |-- render.rs            #   fill_* rendering helpers (geometry)
    |       +-- legend.rs            #   Legend layout, hit-testing, icon positioning
    |-- color_picker.rs              # KEEP (429 lines, clean)
    |-- column_drag.rs               # KEEP (606 lines, at limit)
    |-- decorate.rs                  # KEEP (805 lines, trait boilerplate)
    |-- multi_split.rs               # KEEP (338 lines, clean)
    +-- toast.rs                     # KEEP (530 lines, clean)
```

### File Count Change

- **Before**: 112 files
- **After**: ~170 files (net +58: ~44 component files + 14 from file splits)
- **Component library**: 44 files across 6 categories (primitives, input, display, layout, overlay, form)
- **Largest file after**: tickers_table.rs (1,475) -- self-contained, acceptable
- **No file over 900 lines** except tickers_table.rs (self-contained virtual scroll) and ladder.rs (self-contained canvas panel)
- **Average component file**: 50-150 lines (focused, single-responsibility)

---

## 3. Design Token System

### 3.1 Token File: `src/theme/tokens.rs`

```rust
//! Design tokens -- single source of truth for all visual constants.
//! All view code should reference these instead of magic numbers.

// --- Spacing (4px base grid) ---
pub mod spacing {
    pub const XXXS: f32 = 1.0;   // Hairline (dividers)
    pub const XXS: f32 = 2.0;    // Tight (drag margins)
    pub const XS: f32 = 4.0;     // Compact (icon padding, tight rows)
    pub const SM: f32 = 6.0;     // Small (button internal padding)
    pub const MD: f32 = 8.0;     // Default (row spacing, section gaps)
    pub const LG: f32 = 12.0;    // Comfortable (form field spacing)
    pub const XL: f32 = 16.0;    // Generous (card padding)
    pub const XXL: f32 = 24.0;   // Spacious (modal padding, section breaks)
    pub const XXXL: f32 = 32.0;  // Page-level (outer margins)
}

// --- Typography ---
pub mod text {
    pub const TINY: f32 = 10.0;   // Badges, labels
    pub const SMALL: f32 = 11.0;  // Chart labels, panel data (AZERET_MONO)
    pub const BODY: f32 = 12.0;   // Default UI text
    pub const LABEL: f32 = 13.0;  // Form labels, section headers
    pub const TITLE: f32 = 14.0;  // Dialog titles, prominent text
    pub const HEADING: f32 = 16.0; // Modal headings
}

// --- Border Radii ---
pub mod radius {
    pub const NONE: f32 = 0.0;
    pub const SM: f32 = 2.0;    // Inputs, scrollbars
    pub const MD: f32 = 4.0;    // Buttons, containers, modals (default)
    pub const LG: f32 = 6.0;    // Emphasized panels
    pub const ROUND: f32 = 16.0; // Circles, pills
}

// --- Border Widths ---
pub mod border {
    pub const NONE: f32 = 0.0;
    pub const THIN: f32 = 1.0;    // Standard borders
    pub const MEDIUM: f32 = 1.5;  // Emphasized (confirm modals)
    pub const THICK: f32 = 2.0;   // Active state, scrollbars
}

// --- Shadows ---
pub mod shadow {
    pub const NONE: f32 = 0.0;
    pub const SM: f32 = 2.0;     // Minimal (modal containers)
    pub const MD: f32 = 4.0;     // Subtle (drag rows)
    pub const LG: f32 = 8.0;     // Dropdowns
    pub const XL: f32 = 12.0;    // Chart modals, confirm dialogs
    pub const XXL: f32 = 20.0;   // Dashboard modals (deepest)
}

// --- Layout Constants ---
pub mod layout {
    pub const TITLE_BAR_HEIGHT: f32 = 32.0;
    pub const SIDEBAR_WIDTH: f32 = 32.0;
    pub const SIDEBAR_BUTTON_HEIGHT: f32 = 34.0;
    pub const TICKER_CARD_HEIGHT: f32 = 56.0;
    pub const TICKER_CARD_EXPANDED: f32 = 64.0;
    pub const PANEL_ROW_HEIGHT: f32 = 16.0;    // Ladder
    pub const PANEL_ROW_HEIGHT_SM: f32 = 14.0;  // TimeAndSales
    pub const MIN_PANEL_HEIGHT: f32 = 40.0;
    pub const DRAG_HANDLE_WIDTH: f32 = 14.0;
    pub const SCROLLBAR_WIDTH: f32 = 4.0;
    pub const MODAL_MAX_WIDTH: u32 = 650;
}

// --- Chart Rendering ---
pub mod chart {
    pub const Y_AXIS_GUTTER: f32 = 66.0;
    pub const X_AXIS_HEIGHT: f32 = 24.0;
    pub const MIN_X_TICK_PX: f32 = 80.0;
    pub const CLUSTER_BAR_WIDTH_FACTOR: f32 = 0.9;
    pub const CLUSTER_SPACING_PX: f32 = 1.0;
    pub const TEXT_ALPHA_VISIBLE: f32 = 0.8;
    pub const ZOOM_SENSITIVITY: f32 = 30.0;
    pub const ZOOM_BASE: f32 = 2.0;
    pub const ZOOM_STEP_PCT: f32 = 0.05;
    pub const GAP_BREAK_MULTIPLIER: f32 = 3.0;
}

// --- Alpha Scale ---
pub mod alpha {
    pub const FAINT: f32 = 0.2;     // Disabled, dark-theme hints
    pub const SUBTLE: f32 = 0.3;    // Faint backgrounds
    pub const LIGHT: f32 = 0.4;     // Cards, weak shadows
    pub const MEDIUM: f32 = 0.5;    // Pane grids
    pub const STRONG: f32 = 0.6;    // Mid-tone backgrounds
    pub const HEAVY: f32 = 0.8;     // Heavy shadows, dashed lines
    pub const OPAQUE: f32 = 0.99;   // Modal backgrounds (near-opaque)
}
```

### 3.2 Semantic Status Colors: `src/theme/palette.rs`

Extract the 5x duplicated status colors:

```rust
use iced::Color;

/// Connection status colors -- single source of truth.
/// Previously hardcoded in connections_menu.rs:126-134 and data_feeds.rs:793-797.
pub fn status_color(status: ConnectionStatus) -> Color {
    match status {
        ConnectionStatus::Connected   => Color::from_rgb(0.2, 0.8, 0.2),
        ConnectionStatus::Connecting  => Color::from_rgb(0.9, 0.7, 0.1),
        ConnectionStatus::Downloading => Color::from_rgb(0.3, 0.6, 1.0),
        ConnectionStatus::Error       => Color::from_rgb(0.9, 0.2, 0.2),
        ConnectionStatus::Disconnected => Color::from_rgb(0.5, 0.5, 0.5),
    }
}
```

### 3.3 Adoption Strategy

**Not a big-bang rewrite.** Replace hardcoded values file-by-file during each migration phase:

1. Create `theme/tokens.rs` with all constants
2. Each time a file is touched for other refactoring, replace its magic numbers
3. Add a convention: "no f32 literals in view code outside tokens.rs"

---

## 4. Reusable Component Library

> **Design goals**: Every component is a plain function returning `Element<'a, Message>` (or a
> builder struct with `.into()` / `.view()`). No special traits, no framework — just composable
> Iced elements themed through `tokens::` and `theme/palette.rs`.

### Current State Audit

From exhaustive review of all 112 src/ files:

| Primitive Pattern | Instances Found | Files | Current Status |
|---|---|---|---|
| text_input (plain) | 6 | 4 files | Hand-rolled, inconsistent sizing |
| text_input (secure) | 5 | 2 files | `.secure(true)` with no visibility toggle |
| text_input (validated) | 2 | 2 files | Custom border color logic inline |
| text_input (search) | 2 | 1 file | No search icon, no clear button |
| pick_list (dropdown) | 12 | 8 files | Bare pick_list, no label wrapper |
| combo_box (searchable) | 0 | — | **Not used** (available in Iced) |
| checkbox | 11 | 6 files | Bare checkbox, inconsistent label sizing |
| radio group | 3 | 2 files | Hand-rolled row of radio widgets |
| toggler | 0 | — | **Not used** (available in Iced) |
| slider (labeled) | 10 | 5 files | `labeled_slider()` exists in widget/mod.rs |
| numeric_input_box | 1 | 1 file | Exists in widget/mod.rs, rarely reused |
| color_picker | 1 | 1 file | Full widget in widget/color_picker.rs |
| button (icon+tooltip) | 40+ | 12 files | `button_with_tooltip()` exists but verbose |
| button (toggle state) | 15+ | 8 files | Manual `is_active` flag per call site |
| status dot (colored) | 5 | 2 files | Hardcoded RGB, 8x8 container with radius |
| progress_bar | 2 | 2 files | Bare progress_bar, no label |
| loading status text | 4 | 2 files | Inline format!() with match arms |
| form field (label+input) | 25+ | 10 files | `column![text(), text_input()].spacing(4)` |
| card container | 10+ | 5 files | Hand-rolled container + border + padding |
| section header | 3+ | 2 files | Local `section_header()` fn, not shared |
| modal backdrop | 15+ | 8 files | `stack![base, opaque(mouse_area(...))]` |
| confirm dialog | 3 | 2 files | `confirm_dialog_container()` in widget/ |
| toast notification | per-pane | 1 widget | `widget::toast::Manager` (well-built) |
| tooltip | 15+ | 6 files | `tooltip()` + `tooltip_with_delay()` in widget/ |
| draggable list | 3 | 2 files | `column_drag::Column` + `dragger_row()` |
| split_column! dividers | 5+ | 3 files | Macro in widget/mod.rs |
| key:value display | 8+ | 3 files | `row![text(label), Space, text(value)]` |
| empty state | 2 | 2 files | `center(text("Waiting..."))` |
| tab/segmented control | 2 | 2 files | Manual button row with active state |

---

### 4.1 Primitives (`component/primitives/`)

#### 4.1.1 Label (`primitives/label.rs`)

**Problem**: 200+ `text().size(N)` calls with inconsistent sizes (8px–16px). No semantic meaning attached to text hierarchy.

```rust
use crate::theme::tokens::text as sz;

/// Heading text (16px) -- modal titles, prominent headings.
pub fn heading<'a>(content: impl ToString) -> Text<'a> {
    text(content).size(sz::HEADING)
}

/// Title text (14px) -- dialog titles, card headings.
pub fn title<'a>(content: impl ToString) -> Text<'a> {
    text(content).size(sz::TITLE)
}

/// Label text (13px) -- form labels, section headers.
pub fn label<'a>(content: impl ToString) -> Text<'a> {
    text(content).size(sz::LABEL)
}

/// Body text (12px) -- default UI text.
pub fn body<'a>(content: impl ToString) -> Text<'a> {
    text(content).size(sz::BODY)
}

/// Small text (11px) -- secondary info, chart labels.
pub fn small<'a>(content: impl ToString) -> Text<'a> {
    text(content).size(sz::SMALL)
}

/// Tiny text (10px) -- badges, timestamps, tertiary info.
pub fn tiny<'a>(content: impl ToString) -> Text<'a> {
    text(content).size(sz::TINY)
}

/// Monospace text (11px AZERET_MONO) -- prices, data values.
pub fn mono<'a>(content: impl ToString) -> Text<'a> {
    text(content).size(sz::SMALL).font(style::AZERET_MONO)
}

/// Colored text with theme-aware styling.
pub fn colored<'a>(
    content: impl ToString,
    color_fn: impl Fn(&Theme) -> Color + 'a,
) -> Element<'a, impl 'a> {
    text(content).style(move |theme: &Theme| text::Style {
        color: Some(color_fn(theme)),
    })
}
```

**Replaces**: 200+ bare `text().size(N)` calls. Provides semantic naming so sizes stay consistent.

#### 4.1.2 Badge (`primitives/badge.rs`)

**Problem**: No pill-shaped count/status badges exist. Currently hand-rolled with container + radius.

```rust
pub enum BadgeKind { Primary, Success, Warning, Danger, Neutral }

/// Small pill-shaped badge with colored background.
/// Used for: counts, status labels, tags.
pub fn badge<'a, Message: 'a>(
    content: impl ToString,
    kind: BadgeKind,
) -> Element<'a, Message>;
```

**New component** -- supports ticker card change indicators, notification counts.

#### 4.1.3 Icon & IconButton (`primitives/icon.rs`, `primitives/icon_button.rs`)

**MOVE** `Icon` enum (40+ variants), `icon_text()`, `exchange_icon()` from `style/mod.rs`.

```rust
// icon.rs -- MOVED from style/mod.rs
pub enum Icon { /* 40+ variants unchanged */ }
pub fn icon_text(icon: Icon, size: u16) -> Text<'static>;
pub fn exchange_icon(venue: FuturesVenue) -> Icon;

// icon_button.rs -- consolidates button_with_tooltip + common icon patterns
/// Icon button with optional tooltip. Replaces 40+ hand-rolled instances.
pub fn icon_button<'a, Message: Clone + 'a>(
    icon: Icon,
    size: u16,
    on_press: Option<Message>,
) -> IconButtonBuilder<'a, Message>;

pub struct IconButtonBuilder<'a, Message> { /* ... */ }

impl<'a, Message: Clone + 'a> IconButtonBuilder<'a, Message> {
    /// Add tooltip text (shown on hover).
    pub fn tooltip(self, text: &'a str, position: tooltip::Position) -> Self;
    /// Set tooltip delay (default: 500ms).
    pub fn tooltip_delay(self, delay: Duration) -> Self;
    /// Set button style function.
    pub fn style(self, f: impl Fn(&Theme, button::Status) -> button::Style + 'a) -> Self;
    /// Set active/toggled state (for toolbar toggles).
    pub fn active(self, is_active: bool) -> Self;
    /// Set explicit width/height (default: auto).
    pub fn size_box(self, width: f32, height: f32) -> Self;
    /// Set padding.
    pub fn padding(self, padding: impl Into<Padding>) -> Self;
    /// Build into Element.
    pub fn into_element(self) -> Element<'a, Message>;
}

// Convenience for the most common case:
/// Toolbar-style icon button (transparent, 12px icon, Bottom tooltip).
pub fn toolbar_icon<'a, Message: Clone + 'a>(
    icon: Icon,
    tooltip_text: &'a str,
    on_press: Option<Message>,
    is_active: bool,
) -> Element<'a, Message>;
```

**Replaces**: 40+ `button(icon_text(...)).style(transparent).on_press(...)` + manual tooltip wrapping. Current `button_with_tooltip()` in widget/mod.rs has a rigid signature that forces style as a parameter; the builder pattern is more ergonomic.

#### 4.1.4 Separator (`primitives/separator.rs`)

**Problem**: 15+ `rule::horizontal(1).style(style::split_ruler)` scattered everywhere.

```rust
/// Themed horizontal rule divider.
pub fn divider<'a, Message: 'a>() -> Element<'a, Message>;

/// Heavier divider (2px) for major section breaks.
pub fn thick_divider<'a, Message: 'a>() -> Element<'a, Message>;

/// Vertical rule divider.
pub fn vertical_divider<'a, Message: 'a>(height: Length) -> Element<'a, Message>;

/// Flexible spacer (equivalent to Space with Length::Fill).
pub fn flex_space() -> Space;
```

**Replaces**: 15+ inline rule constructions.

#### 4.1.5 TruncatedText (`primitives/truncated_text.rs`)

**Problem**: `text(name).size(12).wrapping(None)` pattern used in constrained spaces (connection names, ticker symbols) with no ellipsis handling.

```rust
/// Text that clips to a max width without wrapping.
pub fn truncated<'a>(
    content: impl ToString,
    max_width: f32,
) -> Element<'a, impl 'a>;
```

---

### 4.2 Input Controls (`component/input/`)

#### 4.2.1 TextField (`input/text_field.rs`)

**Problem**: 25+ instances of `column![text("Label").size(12), text_input(...)].spacing(4)` with inconsistent sizing, no validation feedback.

```rust
/// Standard text field with label, placeholder, and optional validation.
pub fn text_field<'a, Message: 'a>(
    label_text: &str,
    placeholder: &str,
    value: &str,
    on_input: impl Fn(String) -> Message + 'a,
) -> TextFieldBuilder<'a, Message>;

pub struct TextFieldBuilder<'a, Message> { /* ... */ }

impl<'a, Message: 'a> TextFieldBuilder<'a, Message> {
    /// Mark as invalid (shows danger border + error message).
    pub fn validate(self, is_valid: bool) -> Self;
    /// Show validation error message below the input.
    pub fn error_message(self, msg: &'a str) -> Self;
    /// Enable on_submit callback (Enter key).
    pub fn on_submit(self, msg: Message) -> Self;
    /// Set width (default: Length::Fill).
    pub fn width(self, width: impl Into<Length>) -> Self;
    /// Set text size (default: tokens::text::LABEL).
    pub fn text_size(self, size: f32) -> Self;
    /// Set input ID for focus management.
    pub fn id(self, id: impl Into<widget::Id>) -> Self;
    /// Build into Element.
    pub fn into_element(self) -> Element<'a, Message>;
}
```

**Replaces**: 25+ hand-rolled label+input columns across data_feeds.rs, historical_download.rs, layout_manager.rs, theme_editor.rs.

#### 4.2.2 SecureField (`input/secure_field.rs`)

**Problem**: 5 instances of `.secure(true)` text inputs for passwords/API keys, with no visibility toggle and no "key set" indicator.

```rust
/// Password/API key field with secure input and optional visibility toggle.
pub fn secure_field<'a, Message: 'a>(
    label_text: &str,
    placeholder: &str,
    value: &str,
    on_input: impl Fn(String) -> Message + 'a,
) -> SecureFieldBuilder<'a, Message>;

pub struct SecureFieldBuilder<'a, Message> { /* ... */ }

impl<'a, Message: 'a> SecureFieldBuilder<'a, Message> {
    /// Show "key is set" indicator when value is non-empty but hidden.
    pub fn show_set_indicator(self, is_set: bool) -> Self;
    /// Add a reveal/hide toggle button.
    pub fn with_visibility_toggle(self, is_visible: bool, on_toggle: Message) -> Self;
    /// Set width (default: Length::Fill).
    pub fn width(self, width: impl Into<Length>) -> Self;
    pub fn into_element(self) -> Element<'a, Message>;
}
```

**Replaces**: 5 secure text_input patterns in data_feeds.rs (API key, password) and historical_download.rs (API key).

#### 4.2.3 NumericField (`input/numeric_field.rs`)

**MOVE** `numeric_input_box()` from `widget/mod.rs` with improvements.

```rust
/// Numeric input with validation and formatted display.
pub fn numeric_field<'a, Message: 'a>(
    label_text: &str,
    placeholder: &str,
    raw_buf: &str,
    is_valid: bool,
    on_input: impl Fn(String) -> Message + 'a,
) -> NumericFieldBuilder<'a, Message>;

pub struct NumericFieldBuilder<'a, Message> { /* ... */ }

impl<'a, Message: 'a> NumericFieldBuilder<'a, Message> {
    /// Add submit callback (Enter key).
    pub fn on_submit(self, msg: Message) -> Self;
    /// Set width (default: auto).
    pub fn width(self, width: impl Into<Length>) -> Self;
    pub fn into_element(self) -> Element<'a, Message>;
}
```

**Replaces**: `numeric_input_box()` in widget/mod.rs (1 call site, but standardizes the pattern).

#### 4.2.4 SearchField (`input/search_field.rs`)

**Problem**: 2 search inputs in tickers_table.rs use a plain text_input with no search icon and no clear button.

```rust
/// Search input with search icon prefix and clear button.
pub fn search_field<'a, Message: Clone + 'a>(
    placeholder: &str,
    value: &str,
    on_input: impl Fn(String) -> Message + 'a,
    on_clear: Message,
) -> SearchFieldBuilder<'a, Message>;

pub struct SearchFieldBuilder<'a, Message> { /* ... */ }

impl<'a, Message: Clone + 'a> SearchFieldBuilder<'a, Message> {
    /// Set input ID for focus management.
    pub fn id(self, id: impl Into<widget::Id>) -> Self;
    /// Set width.
    pub fn width(self, width: impl Into<Length>) -> Self;
    pub fn into_element(self) -> Element<'a, Message>;
}
```

**Replaces**: 2 bare search text_input patterns in tickers_table.rs.

#### 4.2.5 Dropdown (`input/dropdown.rs`)

**Problem**: 12 pick_list usages with inconsistent text_size (11–13px) and no standard label.

```rust
/// Themed dropdown (wraps pick_list) with label and consistent styling.
pub fn dropdown<'a, T, Message>(
    label_text: &str,
    options: impl Into<Cow<'a, [T]>>,
    selected: Option<T>,
    on_selected: impl Fn(T) -> Message + 'a,
) -> DropdownBuilder<'a, T, Message>
where
    T: ToString + PartialEq + Clone + 'a;

pub struct DropdownBuilder<'a, T, Message> { /* ... */ }

impl<'a, T, Message> DropdownBuilder<'a, T, Message> {
    /// Set text size (default: tokens::text::LABEL).
    pub fn text_size(self, size: f32) -> Self;
    /// Set width (default: Length::Fill).
    pub fn width(self, width: impl Into<Length>) -> Self;
    /// Set placeholder text when nothing is selected.
    pub fn placeholder(self, text: &'a str) -> Self;
    pub fn into_element(self) -> Element<'a, Message>;
}
```

**Replaces**: 12 bare pick_list calls with inconsistent styling.

#### 4.2.6 MultiSelect (`input/multi_select.rs`)

**Problem**: No multi-select dropdown exists. Audio modal and indicator selection use checkbox lists in scrollable containers -- works but not reusable.

```rust
/// Multi-select dropdown panel with checkboxes.
pub fn multi_select<'a, T, Message>(
    label_text: &str,
    options: &'a [T],
    selected: &'a [T],
    on_toggle: impl Fn(T, bool) -> Message + 'a,
) -> Element<'a, Message>
where
    T: ToString + PartialEq + Clone + 'a;
```

**New component** -- standardizes indicator selection (modal/pane/indicators.rs), audio ticker selection, study selection patterns.

#### 4.2.7 ComboSelect (`input/combo_select.rs`)

**Problem**: No searchable dropdown exists despite Iced providing `combo_box`. Ticker selection in data_management.rs and historical_download.rs would benefit from search-as-you-type.

```rust
/// Searchable dropdown (wraps Iced combo_box) with label.
pub fn combo_select<'a, T, Message>(
    label_text: &str,
    options: &'a [T],
    selected: Option<&T>,
    on_selected: impl Fn(T) -> Message + 'a,
    on_input: impl Fn(String) -> Message + 'a,
    input_value: &str,
) -> Element<'a, Message>
where
    T: ToString + Clone + 'a;
```

**New component** -- enables search-as-you-type for ticker dropdowns, schema selection.

#### 4.2.8 CheckboxField (`input/checkbox_field.rs`)

**Problem**: 11 checkbox usages with inconsistent label sizing and manual tooltip attachment.

```rust
/// Themed checkbox with label and optional tooltip.
pub fn checkbox_field<'a, Message: 'a>(
    label_text: &str,
    is_checked: bool,
    on_toggle: impl Fn(bool) -> Message + 'a,
) -> CheckboxFieldBuilder<'a, Message>;

pub struct CheckboxFieldBuilder<'a, Message> { /* ... */ }

impl<'a, Message: 'a> CheckboxFieldBuilder<'a, Message> {
    /// Add info tooltip (shows "i" icon with hover text).
    pub fn tooltip(self, text: &'a str) -> Self;
    /// Set text size (default: tokens::text::BODY).
    pub fn text_size(self, size: f32) -> Self;
    /// Set spacing between checkbox and label.
    pub fn spacing(self, spacing: f32) -> Self;
    pub fn into_element(self) -> Element<'a, Message>;
}
```

**Replaces**: 11 bare checkbox calls + 2 adjacent tooltip patterns in settings/panel.rs.

#### 4.2.9 RadioGroup (`input/radio_group.rs`)

**Problem**: 3 radio group patterns manually composed with `row![radio(...), radio(...)].spacing(12)`.

```rust
/// Radio button group with consistent spacing and layout.
pub fn radio_group<'a, T, Message>(
    options: &'a [(T, &'a str)],  // (value, label) pairs
    selected: Option<T>,
    on_selected: impl Fn(T) -> Message + 'a,
) -> RadioGroupBuilder<'a, T, Message>
where
    T: Copy + PartialEq + 'a;

pub struct RadioGroupBuilder<'a, T, Message> { /* ... */ }

impl<'a, T, Message> RadioGroupBuilder<'a, T, Message> {
    /// Layout direction (default: Row).
    pub fn direction(self, dir: Direction) -> Self;  // Row or Column
    /// Set spacing between radio buttons.
    pub fn spacing(self, spacing: f32) -> Self;
    pub fn into_element(self) -> Element<'a, Message>;
}
```

**Replaces**: 3 hand-rolled radio groups in settings/panel.rs and settings/heatmap.rs.

#### 4.2.10 ToggleButton (`input/toggle_button.rs`)

**Problem**: 15+ button instances that toggle between on/off state with manual `is_active` flag. Two different patterns: icon toggles (sidebar, toolbar) and text toggles (On/Off in data_feeds.rs).

```rust
/// Button with explicit on/off visual state.
pub fn toggle_button<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    is_on: bool,
    on_toggle: Message,
) -> ToggleButtonBuilder<'a, Message>;

pub struct ToggleButtonBuilder<'a, Message> { /* ... */ }

impl<'a, Message: Clone + 'a> ToggleButtonBuilder<'a, Message> {
    /// Set button style (default: transparent with active state).
    pub fn style(self, f: impl Fn(&Theme, button::Status, bool) -> button::Style + 'a) -> Self;
    /// Set padding.
    pub fn padding(self, padding: impl Into<Padding>) -> Self;
    /// Set width.
    pub fn width(self, width: impl Into<Length>) -> Self;
    pub fn into_element(self) -> Element<'a, Message>;
}
```

**Replaces**: 15+ manual button+is_active patterns.

#### 4.2.11 ToggleSwitch (`input/toggle_switch.rs`)

**Problem**: Iced's `toggler` widget not used at all despite being appropriate for boolean settings (auto-reconnect, enable caching). Currently using On/Off button toggles.

```rust
/// Themed toggle switch (wraps Iced toggler).
pub fn toggle_switch<'a, Message: 'a>(
    label_text: &str,
    is_on: bool,
    on_toggle: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message>;
```

**New component** -- replaces 2 "button(text('On'/'Off'))" patterns in data_feeds.rs with proper switch widget.

#### 4.2.12 SliderField (`input/slider_field.rs`)

**MOVE** `labeled_slider()` from `widget/mod.rs`. Also move `classic_slider_row()`.

```rust
/// Slider with label, value display, and optional step.
/// Moved from widget/mod.rs::labeled_slider.
pub fn slider_field<'a, T, Message: Clone + 'a>(
    label_text: &str,
    range: RangeInclusive<T>,
    value: T,
    on_change: impl Fn(T) -> Message + 'a,
) -> SliderFieldBuilder<'a, T, Message>
where
    T: Copy + Into<f64> + num_traits::FromPrimitive;

pub struct SliderFieldBuilder<'a, T, Message> { /* ... */ }

impl<'a, T, Message: Clone + 'a> SliderFieldBuilder<'a, T, Message> {
    /// Set step size.
    pub fn step(self, step: T) -> Self;
    /// Set value formatter (shown right-aligned).
    pub fn format(self, f: impl Fn(T) -> String + 'a) -> Self;
    /// Wrap in a card container (for settings modals).
    pub fn in_card(self) -> Self;
    pub fn into_element(self) -> Element<'a, Message>;
}
```

**Replaces**: 10 slider usages + `labeled_slider()` + `classic_slider_row()` from widget/mod.rs.

#### 4.2.13 Stepper (`input/stepper.rs`)

**Problem**: No increment/decrement control exists. Some numeric settings (levels count, row count) would benefit from [-] N [+] pattern.

```rust
/// Increment/decrement control: [-] value [+].
pub fn stepper<'a, Message: Clone + 'a>(
    value: i32,
    range: RangeInclusive<i32>,
    on_change: impl Fn(i32) -> Message + 'a,
) -> StepperBuilder<'a, Message>;

pub struct StepperBuilder<'a, Message> { /* ... */ }

impl<'a, Message: Clone + 'a> StepperBuilder<'a, Message> {
    /// Set label shown above.
    pub fn label(self, text: &'a str) -> Self;
    /// Set step size (default: 1).
    pub fn step(self, step: i32) -> Self;
    /// Set value formatter.
    pub fn format(self, f: impl Fn(i32) -> String + 'a) -> Self;
    pub fn into_element(self) -> Element<'a, Message>;
}
```

**New component** -- useful for ladder levels, time-and-sales row counts.

#### 4.2.14 ColorPicker (`input/color_picker.rs`)

**MOVE** `widget/color_picker.rs` here. Already well-built (HSV picker, 429 lines). Expose as component with builder.

```rust
/// HSV color picker with hue slider + saturation/value grid.
/// Moved from widget/color_picker.rs (unchanged internally).
pub fn color_picker<'a, Message: Clone + 'a>(
    color: Hsva,
    on_change: impl Fn(Hsva) -> Message + 'a,
) -> Element<'a, Message>;
```

---

### 4.3 Display Components (`component/display/`)

#### 4.3.1 StatusDot (`display/status_dot.rs`)

**Problem**: 5 instances of hardcoded RGB status colors with hand-built 8x8 containers across connections_menu.rs and data_feeds.rs.

```rust
/// Colored dot indicating connection status.
/// Colors from theme/palette.rs::status_color().
pub fn status_dot<'a, Message: 'a>(status: ConnectionStatus) -> Element<'a, Message>;

/// Status dot + label text.
pub fn status_badge<'a, Message: 'a>(
    status: ConnectionStatus,
    label_text: &str,
) -> Element<'a, Message>;

/// Status dot + label + optional detail text (e.g. error message).
pub fn status_row<'a, Message: 'a>(
    status: ConnectionStatus,
    label_text: &str,
    detail: Option<&str>,
) -> Element<'a, Message>;
```

**Replaces**: 5 hardcoded status dot patterns. All colors come from `theme::palette::status_color()`.

#### 4.3.2 ProgressBar (`display/progress_bar.rs`)

**Problem**: 2 progress_bar usages with no label and inconsistent girth.

```rust
/// Themed progress bar with optional label showing percentage or custom text.
pub fn themed_progress_bar<'a, Message: 'a>(
    progress: f32,   // 0.0..=1.0
) -> ProgressBarBuilder<'a, Message>;

pub struct ProgressBarBuilder<'a, Message> { /* ... */ }

impl<'a, Message: 'a> ProgressBarBuilder<'a, Message> {
    /// Show percentage text (e.g. "42%").
    pub fn show_percentage(self) -> Self;
    /// Show custom label.
    pub fn label(self, text: &'a str) -> Self;
    /// Set girth (default: 6.0).
    pub fn girth(self, girth: f32) -> Self;
    pub fn into_element(self) -> Element<'a, Message>;
}
```

**Replaces**: 2 bare progress_bar patterns in data_management.rs and historical_download.rs.

#### 4.3.3 LoadingStatus (`display/loading_status.rs`)

**Problem**: 4 inline `match loading_status { ... }` blocks that format "Downloading X (3/7)", "Building (42%)", etc.

```rust
/// Renders the current loading status as formatted text.
/// Handles: Downloading, LoadingFromCache, Building, Error states.
pub fn loading_status_display<'a, Message: 'a>(
    status: &LoadingStatus,
) -> Element<'a, Message>;
```

**Replaces**: 4 hand-rolled loading status format blocks in pane/view.rs and data_management.rs.

#### 4.3.4 KeyValue (`display/key_value.rs`)

**Problem**: 8+ instances of `row![text(label), Space, text(value)]` for displaying data pairs.

```rust
/// "Label: Value" display pair with consistent formatting.
pub fn key_value<'a, Message: 'a>(
    key: &str,
    value: &str,
) -> KeyValueBuilder<'a, Message>;

pub struct KeyValueBuilder<'a, Message> { /* ... */ }

impl<'a, Message: 'a> KeyValueBuilder<'a, Message> {
    /// Use monospace font for value.
    pub fn mono_value(self) -> Self;
    /// Color the value text.
    pub fn value_color(self, f: impl Fn(&Theme) -> Color + 'a) -> Self;
    /// Set key width (for alignment in a column of key-value pairs).
    pub fn key_width(self, width: impl Into<Length>) -> Self;
    pub fn into_element(self) -> Element<'a, Message>;
}
```

**Replaces**: 8+ inline key-value row constructions in tickers_table.rs, data_management.rs, historical_download.rs.

#### 4.3.5 EmptyState (`display/empty_state.rs`)

**Problem**: 2 scattered `center(text("Waiting for data...").size(16))` patterns. No consistent empty/placeholder state.

```rust
/// Centered placeholder for empty content areas.
pub fn empty_state<'a, Message: 'a>(
    message: &str,
) -> EmptyStateBuilder<'a, Message>;

pub struct EmptyStateBuilder<'a, Message> { /* ... */ }

impl<'a, Message: 'a> EmptyStateBuilder<'a, Message> {
    /// Add icon above the message.
    pub fn icon(self, icon: Icon) -> Self;
    /// Add call-to-action button below the message.
    pub fn action(self, label: &'a str, on_press: Message) -> Self;
    pub fn into_element(self) -> Element<'a, Message>;
}
```

**Replaces**: 2 inline empty state patterns + provides standard pattern for future use.

#### 4.3.6 Toast (`display/toast.rs`)

**MOVE** `widget/toast.rs` here as-is (530 lines, well-built). Already has `Manager`, `Toast`, `Notification`, `Status` types. No changes needed to internal implementation.

```rust
// Re-export from widget/toast.rs (moved here)
pub use self::toast::{Manager as ToastManager, Toast, Notification, Status as ToastStatus};
```

#### 4.3.7 Tooltip (`display/tooltip.rs`)

**MOVE** `tooltip()` and `tooltip_with_delay()` from `widget/mod.rs`.

```rust
/// Themed tooltip with immediate display.
/// Moved from widget/mod.rs::tooltip.
pub fn themed_tooltip<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    tooltip_text: Option<&'a str>,
    position: tooltip::Position,
) -> Element<'a, Message>;

/// Themed tooltip with configurable delay.
/// Moved from widget/mod.rs::tooltip_with_delay.
pub fn themed_tooltip_delayed<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    tooltip_text: Option<&'a str>,
    position: tooltip::Position,
    delay: Duration,
) -> Element<'a, Message>;
```

---

### 4.4 Layout Components (`component/layout/`)

#### 4.4.1 Card (`layout/card.rs`)

**Problem**: 10+ card patterns with inconsistent padding, borders, and backgrounds.

```rust
pub enum CardKind {
    Default,    // modal_container style (subtle border, light bg)
    Elevated,   // dashboard_modal style (shadow, stronger bg)
    Interactive, // ticker_card style (hover state, clickable)
}

/// Generic card container with consistent border, padding, background.
pub fn card<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
) -> CardBuilder<'a, Message>;

pub struct CardBuilder<'a, Message> { /* ... */ }

impl<'a, Message: 'a> CardBuilder<'a, Message> {
    /// Set card kind (default: Default).
    pub fn kind(self, kind: CardKind) -> Self;
    /// Set padding (default: tokens::spacing::XL).
    pub fn padding(self, padding: impl Into<Padding>) -> Self;
    /// Set max width.
    pub fn max_width(self, width: u32) -> Self;
    pub fn into_element(self) -> Element<'a, Message>;
}
```

#### 4.4.2 InteractiveCard (`layout/interactive_card.rs`)

**Problem**: Ticker cards, feed cards, and layout rows all need clickable card containers with selection state.

```rust
/// Clickable card with hover highlight and selection state.
pub fn interactive_card<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    on_press: Message,
) -> InteractiveCardBuilder<'a, Message>;

pub struct InteractiveCardBuilder<'a, Message> { /* ... */ }

impl<'a, Message: Clone + 'a> InteractiveCardBuilder<'a, Message> {
    /// Mark as selected (uses primary border/bg).
    pub fn selected(self, is_selected: bool) -> Self;
    /// Set height (default: auto).
    pub fn height(self, height: impl Into<Length>) -> Self;
    /// Set padding.
    pub fn padding(self, padding: impl Into<Padding>) -> Self;
    /// Add a colored left accent bar (ticker card pattern).
    pub fn accent_bar(self, color: Color) -> Self;
    pub fn into_element(self) -> Element<'a, Message>;
}
```

**Replaces**: ticker_card button styling in tickers_table.rs, feed item styling in data_feeds.rs, layout item styling in layout_manager.rs.

#### 4.4.3 SectionHeader (`layout/section_header.rs`)

**Problem**: 3+ local `section_header()` definitions, not shared.

```rust
/// Section heading with optional trailing control (button, count, etc).
pub fn section_header<'a, Message: 'a>(
    label_text: &str,
) -> SectionHeaderBuilder<'a, Message>;

pub struct SectionHeaderBuilder<'a, Message> { /* ... */ }

impl<'a, Message: 'a> SectionHeaderBuilder<'a, Message> {
    /// Add trailing element (button, count badge, etc).
    pub fn trailing(self, element: impl Into<Element<'a, Message>>) -> Self;
    /// Add bottom divider.
    pub fn with_divider(self) -> Self;
    pub fn into_element(self) -> Element<'a, Message>;
}
```

**Replaces**: 3+ local section_header functions.

#### 4.4.4 Collapsible (`layout/collapsible.rs`)

**Problem**: No expand/collapse sections exist despite settings modals having many grouped controls that could benefit from collapsing.

```rust
/// Expand/collapse section with animated arrow toggle.
pub fn collapsible<'a, Message: Clone + 'a>(
    header: &str,
    is_expanded: bool,
    on_toggle: Message,
    body: impl Into<Element<'a, Message>>,
) -> Element<'a, Message>;
```

**New component** -- useful for settings panels with many grouped options.

#### 4.4.5 SplitSection (`layout/split_section.rs`)

**MOVE** `split_column!` macro from `widget/mod.rs`.

```rust
/// Column with horizontal rule dividers between items.
/// Moved from widget/mod.rs::split_column! macro.
macro_rules! split_section { /* unchanged */ }
```

#### 4.4.6 ListItem (`layout/list_item.rs`)

**Problem**: Feed items, ticker rows, and layout items all need a consistent selectable list row pattern.

```rust
/// Selectable list row with consistent padding and hover state.
pub fn list_item<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    on_press: Message,
) -> ListItemBuilder<'a, Message>;

pub struct ListItemBuilder<'a, Message> { /* ... */ }

impl<'a, Message: Clone + 'a> ListItemBuilder<'a, Message> {
    /// Mark as selected.
    pub fn selected(self, is_selected: bool) -> Self;
    /// Add leading element (icon, status dot).
    pub fn leading(self, element: impl Into<Element<'a, Message>>) -> Self;
    /// Add trailing element (badge, button).
    pub fn trailing(self, element: impl Into<Element<'a, Message>>) -> Self;
    /// Set height (default: auto).
    pub fn height(self, height: impl Into<Length>) -> Self;
    pub fn into_element(self) -> Element<'a, Message>;
}
```

**Replaces**: 4+ list row patterns in data_feeds.rs, layout_manager.rs, indicators.rs.

#### 4.4.7 ReorderableList (`layout/reorderable_list.rs`)

Thin wrapper around `widget/column_drag.rs` + `dragger_row()`.

```rust
/// Drag-to-reorder list. Wraps column_drag::Column + dragger_row.
pub fn reorderable_list<'a, Message: Clone + 'a>(
    items: Vec<Element<'a, Message>>,
    is_editing: bool,
    on_drag: impl Fn(column_drag::DragEvent) -> Message + 'a,
) -> Element<'a, Message>;
```

**Replaces**: 3 conditional `column_drag::Column` + `dragger_row()` patterns.

#### 4.4.8 Toolbar (`layout/toolbar.rs`)

**Problem**: Multiple inline `row![btn1, btn2, btn3].spacing(4)` patterns for control bars.

```rust
/// Horizontal toolbar with icon buttons and optional separators.
pub fn toolbar<'a, Message: 'a>(
    items: Vec<ToolbarItem<'a, Message>>,
) -> Element<'a, Message>;

pub enum ToolbarItem<'a, Message> {
    Button(Element<'a, Message>),
    Separator,
    FlexSpace,
}
```

**Replaces**: pane title bar control buttons, drawing tools toolbar, sidebar button groups.

#### 4.4.9 ButtonGroup (`layout/button_group.rs`)

**Problem**: 2 tab/segmented control patterns manually composed with active state tracking.

```rust
/// Grouped buttons acting as tabs or segmented control.
pub fn button_group<'a, T, Message: Clone + 'a>(
    options: &'a [(T, &'a str)],
    selected: T,
    on_selected: impl Fn(T) -> Message + 'a,
) -> ButtonGroupBuilder<'a, T, Message>
where
    T: Copy + PartialEq + 'a;

pub struct ButtonGroupBuilder<'a, T, Message> { /* ... */ }

impl<'a, T, Message: Clone + 'a> ButtonGroupBuilder<'a, T, Message> {
    /// Use tab style (active/inactive tab appearance).
    pub fn tab_style(self) -> Self;
    /// Use segmented style (connected button group).
    pub fn segmented_style(self) -> Self;
    /// Set spacing between buttons.
    pub fn spacing(self, spacing: f32) -> Self;
    pub fn into_element(self) -> Element<'a, Message>;
}
```

**Replaces**: 2 tab patterns in stream.rs and settings, plus study profile kind toggles.

#### 4.4.10 ButtonGrid (`layout/button_grid.rs`)

**Problem**: Link group modal uses a hand-built 3-per-row button grid.

```rust
/// Grid of buttons with configurable columns.
pub fn button_grid<'a, T, Message: Clone + 'a>(
    items: &'a [T],
    columns: usize,
    selected: Option<T>,
    on_selected: impl Fn(T) -> Message + 'a,
    label_fn: impl Fn(&T) -> String + 'a,
) -> Element<'a, Message>
where
    T: Copy + PartialEq + 'a;
```

**Replaces**: Link group modal button grid in pane/helpers.rs.

---

### 4.5 Overlay Components (`component/overlay/`)

#### 4.5.1 ModalShell (`overlay/modal_shell.rs`)

**Problem**: 15+ modals hand-roll `stack![base, opaque(mouse_area(container(...)))]` with inconsistent padding, shadows, and no keyboard handling.

```rust
pub enum ModalKind {
    /// Chart settings overlay (blur 12px, 0.99 alpha bg, tight padding).
    Chart,
    /// Dashboard-level dialog (blur 20px, 0.99 alpha bg, generous padding).
    Dashboard,
    /// Confirmation dialog (primary border, blur 12px, offset shadow).
    Confirm,
}

/// Consistent modal wrapper: backdrop + styled container + optional header/footer.
pub struct ModalShell<'a, Message> {
    body: Element<'a, Message>,
    on_close: Message,
    title: Option<&'a str>,
    footer: Option<Element<'a, Message>>,
    kind: ModalKind,
    max_width: Option<u32>,
    max_height: Option<u32>,
    padding: Option<Padding>,
}

impl<'a, Message: Clone + 'a> ModalShell<'a, Message> {
    pub fn new(body: impl Into<Element<'a, Message>>, on_close: Message) -> Self;
    pub fn title(self, title: &'a str) -> Self;
    pub fn footer(self, footer: impl Into<Element<'a, Message>>) -> Self;
    pub fn kind(self, kind: ModalKind) -> Self;
    pub fn max_width(self, width: u32) -> Self;
    pub fn max_height(self, height: u32) -> Self;
    pub fn padding(self, padding: impl Into<Padding>) -> Self;

    /// Render: stack![base_content, backdrop, modal_container].
    /// Backdrop: opaque black (0.8 alpha) + mouse_area on_press(on_close).
    /// Container: styled per ModalKind + optional scrollable body.
    pub fn view(self, base: impl Into<Element<'a, Message>>) -> Element<'a, Message>;
}

// Convenience functions matching existing patterns:

/// Chart-level modal overlay. Replaces modal/pane/mod.rs::stack_modal.
pub fn chart_modal<'a, Message: Clone + 'a>(
    base: impl Into<Element<'a, Message>>,
    body: impl Into<Element<'a, Message>>,
    on_close: Message,
) -> Element<'a, Message>;

/// Dashboard-level modal. Replaces modal/mod.rs::dashboard_modal.
pub fn dashboard_modal<'a, Message: Clone + 'a>(
    base: impl Into<Element<'a, Message>>,
    body: impl Into<Element<'a, Message>>,
    on_close: Message,
    padding: impl Into<Padding>,
) -> Element<'a, Message>;
```

**Replaces**: 15+ hand-rolled stack+opaque+mouse_area patterns across all modal files. 3 existing helper functions (`main_dialog_modal`, `dashboard_modal`, `stack_modal`) consolidated into one API.

#### 4.5.2 ConfirmDialog (`overlay/confirm_dialog.rs`)

**MOVE** `confirm_dialog_container()` from `widget/mod.rs` with ModalShell integration.

```rust
/// Confirmation dialog with message + action buttons.
/// Uses ModalShell::Confirm internally.
pub fn confirm_dialog<'a, Message: Clone + 'a>(
    message: &str,
    on_confirm: Message,
    on_cancel: Message,
) -> ConfirmDialogBuilder<'a, Message>;

pub struct ConfirmDialogBuilder<'a, Message> { /* ... */ }

impl<'a, Message: Clone + 'a> ConfirmDialogBuilder<'a, Message> {
    /// Set confirm button text (default: "Confirm").
    pub fn confirm_text(self, text: &'a str) -> Self;
    /// Set cancel button text (default: "Cancel").
    pub fn cancel_text(self, text: &'a str) -> Self;
    /// Use danger style for confirm button (destructive actions).
    pub fn destructive(self) -> Self;
    /// Build the complete overlay (base + backdrop + dialog).
    pub fn view(self, base: impl Into<Element<'a, Message>>) -> Element<'a, Message>;
}
```

**Replaces**: `confirm_dialog_container()` in widget/mod.rs + 3 inline confirm dialog patterns.

#### 4.5.3 FormModal (`overlay/form_modal.rs`)

**Problem**: data_feeds.rs, historical_download.rs build modal forms with save/cancel footers manually.

```rust
/// Modal with scrollable form body + consistent Save/Cancel footer.
pub fn form_modal<'a, Message: Clone + 'a>(
    title: &'a str,
    body: impl Into<Element<'a, Message>>,
    on_save: Option<Message>,    // None = disabled (validation failed)
    on_cancel: Message,
) -> FormModalBuilder<'a, Message>;

pub struct FormModalBuilder<'a, Message> { /* ... */ }

impl<'a, Message: Clone + 'a> FormModalBuilder<'a, Message> {
    /// Set save button text (default: "Save").
    pub fn save_text(self, text: &'a str) -> Self;
    /// Set cancel button text (default: "Cancel").
    pub fn cancel_text(self, text: &'a str) -> Self;
    /// Set max width (default: tokens::layout::MODAL_MAX_WIDTH).
    pub fn max_width(self, width: u32) -> Self;
    /// Build the complete overlay.
    pub fn view(self, base: impl Into<Element<'a, Message>>) -> Element<'a, Message>;
}
```

**Replaces**: Form modal patterns in data_feeds.rs (edit form), historical_download.rs.

#### 4.5.4 DropdownMenu (`overlay/dropdown_menu.rs`)

**Problem**: sidebar.rs and drawing_tools.rs manually build positioned dropdown overlays with `stack!` + calculated offsets.

```rust
/// Positioned dropdown menu overlay.
pub fn dropdown_menu<'a, Message: Clone + 'a>(
    items: Vec<Element<'a, Message>>,
    on_close: Message,
) -> DropdownMenuBuilder<'a, Message>;

pub struct DropdownMenuBuilder<'a, Message> { /* ... */ }

impl<'a, Message: Clone + 'a> DropdownMenuBuilder<'a, Message> {
    /// Set position offset from anchor.
    pub fn offset(self, x: f32, y: f32) -> Self;
    /// Set max height with scrolling.
    pub fn max_height(self, height: f32) -> Self;
    /// Set width.
    pub fn width(self, width: impl Into<Length>) -> Self;
    /// Build as stack overlay on top of base content.
    pub fn view(self, base: impl Into<Element<'a, Message>>) -> Element<'a, Message>;
}
```

**Replaces**: 2 hand-built dropdown overlays in sidebar.rs and drawing_tools.rs.

#### 4.5.5 ContextMenu (`overlay/context_menu.rs`)

**Problem**: Pane title bar has right-click menu potential but no context menu component exists.

```rust
/// Right-click context menu overlay (future use for pane operations).
pub fn context_menu<'a, Message: Clone + 'a>(
    items: Vec<(&'a str, Option<Message>)>,
    position: Point,
    on_close: Message,
) -> Element<'a, Message>;
```

**New component** -- initially for pane right-click operations (split, close, popout).

---

### 4.6 Form Components (`component/form/`)

#### 4.6.1 FormField (`form/form_field.rs`)

**Problem**: The most duplicated pattern in the codebase -- 25+ instances of label + control + validation.

```rust
/// Universal form field wrapper: label above, any control below,
/// optional validation error.
pub fn form_field<'a, Message: 'a>(
    label_text: &str,
    control: impl Into<Element<'a, Message>>,
) -> FormFieldBuilder<'a, Message>;

pub struct FormFieldBuilder<'a, Message> { /* ... */ }

impl<'a, Message: 'a> FormFieldBuilder<'a, Message> {
    /// Show validation error below the control.
    pub fn error(self, msg: &'a str) -> Self;
    /// Add info tooltip next to label.
    pub fn tooltip(self, text: &'a str) -> Self;
    /// Mark as required (adds * after label).
    pub fn required(self) -> Self;
    /// Set label width for horizontal alignment.
    pub fn label_width(self, width: impl Into<Length>) -> Self;
    /// Use horizontal layout (label: control) instead of vertical (label / control).
    pub fn horizontal(self) -> Self;
    pub fn into_element(self) -> Element<'a, Message>;
}
```

This is the **composition wrapper** -- it takes any control (text_field, dropdown, checkbox, slider) and adds a label + validation. Individual input components can be used standalone or wrapped in `form_field()`.

#### 4.6.2 FormRow (`form/form_row.rs`)

**Problem**: Horizontal form layouts (label on left, control on right) are manually composed with `row!` + width proportions.

```rust
/// Horizontal form layout: label on left, control on right.
pub fn form_row<'a, Message: 'a>(
    label_text: &str,
    control: impl Into<Element<'a, Message>>,
) -> Element<'a, Message>;
```

**Replaces**: Horizontal label+control rows in settings modals.

#### 4.6.3 FormSection (`form/form_section.rs`)

**Problem**: Grouped form fields with section headers are composed manually.

```rust
/// Group of form fields under a section header with dividers.
pub fn form_section<'a, Message: 'a>(
    header: &str,
    fields: Vec<Element<'a, Message>>,
) -> FormSectionBuilder<'a, Message>;

pub struct FormSectionBuilder<'a, Message> { /* ... */ }

impl<'a, Message: 'a> FormSectionBuilder<'a, Message> {
    /// Add spacing between fields (default: tokens::spacing::MD).
    pub fn spacing(self, spacing: f32) -> Self;
    /// Add top divider before header.
    pub fn with_top_divider(self) -> Self;
    /// Add trailing element to header.
    pub fn header_trailing(self, element: impl Into<Element<'a, Message>>) -> Self;
    pub fn into_element(self) -> Element<'a, Message>;
}
```

**Replaces**: Manual section patterns in settings modals, data_feeds edit form.

---

### 4.7 Component Summary

| Category | Components | Existing (MOVE) | New | Instances Replaced | Est. Lines Saved |
|---|---|---|---|---|---|
| **Primitives** | label, badge, icon, icon_button, separator, truncated_text | 2 (icon, icon_text) | 4 | 260+ | ~200 |
| **Input** | text_field, secure_field, numeric_field, search_field, dropdown, multi_select, combo_select, checkbox_field, radio_group, toggle_button, toggle_switch, slider_field, stepper, color_picker | 3 (numeric, slider, color_picker) | 11 | 90+ | ~350 |
| **Display** | status_dot, progress_bar, loading_status, key_value, empty_state, toast, tooltip | 2 (toast, tooltip) | 5 | 30+ | ~150 |
| **Layout** | card, interactive_card, section_header, collapsible, split_section, list_item, reorderable_list, toolbar, button_group, button_grid | 1 (split_column) | 9 | 40+ | ~250 |
| **Overlay** | modal_shell, confirm_dialog, form_modal, dropdown_menu, context_menu | 1 (confirm_dialog) | 4 | 20+ | ~200 |
| **Form** | form_field, form_row, form_section | 0 | 3 | 30+ | ~150 |
| **TOTAL** | **44 components** | **9 moved** | **36 new** | **470+** | **~1,300** |

### 4.8 Migration Strategy

Components are extracted **bottom-up** (leaves first, then composites):

```
Phase B1: Primitives (label, separator, icon MOVE, icon_button)
    ↓
Phase B2: Input controls (text_field, secure_field, dropdown, checkbox_field, radio_group, toggle_button)
    ↓
Phase B3: Display components (status_dot, progress_bar, key_value, empty_state, tooltip MOVE)
    ↓
Phase B4: Layout components (card, interactive_card, list_item, section_header, button_group)
    ↓
Phase B5: Overlay components (modal_shell, confirm_dialog, form_modal, dropdown_menu)
    ↓
Phase B6: Form composition (form_field, form_row, form_section)
    ↓
Phase B7: Remaining MOVEs (toast, color_picker, slider_field, numeric_field, split_section, reorderable_list)
    ↓
Phase B8: New components (combo_select, multi_select, toggle_switch, stepper, collapsible, context_menu, badge, toolbar, button_grid)

Each phase: create component → update ONE call site → cargo build && cargo test → update remaining call sites
```

Components with 0 existing call sites (combo_select, multi_select, toggle_switch, stepper, collapsible, context_menu) are created in Phase B8 but only wired into existing code opportunistically when touching those files for other refactoring.

---

## 5. Message Architecture

### 5.1 Current State

```
Message (41 variants)
+-- 9 forwarding variants (Sidebar, TickersTable, Dashboard, DataManagement,
|   ConnectionsMenu, DataFeeds, ThemeEditor, Layouts, AudioStream)
+-- 7 chart/data loading variants
+-- 5 download variants
+-- 6 window/navigation variants
+-- 5 theme/UI variants
+-- 3 modal dialog variants
+-- 2 Rithmic variants
+-- 4 options data variants
```

### 5.2 Proposed Decomposition

**Phase 1 -- Group into sub-enums (no Element::map yet):**

```rust
pub enum Message {
    // Forwarding (keep for now -- migrate to Element::map in Phase D)
    Sidebar(sidebar::Message),
    TickersTable(tickers_table::Message),
    Dashboard { layout_id: LayoutId, event: dashboard::Message },

    // Grouped by domain
    Chart(ChartMessage),
    Download(DownloadMessage),
    Options(OptionsMessage),
    Window(WindowMessage),
    Preferences(PreferencesMessage),

    // Modal forwarding (keep for now)
    DataManagement(DataManagementMessage),
    ConnectionsMenu(ConnectionsMenuMessage),
    DataFeeds(DataFeedsMessage),
    ThemeEditor(ThemeEditorMessage),
    Layouts(LayoutsMessage),
    AudioStream(AudioStreamMessage),
    HistoricalDownload(HistoricalDownloadMessage),

    // Rithmic
    RithmicConnected { result: Result<RithmicServiceResult, String> },
    RithmicStreamEvent(Vec<exchange::Event>),
}

pub enum ChartMessage {
    LoadData { pane: pane_grid::Pane, /* ... */ },
    DataLoaded { pane: pane_grid::Pane, result: /* ... */ },
    ReplayEvent(data::services::replay::Event),
    UpdateLoadingStatus,
}

pub enum DownloadMessage {
    EstimateCost { /* ... */ },
    CostEstimated { /* ... */ },
    Start { /* ... */ },
    Progress { id: Uuid, current: usize, total: usize },
    Complete { /* ... */ },
}

pub enum WindowMessage {
    Event(window::Id, window::Event),
    ExitRequested,
    RestartRequested,
    Tick(Instant),
    GoBack,
    DataFolderRequested,
}

pub enum PreferencesMessage {
    ThemeSelected(data::Theme),
    ScaleFactorChanged(data::ScaleFactor),
    SetTimezone(data::UserTimezone),
    RemoveNotification(usize),
    ToggleDialogModal(Option<ConfirmDialog<Message>>),
    ReinitializeService,
}
```

**Phase 2 -- Element::map for standalone UI components (Phase D of migration):**

```rust
// In Flowsurface::view():
let sidebar_view = self.sidebar.view().map(Message::Sidebar);
let tickers_view = self.tickers_table.view().map(Message::TickersTable);

// In Flowsurface::update():
// Sidebar messages handled entirely in sidebar.update()
// Only effects bubble up via Action enum
```

### 5.3 update.rs Split

Split 1,530-line update.rs into domain modules:

```
app/update/
+-- mod.rs           # Top-level match dispatching to sub-modules (~80 lines)
+-- chart.rs         # ChartMessage handling (~100 lines)
+-- feeds.rs         # DataFeeds + ConnectionsMenu + Rithmic (~500 lines)
+-- download.rs      # DownloadMessage handling (~150 lines)
+-- options.rs       # OptionsMessage handling (~80 lines)
+-- navigation.rs    # WindowMessage + GoBack + Tick (~100 lines)
+-- preferences.rs   # PreferencesMessage handling (~60 lines)
```

**Critical**: `feeds.rs` gets the DataFeeds handler (currently 300+ lines) and the RithmicConnected handler (currently 150 lines). These are the worst offenders and must be flattened:

```rust
// BEFORE: 7 levels deep in update.rs
Message::DataFeeds(msg) => {
    if let Some(action) = self.data_feeds_modal.update(msg, &mut feed_manager) {
        match action {
            Action::ConnectFeed(feed_id) => {
                if let Some(feed) = feed_manager.get(feed_id) {
                    match feed.provider {
                        FeedProvider::Rithmic => {
                            if let Some(password) = ... { // Level 7!

// AFTER: extracted helper methods in feeds.rs
Message::DataFeeds(msg) => {
    let Some(action) = self.data_feeds_modal.update(msg, &mut feed_manager) else {
        return Task::none();
    };
    self.handle_feed_action(action, &mut feed_manager)
}

fn handle_feed_action(&mut self, action: FeedAction, mgr: &mut FeedManager) -> Task<Message> {
    match action {
        FeedAction::ConnectFeed(id) => self.connect_feed(id, mgr),
        FeedAction::DisconnectFeed(id) => self.disconnect_feed(id, mgr),
        FeedAction::UpdateFeed(id, config) => self.update_feed(id, config, mgr),
        // ... each method is 20-40 lines, flat
    }
}
```

---

## 6. Deduplication Targets

### Ranked by Lines Saved

| Rank | Target | Location | Lines Saved | Complexity |
|------|--------|----------|-------------|------------|
| 1 | Cluster rendering template | chart/candlestick/footprint.rs:246-613 | ~140 | Medium |
| 2 | Modal backdrop pattern | 15+ modal files | ~120 | Low |
| 3 | Form field pattern | 20+ instances across modals | ~60 | Low |
| 4 | Button style consolidation | style/button.rs (4 redundant variants) | ~60 | Low |
| 5 | Icon+tooltip button | 8+ instances | ~40 | Low |
| 6 | Status color constants | connections_menu + data_feeds (5 sites) | ~30 | Trivial |
| 7 | Section header | data_feeds.rs:1361 (local, not shared) | ~10 | Trivial |
| **Total** | | | **~460** | |

### 6.1 Cluster Rendering Template (Highest Impact)

**Current**: `draw_clusters()` in footprint.rs handles 4 cluster kinds with copy-pasted rendering:

```
Lines 274-384: VolumeProfile/DeltaProfile rendering
Lines 385-497: BidAsk rendering         <-- 70% identical to above
Lines 498-612: Delta/Volume/Trades       <-- 70% identical to above
```

**After**: Extract shared template into `chart/candlestick/cluster.rs`:

```rust
/// Renders a single column of cluster bars at the given price levels.
fn render_cluster_column(
    frame: &mut Frame,
    footprint: &BTreeMap<i64, TradeGroup>,
    area: &ClusterArea,
    max_qty: f32,
    config: &ClusterRenderConfig,
) { /* shared rendering logic */ }

struct ClusterRenderConfig {
    value_extractor: fn(&TradeGroup) -> f32,
    color_fn: fn(&TradeGroup, &Palette) -> Color,
    show_text: bool,
    text_formatter: fn(&TradeGroup) -> String,
    show_imbalance: bool,
}
```

**Estimated**: footprint.rs goes from 765 -> ~500 lines. cluster.rs is ~140 lines.

### 6.2 Button Style Consolidation

**Merge candidates**:

| Current | Merged With | Merged Into |
|---------|-------------|------------|
| `transparent()` | `modifier()` | `toolbar_button(theme, status, has_bg, is_active)` |
| `confirm()` | `cancel()` | `action_button(theme, status, palette_kind)` |

This reduces 16 -> 12 functions and eliminates ~60 lines of near-identical match arms.

---

## 7. Dead Code Resolution

### Study Modules (`chart/study/`)

| Module | Status | Recommendation | Rationale |
|--------|--------|---------------|-----------|
| `npoc.rs` | **USED** -- integrated in candlestick render | Remove `#[allow(dead_code)]` | Already rendering NPoC lines |
| `imbalance.rs` | **USED** -- integrated in draw_clusters | Remove `#[allow(dead_code)]` | Imbalance markers working |
| `poc.rs` | Implemented, not wired | **Wire up** as footprint overlay | `find_poc()` ready; add draw call in render.rs + toggle in study.rs UI |
| `value_area.rs` | Implemented, not wired | **Wire up** as footprint overlay | `calculate_value_area()` ready; render VAH/VAL lines |
| `volume_profile.rs` | Working in heatmap only | **Wire up for candlestick** | Port existing heatmap integration to candlestick footprint |

**Action**: ~20 lines of UI + ~30 lines of render integration each for poc and value_area.

### Performance Modules (`chart/perf/`)

| Module | Status | Recommendation | Rationale |
|--------|--------|---------------|-----------|
| `lod.rs` | 60% integrated (heatmap uses) | **Wire for candlestick** | Add decimation to render_candles() for high-trade scenarios |
| `viewport.rs` | 80% integrated (BTreeMap ranges) | **Keep, implicit use OK** | Already implicitly used via BTreeMap range queries |
| `progressive.rs` | 0% integrated | **Feature-flag** behind `perf-debug` | Framework ready but not needed at current data volumes |
| `overlay.rs` | 0% integrated | **Feature-flag** behind `perf-debug` | Debug overlay; useful for development only |
| `presets.rs` | 0% integrated | **Feature-flag** behind `perf-debug` | Configuration defined, no rendering yet |

**Action**:
- Wire `lod.rs` into candlestick rendering (~2 hours effort)
- Gate progressive/overlay/presets behind `#[cfg(feature = "perf-debug")]`

### Drawing Tool Icons

7 drawing tool icons use placeholder glyphs (reusing Edit, DragHandle, ResizeFull). Commission proper icon glyphs for the icon font. Low priority -- functional but visually confusing.

---

## 8. Migration Order

### Guiding Principles

- **Every step compiles and runs** -- no big-bang rewrites
- **Bottom-up** -- extract leaves first, then compose
- **Test after each step** -- `cargo build && cargo test && cargo clippy`
- **One concern per PR** -- each phase is a reviewable pull request

---

### Phase A: Design Tokens & Theme (No Behavior Change)

**Goal**: Establish the design token system. Zero visual changes.

**Steps**:
1. Create `src/theme/` directory with `mod.rs`
2. Create `src/theme/tokens.rs` with all constants (S3.1)
3. Create `src/theme/palette.rs` with `status_color()` (S3.2)
4. Move `src/style/mod.rs` Icon enum, icon_text, font constants -> `src/theme/icon.rs`
5. Move `src/style/button.rs` -> `src/theme/button.rs`
6. Move `src/style/container.rs` -> `src/theme/container.rs`
7. Create `src/theme/mod.rs` that re-exports everything + `style::` compat alias
8. Update all `style::` imports -> `theme::` across the codebase
9. Replace hardcoded magic numbers with `tokens::` references in theme/ files first
10. Consolidate 4 redundant button variants (transparent+modifier, confirm+cancel)

**Verification**: `cargo build && cargo test` -- pure rename + constant extraction, zero behavior change.

**Effort**: 2-3 hours. **Risk**: Low (rename only).

---

### Phase B: Extract Component Library (Bottom-up, 8 Sub-phases)

**Goal**: Build the full 44-component UI library (see S4). Extract leaves first, then composites. Each sub-phase compiles and runs independently.

**B1. Primitives** (foundation — no deps on other components):
1. Create `src/component/` directory tree (primitives/, input/, display/, layout/, overlay/, form/)
2. Create `component/mod.rs` with module declarations
3. MOVE `Icon` enum + `icon_text()` from `style/mod.rs` → `component/primitives/icon.rs`
4. Create `primitives/label.rs` — heading(), title(), label(), body(), small(), tiny(), mono()
5. Create `primitives/separator.rs` — divider(), thick_divider(), vertical_divider(), flex_space()
6. Create `primitives/icon_button.rs` — IconButtonBuilder + toolbar_icon() convenience
7. Create `primitives/truncated_text.rs` — truncated()
8. Create `primitives/badge.rs` — badge()
9. Update call sites file by file (start with modal files that have densest usage)
- `cargo build && cargo test`

**B2. Input Controls** (core form inputs):
1. Create `input/text_field.rs` — TextFieldBuilder (replaces 25+ label+input patterns)
2. Create `input/secure_field.rs` — SecureFieldBuilder (replaces 5 secure input patterns)
3. Create `input/dropdown.rs` — DropdownBuilder (wraps pick_list, replaces 12 bare usages)
4. Create `input/checkbox_field.rs` — CheckboxFieldBuilder (replaces 11 checkbox patterns)
5. Create `input/radio_group.rs` — RadioGroupBuilder (replaces 3 manual radio groups)
6. Create `input/toggle_button.rs` — ToggleButtonBuilder (replaces 15+ toggle patterns)
7. Update call sites in modal/pane/data_feeds.rs, settings/*.rs, historical_download.rs
- `cargo build && cargo test`

**B3. Display Components** (status and feedback):
1. Create `display/status_dot.rs` — status_dot(), status_badge(), status_row()
2. Create `display/progress_bar.rs` — themed_progress_bar()
3. Create `display/loading_status.rs` — loading_status_display()
4. Create `display/key_value.rs` — KeyValueBuilder
5. Create `display/empty_state.rs` — EmptyStateBuilder
6. MOVE tooltip(), tooltip_with_delay() → `display/tooltip.rs`
7. Update call sites in connections_menu.rs, data_feeds.rs, data_management.rs, pane/view.rs
- `cargo build && cargo test`

**B4. Layout Components** (cards, lists, sections):
1. Create `layout/card.rs` — CardBuilder
2. Create `layout/interactive_card.rs` — InteractiveCardBuilder
3. Create `layout/list_item.rs` — ListItemBuilder
4. Create `layout/section_header.rs` — SectionHeaderBuilder
5. Create `layout/button_group.rs` — ButtonGroupBuilder
6. Update call sites in tickers_table.rs, data_feeds.rs, layout_manager.rs, stream.rs
- `cargo build && cargo test`

**B5. Overlay Components** (modals and dropdowns):
1. Create `overlay/modal_shell.rs` — ModalShell + chart_modal() + dashboard_modal()
2. MOVE confirm_dialog_container() → `overlay/confirm_dialog.rs` as ConfirmDialogBuilder
3. Create `overlay/form_modal.rs` — FormModalBuilder
4. Create `overlay/dropdown_menu.rs` — DropdownMenuBuilder
5. Replace all 15+ hand-rolled modal backdrops with ModalShell
6. Replace sidebar/drawing_tools dropdown overlays with DropdownMenu
- `cargo build && cargo test`

**B6. Form Composition** (high-level wrappers):
1. Create `form/form_field.rs` — FormFieldBuilder (label + control + validation)
2. Create `form/form_row.rs` — form_row() horizontal layout
3. Create `form/form_section.rs` — FormSectionBuilder (grouped fields)
4. Update form-heavy files: data_feeds.rs, historical_download.rs, settings/*.rs
- `cargo build && cargo test`

**B7. Remaining Moves** (relocate well-built existing components):
1. MOVE widget/toast.rs → `display/toast.rs`
2. MOVE widget/color_picker.rs → `input/color_picker.rs`
3. MOVE labeled_slider() + classic_slider_row() → `input/slider_field.rs`
4. MOVE numeric_input_box() → `input/numeric_field.rs`
5. MOVE split_column! macro → `layout/split_section.rs`
6. Create `layout/reorderable_list.rs` (thin wrapper around column_drag + dragger_row)
7. Update all import paths
- `cargo build && cargo test`

**B8. New Components** (unused Iced primitives + future-ready):
1. Create `input/combo_select.rs` — searchable dropdown (Iced combo_box)
2. Create `input/multi_select.rs` — multi-checkbox dropdown
3. Create `input/toggle_switch.rs` — Iced toggler wrapper
4. Create `input/stepper.rs` — [-] N [+] increment control
5. Create `input/search_field.rs` — search icon + input + clear
6. Create `layout/collapsible.rs` — expand/collapse section
7. Create `layout/toolbar.rs` — horizontal toolbar
8. Create `layout/button_grid.rs` — grid of buttons
9. Create `overlay/context_menu.rs` — right-click menu
10. Wire components into existing code opportunistically (only where currently touching files)
- `cargo build && cargo test`

**Verification**: After each sub-phase, `cargo build && cargo test`. Visual output should be identical (except new features like Escape-to-close, combo_box search).

**Effort**: 8-12 hours (increased from 4-6h due to comprehensive library). **Risk**: Low (each sub-phase is independent; pure extract+replace with no logic changes).

---

### Phase C: Restructure File Organization

**Goal**: Split oversized files. Move files to new locations. Update `mod.rs`.

**Steps** (in dependency order):

**C1. Split `widget/chart/comparison.rs`** (1,845 -> 4 files):
- Create `widget/chart/comparison/` directory
- Extract `scene.rs` (compute_scene, PlotContext, domain -- ~400 lines)
- Extract `render.rs` (fill_* geometry helpers -- ~500 lines)
- Extract `legend.rs` (legend layout + hit-testing -- ~250 lines)
- Keep `mod.rs` (Widget impl, events, state -- ~700 lines)
- `cargo build && cargo test`

**C2. Split `modal/pane/data_feeds.rs`** (1,377 -> 3 files):
- Create `modal/pane/data_feeds/` directory
- Extract `view.rs` (view_left_panel, view_right_panel, view_edit_form -- ~500 lines)
- Extract `preview.rs` (historical preview rendering -- ~200 lines)
- Keep `mod.rs` (struct, Message, update -- ~700 lines)
- `cargo build && cargo test`

**C3. Split `screen/dashboard/pane/view.rs`** (760 -> 5 files):
- Create `screen/dashboard/pane/view/` directory
- Extract `kline.rs` (Kline content rendering -- ~150 lines)
- Extract `heatmap.rs` (Heatmap content rendering -- ~150 lines)
- Extract `comparison.rs` (Comparison rendering -- ~80 lines)
- Extract `starter.rs` (Starter placeholder -- ~30 lines)
- Keep `mod.rs` (compose_stack_view, title_bar -- ~350 lines)
- `cargo build && cargo test`

**C4. Split `app/update.rs`** (1,530 -> 7 files):
- Create `app/update/` directory
- Extract `chart.rs` -- ChartMessage handling
- Extract `feeds.rs` -- DataFeeds + Rithmic handlers (**flatten nesting here**)
- Extract `download.rs` -- Download handlers
- Extract `options.rs` -- Options handlers
- Extract `navigation.rs` -- Window/navigation handlers
- Extract `preferences.rs` -- Theme/UI preference handlers
- Keep `mod.rs` -- top-level dispatch
- `cargo build && cargo test`

**C5. Extract cluster rendering** from footprint.rs:
- Create `chart/candlestick/cluster.rs` (~140 lines)
- Refactor draw_clusters() to use shared template
- footprint.rs drops from 765 -> ~500 lines
- `cargo build && cargo test`

**Verification**: After each split, run full test suite. Visual diff confirms no rendering changes.

**Effort**: 6-8 hours. **Risk**: Medium (file moves can break imports; test thoroughly).

---

### Phase D: Decompose Message Enum (Feature by Feature)

**Goal**: Introduce sub-message enums and Element::map where beneficial.

**Steps**:

**D1. Group messages into sub-enums** (S5.2):
- Create `ChartMessage`, `DownloadMessage`, `OptionsMessage`, `WindowMessage`, `PreferencesMessage`
- Wrap in Message enum
- Update all `match message { ... }` arms in update/ modules
- `cargo build && cargo test`

**D2. Element::map for Sidebar**:
- Sidebar already has its own Message type
- In view(): `self.sidebar.view().map(Message::Sidebar)`
- In update(): delegate entirely to `self.sidebar.update(msg)`
- Extract side-effects as Actions returned from sidebar.update()
- `cargo build && cargo test`

**D3. Element::map for TickersTable**:
- Same pattern as Sidebar
- `cargo build && cargo test`

**D4. Element::map for ThemeEditor, AudioStream, LayoutManager**:
- These are UI-only components with minimal side-effects
- `cargo build && cargo test` after each

**D5. Element::map for modal dialogs** (DataManagement, ConnectionsMenu, HistoricalDownload):
- Use Action enum pattern: modal returns `Option<Action>`, app handles Action
- `cargo build && cargo test` after each

**Verification**: After each Element::map migration, verify the component still works end-to-end.

**Effort**: 4-6 hours. **Risk**: Medium (message routing changes need careful testing).

---

### Phase E: Polish Interactions & Wire Dead Code

**Goal**: Add missing UX, wire unused modules, final quality pass.

**Steps**:

**E1. Keyboard navigation**:
- Add Escape-to-close to ModalShell component (if not done in Phase B)
- Wire to all modals via consistent on_key_press event
- `cargo build && cargo test`

**E2. Wire POC + Value Area studies**:
- Add render calls in candlestick/render.rs
- Add UI toggles in modal/pane/settings/study.rs
- `cargo build && cargo test`

**E3. Wire LOD for candlestick**:
- Integrate LodCalculator into candlestick render path
- Add decimation when visible_trade_count > threshold
- `cargo build && cargo test`

**E4. Feature-flag perf debug modules**:
- Add `[features] perf-debug = []` to Cargo.toml
- Gate progressive.rs, overlay.rs, presets.rs behind feature
- Remove `#[allow(dead_code)]` from gated modules
- `cargo build && cargo test`

**E5. Replace remaining magic numbers**:
- Grep for f32 literals in view code
- Replace with `tokens::` references
- `cargo build && cargo test`

**E6. Final cleanup**:
- Run `cargo clippy` -- fix all warnings
- Run `cargo fmt` -- format all files
- Remove any remaining `#[allow(dead_code)]` that's no longer needed
- Update CLAUDE.md with new file structure

**Effort**: 4-6 hours. **Risk**: Low-Medium (new features + cleanup).

---

## Summary

### Phase Timeline

| Phase | Description | Effort | Risk | Files Changed |
|-------|-------------|--------|------|--------------|
| **A** | Design tokens & theme | 2-3h | Low | ~30 (rename/const) |
| **B** | Extract component library (44 components, 8 sub-phases) | 8-12h | Low | ~80 (create+replace) |
| **C** | Restructure files | 6-8h | Medium | ~20 (split+move) |
| **D** | Decompose messages | 4-6h | Medium | ~15 (refactor) |
| **E** | Polish & wire dead code | 4-6h | Low-Med | ~15 (feature work) |
| **Total** | | **24-35h** | | |

### Success Criteria

- [ ] No file over 900 lines (except tickers_table.rs and ladder.rs -- self-contained)
- [ ] No function over 150 lines
- [ ] No nesting deeper than 4 levels in update handlers
- [ ] Zero hardcoded status colors (all from theme/palette.rs)
- [ ] Design tokens used for all spacing/sizing in new/modified code
- [ ] Escape-to-close works in all modals
- [ ] All study modules either wired or feature-flagged
- [ ] All perf modules either wired or feature-flagged
- [ ] `cargo build && cargo test && cargo clippy` passes at every step
- [ ] Zero visual regressions
- [ ] Component library complete: 44 components across 6 categories
- [ ] All text uses semantic label functions (heading/title/label/body/small/tiny/mono)
- [ ] All form fields use FormField/TextField/SecureField/Dropdown wrappers
- [ ] All modals use ModalShell (zero hand-rolled stack+opaque+mouse_area)
- [ ] All status indicators use StatusDot/StatusBadge (zero hardcoded RGB colors)
- [ ] All icon buttons use IconButtonBuilder (zero manual icon+button+tooltip composition)
- [ ] Zero bare pick_list -- all wrapped in Dropdown component
- [ ] Zero bare text_input -- all wrapped in TextField/SecureField/SearchField/NumericField
