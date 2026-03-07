//! Rendering constants extracted from the app crate.
//!
//! Centralises magic numbers so renderers stay free of app-crate imports.

use data::Rgba;

// ── Text sizes ──────────────────────────────────────────────────────

/// Smallest readable text size (badges, labels). Equivalent to
/// `app::style::tokens::text::TINY`.
pub const TINY_TEXT: f32 = 10.0;

/// Maximum text size for footprint cells.
pub const MAX_TEXT_SIZE: f32 = 14.0;

// ── Spacing ─────────────────────────────────────────────────────────

/// Comfortable spacing used for label offsets. Equivalent to
/// `app::style::tokens::spacing::LG`.
pub const LABEL_SPACING: f32 = 12.0;

// ── Histogram ───────────────────────────────────────────────────────

/// Color for the zero-baseline in histogram renderers.
pub const ZERO_LINE_COLOR: Rgba = Rgba {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.2,
};

// ── Levels / Zones ──────────────────────────────────────────────────

/// Width fractions of zone_half_width for each feathered strip
/// (outermost first).
pub const ZONE_STRIP_WIDTHS: [f32; 3] = [1.0, 0.65, 0.3];

/// Alpha multipliers for each feathered strip (outermost = most
/// transparent).
pub const ZONE_STRIP_ALPHAS: [f32; 3] = [0.04, 0.07, 0.12];

// ── Markers ─────────────────────────────────────────────────────────

/// Reference cell_width at default candlestick zoom.
pub const REFERENCE_CELL_WIDTH: f32 = 4.0;
/// Reference cell_height at default candlestick zoom.
pub const REFERENCE_CELL_HEIGHT: f32 = 1.0;
/// Maximum marker radius in X-axis cell widths (~12 candle diameter).
pub const MAX_RADIUS_CELLS_X: f32 = 6.0;
/// Maximum marker radius in Y-axis cell heights (~80 ticks diameter).
pub const MAX_RADIUS_CELLS_Y: f32 = 40.0;
/// Minimum marker radius in screen pixels (visibility floor).
pub const MIN_RADIUS_SCREEN_PX: f32 = 3.0;
/// Maximum marker radius in screen pixels (absolute ceiling).
pub const MAX_RADIUS_SCREEN_PX: f32 = 60.0;
/// Minimum screen-pixel font size for text legibility.
pub const MIN_TEXT_SCREEN_PX: f32 = 9.0;
/// Coarse grid bucket size (screen pixels) for density detection.
pub const DENSITY_BUCKET_PX: f32 = 40.0;

// ── Footprint ───────────────────────────────────────────────────────

/// Ratio of cell width occupied by cluster bars.
pub const FP_BAR_WIDTH_FACTOR: f32 = 0.9;
/// Alpha for cluster bar backgrounds when text labels are visible.
pub const FP_BAR_ALPHA_WITH_TEXT: f32 = 0.25;
/// Alpha for POC highlight background.
pub const FP_POC_HIGHLIGHT_ALPHA: f32 = 0.15;
/// Maximum price levels that receive text labels per candle.
pub const FP_TEXT_BUDGET: usize = 40;
/// Padding subtracted from text size.
pub const FP_TEXT_SIZE_PADDING: f32 = 2.0;
/// Ratio of cell width used as the candle width for footprint.
pub const FP_CANDLE_WIDTH_RATIO: f32 = 0.8;
/// Minimum row height in screen pixels for readable text (footprint).
pub const FP_MIN_ROW_PX: f32 = 16.0;

// ── VBP ─────────────────────────────────────────────────────────────

/// Minimum row height in screen pixels for readable VBP bars.
pub const VBP_MIN_ROW_PX: f32 = 4.0;

/// Font size for VBP price labels.
pub const VBP_LABEL_FONT_SIZE: f32 = TINY_TEXT;

// ── Bounding rect ───────────────────────────────────────────────────

/// Bounding rect outline color for VBP profiles.
pub const VBP_BOUNDING_RECT_COLOR: Rgba = Rgba {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.15,
};
