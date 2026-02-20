//! Chart rendering constants
//!
//! Single source of truth for all chart-specific visual constants.
//! All chart rendering code should reference these instead of inline magic numbers.

/// Drawing system constants
pub mod drawing {
    /// Size of selection handles in pixels
    pub const HANDLE_SIZE: f32 = 8.0;
    /// Extra stroke width when drawing is selected
    pub const SELECTION_WIDTH_BOOST: f32 = 2.0;
    /// Hit test tolerance for clicking on drawings (pixels)
    pub const HIT_TOLERANCE: f32 = 5.0;
    /// Alpha for pending drawing preview (semi-transparent)
    pub const PREVIEW_ALPHA: f32 = 0.5;
}

/// Overlay rendering constants
pub mod overlay {
    /// Dash pattern for dashed lines [dash_length, gap_length]
    pub const DASH_PATTERN: &[f32] = &[8.0, 4.0];
    /// Dot pattern for dotted lines [dot_length, gap_length]
    pub const DOT_PATTERN: &[f32] = &[2.0, 4.0];
    /// Dash-dot pattern [dash, gap, dot, gap]
    pub const DASH_DOT_PATTERN: &[f32] = &[8.0, 4.0, 2.0, 4.0];
    /// Alpha for last price line
    pub const LAST_PRICE_ALPHA: f32 = 0.5;
}

/// Float comparison epsilons
pub mod epsilon {
    /// General float comparison
    pub const FLOAT_CMP: f32 = 1e-6;
    /// Line segment degenerate check
    pub const LINE_DEGENERATE: f32 = 0.0001;
    /// Ray direction minimum component
    pub const RAY_DIRECTION: f32 = 0.0001;
}
