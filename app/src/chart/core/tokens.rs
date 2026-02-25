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
    /// Minimum drag distance before committing to a drag (pixels)
    pub const DRAG_THRESHOLD: f32 = 3.0;
    /// Double-click time window (milliseconds)
    pub const DOUBLE_CLICK_MS: u128 = 400;
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

/// Grid line constants
pub mod grid {
    /// Alpha for grid lines (subtle, behind content)
    pub const ALPHA: f32 = 0.15;
    /// Grid line width in pixels
    pub const LINE_WIDTH: f32 = 1.0;
}

/// Last price label constants
pub mod last_price {
    /// Horizontal padding inside the label box
    pub const LABEL_PADDING_X: f32 = 4.0;
    /// Vertical padding inside the label box
    pub const LABEL_PADDING_Y: f32 = 2.0;
    /// Margin from right edge of chart
    pub const LABEL_MARGIN_RIGHT: f32 = 4.0;
}

/// Candle rendering constants
pub mod candle {
    /// Ratio of candle body width to cell width
    pub const WIDTH_RATIO: f32 = 0.8;
    /// Ratio of wick width to candle body width
    pub const WICK_WIDTH_RATIO: f32 = 0.25;
}

/// Ruler measurement tool constants
pub mod ruler {
    /// Padding around ruler text
    pub const TEXT_PADDING: f32 = 8.0;
    /// Fill alpha for the ruler rectangle
    pub const FILL_ALPHA: f32 = 0.08;
    /// Background padding for ruler label
    pub const RECT_PADDING: f32 = 4.0;
    /// Arrow head length in pixels
    pub const ARROW_LENGTH: f32 = 12.0;
    /// Arrow head width in pixels
    pub const ARROW_WIDTH: f32 = 5.0;
}

/// Label positioning constants
pub mod label {
    /// Y offset to position label above the line
    pub const Y_OFFSET: f32 = 4.0;
    /// X padding from chart edges for labels
    pub const X_PADDING: f32 = 6.0;
}

/// Selection glow effect constants
pub mod selection {
    /// Extra width for selection glow stroke
    pub const GLOW_EXTRA: f32 = 4.0;
    /// Alpha for selection glow
    pub const GLOW_ALPHA: f32 = 0.2;
}

/// Comparison chart legend overlay constants.
/// Used by `chart/comparison/line_widget.rs`, `legend.rs`, `render.rs`, `scene.rs`.
pub mod legend {
    use crate::style::tokens::text;

    /// Padding inside the legend bounding box (pixels).
    pub const PADDING: f32 = 4.0;
    /// Height of one legend row: text size + internal vertical gap.
    pub const LINE_H: f32 = text::BODY + 6.0;
    /// Approximate rendered width of one character (for text sizing).
    pub const CHAR_W: f32 = text::BODY * 0.64;
    /// Square bounding box side for an icon glyph.
    pub const ICON_BOX: f32 = text::BODY + 8.0;
    /// Horizontal gap between adjacent icons in the icon cluster.
    pub const ICON_SPACING: f32 = 4.0;
    /// Gap between the end of the ticker name text and the icon cluster.
    pub const ICON_GAP_AFTER_TEXT: f32 = 8.0;
    /// Body text size for legend labels (mirrors style::tokens::text::BODY).
    pub const TEXT_SIZE: f32 = text::BODY;
}

/// Heatmap chart cell size limits
pub mod heatmap {
    pub const MAX_CELL_WIDTH: f32 = 12.0;
    pub const MIN_CELL_WIDTH: f32 = 1.0;
    pub const MAX_CELL_HEIGHT: f32 = 10.0;
    pub const MIN_CELL_HEIGHT: f32 = 0.1;
    pub const DEFAULT_CELL_WIDTH: f32 = 3.0;
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
