//! Chart-specific constants (axes, zoom, gap colors, histogram).
//! These are UI layout metrics referenced by chart widgets and pane views.
//!
//! Note: ruler constants live in `app/src/chart/core/tokens.rs::ruler` —
//! they are chart-internal rendering constants, not UI style tokens.
//!
//! Note: empty-state icon size lives in `tokens::component::icon::EMPTY_STATE` —
//! it is a generic UI component size, not a chart-specific constant.

pub const Y_AXIS_GUTTER: f32 = 66.0;
pub const X_AXIS_HEIGHT: f32 = 24.0;
pub const MIN_X_TICK_PX: f32 = 80.0;
pub const ZOOM_SENSITIVITY: f32 = 30.0;
pub const ZOOM_BASE: f32 = 2.0;
pub const ZOOM_STEP_PCT: f32 = 0.05;
pub const GAP_BREAK_MULTIPLIER: f32 = 3.0;

/// Colors for data gap overlay bands.
pub mod gap {
    use iced::Color;

    pub const NO_DATA: Color = Color::from_rgba(0.8, 0.2, 0.2, 0.08);
    pub const MARKET_CLOSED: Color = Color::from_rgba(0.5, 0.5, 0.5, 0.05);
    pub const PARTIAL_COVERAGE: Color = Color::from_rgba(0.8, 0.7, 0.2, 0.08);
}

/// Histogram zero-line baseline.
pub mod histogram {
    use iced::Color;

    pub const ZERO_LINE: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.2);
}
