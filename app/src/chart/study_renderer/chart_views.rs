//! ChartView implementations for the three coordinate spaces.
//!
//! - `OverlayChartView` — chart-space (frame has translate+scale applied)
//! - `PanelChartView` — screen-space with local value range Y
//! - `SidePanelChartView` — screen-space with manual Y transform

use crate::chart::ViewState;
use data::{Price, Rgba};
use iced::Size;
use iced::theme::palette::Extended;
use study::output::render::chart_view::{ChartView, ThemeColors, VisibleRegion};

// ── ThemeColors builder ─────────────────────────────────────────────

/// Build `ThemeColors` from an Iced `Extended` palette.
pub fn theme_from_palette(palette: &Extended) -> ThemeColors {
    ThemeColors {
        bullish_weak: iced_to_rgba(palette.success.weak.color),
        bearish_weak: iced_to_rgba(palette.danger.weak.color),
        bullish_base: iced_to_rgba(palette.success.base.color),
        bearish_base: iced_to_rgba(palette.danger.base.color),
        primary: iced_to_rgba(palette.primary.base.color),
        text: iced_to_rgba(palette.background.weakest.text),
        grid: iced_to_rgba(palette.background.strong.color),
        background_weak: iced_to_rgba(palette.background.weak.color),
    }
}

#[inline]
fn iced_to_rgba(c: iced::Color) -> Rgba {
    Rgba::new(c.r, c.g, c.b, c.a)
}

// ── OverlayChartView ────────────────────────────────────────────────

/// Chart-space coordinate system for overlay/background/candle_replace studies.
///
/// The Iced frame already has translate+scale applied, so coordinates
/// are in chart-space directly.
pub struct OverlayChartView<'a> {
    state: &'a ViewState,
    bounds: Size,
    theme: ThemeColors,
}

impl<'a> OverlayChartView<'a> {
    pub fn new(state: &'a ViewState, bounds: Size, theme: ThemeColors) -> Self {
        Self {
            state,
            bounds,
            theme,
        }
    }
}

impl ChartView for OverlayChartView<'_> {
    fn interval_to_x(&self, interval: u64) -> f32 {
        self.state.interval_to_x(interval)
    }

    fn price_to_y(&self, price: f64) -> f32 {
        self.state.price_to_y(Price::from_f64(price))
    }

    fn price_units_to_y(&self, units: i64) -> f32 {
        self.state.price_to_y(Price::from_units(units))
    }

    fn value_to_y(&self, value: f32) -> f32 {
        self.state.price_to_y(Price::from_f32(value))
    }

    fn scaling(&self) -> f32 {
        self.state.scaling
    }

    fn cell_width(&self) -> f32 {
        self.state.cell_width
    }

    fn cell_height(&self) -> f32 {
        self.state.cell_height
    }

    fn tick_size_units(&self) -> i64 {
        self.state.tick_size.units
    }

    fn bounds_width(&self) -> f32 {
        self.bounds.width / self.state.scaling
    }

    fn bounds_height(&self) -> f32 {
        self.bounds.height / self.state.scaling
    }

    fn visible_region(&self) -> VisibleRegion {
        let r = self.state.visible_region(self.bounds);
        VisibleRegion {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }

    fn visible_intervals(&self) -> (u64, u64) {
        let r = self.state.visible_region(self.bounds);
        self.state.interval_range(&r)
    }

    fn theme_colors(&self) -> &ThemeColors {
        &self.theme
    }
}

// ── PanelChartView ──────────────────────────────────────────────────

/// Screen-space coordinate system for panel-placement studies.
///
/// X is screen-space (chart interval_to_x + translation + scaling).
/// Y maps through a local value range to the panel's pixel height.
pub struct PanelChartView<'a> {
    state: &'a ViewState,
    canvas_width: f32,
    y_offset: f32,
    panel_height: f32,
    value_min: f32,
    value_max: f32,
    theme: ThemeColors,
}

impl<'a> PanelChartView<'a> {
    pub fn new(
        state: &'a ViewState,
        canvas_width: f32,
        y_offset: f32,
        panel_height: f32,
        value_min: f32,
        value_max: f32,
        theme: ThemeColors,
    ) -> Self {
        Self {
            state,
            canvas_width,
            y_offset,
            panel_height,
            value_min,
            value_max,
            theme,
        }
    }
}

