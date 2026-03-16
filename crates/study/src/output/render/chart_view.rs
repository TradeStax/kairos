//! Platform-agnostic chart coordinate system and viewport state.
//!
//! The study crate defines this trait; the app crate provides three
//! implementations (Overlay, Panel, SidePanel) that map coordinates
//! to the correct space.

use data::Rgba;

/// Read-only coordinate system and viewport state.
///
/// Each app-side implementation maps coordinates to the right space:
/// - **Overlay**: chart-space (frame has translate+scale applied)
/// - **Panel**: screen-space with local value range Y
/// - **SidePanel**: screen-space with manual Y transform
///
/// Renderers call `view.value_to_y(v)` uniformly — no placement
/// branching in rendering code.
pub trait ChartView {
    /// Map a time interval (timestamp_ms or tick index) to X coordinate.
    fn interval_to_x(&self, interval: u64) -> f32;

    /// Map a price (f64 domain) to Y coordinate.
    fn price_to_y(&self, price: f64) -> f32;

    /// Map a price in fixed-point units (10^-8) to Y coordinate.
    fn price_units_to_y(&self, units: i64) -> f32;

    /// Map a study value (f32) to Y coordinate.
    ///
    /// For overlay views, this converts through the price axis.
    /// For panel views, this maps through the local value range.
    fn value_to_y(&self, value: f32) -> f32;

    /// Current zoom scaling factor.
    fn scaling(&self) -> f32;

    /// Width of one candle cell in chart-space coordinates.
    fn cell_width(&self) -> f32;

    /// Height of one price tick in chart-space coordinates.
    fn cell_height(&self) -> f32;

    /// Instrument tick size in fixed-point price units.
    fn tick_size_units(&self) -> i64;

    /// Canvas width in the view's coordinate space.
    fn bounds_width(&self) -> f32;

    /// Canvas height in the view's coordinate space.
    fn bounds_height(&self) -> f32;

    /// Visible region in chart-space coordinates.
    fn visible_region(&self) -> VisibleRegion;

    /// Visible interval range `(earliest, latest)`.
    fn visible_intervals(&self) -> (u64, u64);

    /// Resolved theme palette colors.
    fn theme_colors(&self) -> &ThemeColors;
}

/// Visible region in chart-space coordinates.
#[derive(Debug, Clone, Copy)]
pub struct VisibleRegion {
    /// Left edge X.
    pub x: f32,
    /// Top edge Y.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

/// Resolved palette colors for study renderers.
///
/// Eliminates the dependency on `iced::theme::palette::Extended`.
/// The app crate fills this from the active Iced theme.
#[derive(Debug, Clone, Copy)]
pub struct ThemeColors {
    /// Bullish candle body color (success.weak).
    pub bullish_weak: Rgba,
    /// Bearish candle body color (danger.weak).
    pub bearish_weak: Rgba,
    /// Bullish bar/fill color (success.base).
    pub bullish_base: Rgba,
    /// Bearish bar/fill color (danger.base).
    pub bearish_base: Rgba,
    /// Primary accent color (primary.base).
    pub primary: Rgba,
    /// Default text color (background.weakest.text).
    pub text: Rgba,
    /// Grid/separator color (background.strong).
    pub grid: Rgba,
    /// Weak background color (background.weak).
    pub background_weak: Rgba,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            bullish_weak: Rgba::from_rgb8(81, 205, 160),
            bearish_weak: Rgba::from_rgb8(192, 80, 77),
            bullish_base: Rgba::from_rgb8(81, 205, 160),
            bearish_base: Rgba::from_rgb8(192, 80, 77),
            primary: Rgba::from_rgb8(100, 149, 237),
            text: Rgba::new(0.88, 0.88, 0.88, 1.0),
            grid: Rgba::new(0.3, 0.3, 0.3, 1.0),
            background_weak: Rgba::new(0.2, 0.2, 0.2, 1.0),
        }
    }
}