impl ChartView for PanelChartView<'_> {
    fn interval_to_x(&self, interval: u64) -> f32 {
        let chart_x = self.state.interval_to_x(interval);
        (chart_x + self.state.translation.x) * self.state.scaling + self.canvas_width / 2.0
    }

    fn price_to_y(&self, price: f64) -> f32 {
        self.value_to_y(price as f32)
    }

    fn price_units_to_y(&self, units: i64) -> f32 {
        self.value_to_y(Price::from_units(units).to_f64() as f32)
    }

    fn value_to_y(&self, value: f32) -> f32 {
        self.y_offset
            + study::output::render::coord::value_to_panel_y(
                value,
                self.value_min,
                self.value_max,
                self.panel_height,
            )
    }

    fn scaling(&self) -> f32 {
        // Panel renders in screen-space — scaling is 1.0
        1.0
    }

    fn cell_width(&self) -> f32 {
        self.state.cell_width * self.state.scaling
    }

    fn cell_height(&self) -> f32 {
        self.state.cell_height * self.state.scaling
    }

    fn tick_size_units(&self) -> i64 {
        self.state.tick_size.units
    }

    fn bounds_width(&self) -> f32 {
        self.canvas_width
    }

    fn bounds_height(&self) -> f32 {
        self.panel_height
    }

    fn visible_region(&self) -> VisibleRegion {
        VisibleRegion {
            x: 0.0,
            y: self.y_offset,
            width: self.canvas_width,
            height: self.panel_height,
        }
    }

    fn visible_intervals(&self) -> (u64, u64) {
        let r = self.state.visible_region(Size::new(self.canvas_width, self.panel_height));
        self.state.interval_range(&r)
    }

    fn theme_colors(&self) -> &ThemeColors {
        &self.theme
    }
}

// ── SidePanelChartView ──────────────────────────────────────────────

/// Screen-space coordinate system for side-panel studies (VBP bars).
///
/// Shares the main chart's Y axis but renders in screen-space coordinates.
pub struct SidePanelChartView<'a> {
    state: &'a ViewState,
    bounds: Size,
    theme: ThemeColors,
}

impl<'a> SidePanelChartView<'a> {
    pub fn new(state: &'a ViewState, bounds: Size, theme: ThemeColors) -> Self {
        Self {
            state,
            bounds,
            theme,
        }
    }
}

impl ChartView for SidePanelChartView<'_> {
    fn interval_to_x(&self, interval: u64) -> f32 {
        let chart_x = self.state.interval_to_x(interval);
        (chart_x + self.state.translation.x) * self.state.scaling + self.bounds.width / 2.0
    }

    fn price_to_y(&self, price: f64) -> f32 {
        self.price_units_to_y(Price::from_f64(price).units())
    }

    fn price_units_to_y(&self, units: i64) -> f32 {
        let chart_y = self.state.price_to_y(Price::from_units(units));
        (chart_y + self.state.translation.y) * self.state.scaling + self.bounds.height / 2.0
    }

    fn value_to_y(&self, value: f32) -> f32 {
        self.price_to_y(value as f64)
    }

    fn scaling(&self) -> f32 {
        // Side panel renders in screen-space — scaling is 1.0
        1.0
    }

    fn cell_width(&self) -> f32 {
        self.state.cell_width * self.state.scaling
    }

    fn cell_height(&self) -> f32 {
        self.state.cell_height * self.state.scaling
    }

    fn tick_size_units(&self) -> i64 {
        self.state.tick_size.units
    }

    fn bounds_width(&self) -> f32 {
        self.bounds.width
    }

    fn bounds_height(&self) -> f32 {
        self.bounds.height
    }

    fn visible_region(&self) -> VisibleRegion {
        VisibleRegion {
            x: 0.0,
            y: 0.0,
            width: self.bounds.width,
            height: self.bounds.height,
        }
    }

    fn visible_intervals(&self) -> (u64, u64) {
        let r = self.state.visible_region(self.bounds);
        self.state.interval_range(&r)
    }

    fn theme_colors(&self) -> &ThemeColors {
        &self.theme
    }
}
