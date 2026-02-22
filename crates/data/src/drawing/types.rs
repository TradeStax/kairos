//! Serializable Drawing Types
//!
//! Types for persisting drawings to disk. These are pure data types
//! without any rendering or interaction logic.

use serde::{Deserialize, Serialize};

/// Unique identifier for a drawing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DrawingId(pub uuid::Uuid);

impl DrawingId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for DrawingId {
    fn default() -> Self {
        Self::new()
    }
}

/// Drawing tool type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum DrawingTool {
    /// Selection/pan mode (default) - no drawing active
    #[default]
    None,
    // ── Lines ─────────────────────────────────────────────────────────
    /// Two-point line segment
    Line,
    /// Line infinite in one direction from first point
    Ray,
    /// Line extending infinitely in both directions through two points
    ExtendedLine,
    /// Horizontal price level line (1 point)
    HorizontalLine,
    /// Vertical time/bar line (1 point)
    VerticalLine,
    // ── Fibonacci ──────────────────────────────────────────────────────
    /// Fibonacci retracement (2 points, configurable levels)
    FibRetracement,
    /// Fibonacci extension (3 points)
    FibExtension,
    // ── Channels ──────────────────────────────────────────────────────
    /// Parallel channel (3 points)
    ParallelChannel,
    // ── Shapes ────────────────────────────────────────────────────────
    /// Two-point rectangle
    Rectangle,
    /// Ellipse defined by center + radius point
    Ellipse,
    // ── Annotations ───────────────────────────────────────────────────
    /// Text label at a single point
    TextLabel,
    /// Price label at a single point (auto-shows price)
    PriceLabel,
    /// Arrow from point A to point B with arrowhead
    Arrow,
    // ── Measurement ───────────────────────────────────────────────────
    /// Price range measurement (2 points, shows price delta)
    PriceRange,
    /// Date range measurement (2 points, shows time delta)
    DateRange,
    // ── Trading ──────────────────────────────────────────────────────
    /// Buy position calculator (entry + target + auto-generated stop)
    BuyCalculator,
    /// Sell position calculator (entry + target + auto-generated stop)
    SellCalculator,
}

impl DrawingTool {
    /// Number of points required to complete this drawing
    pub fn required_points(&self) -> usize {
        match self {
            DrawingTool::None => 0,
            DrawingTool::HorizontalLine
            | DrawingTool::VerticalLine
            | DrawingTool::TextLabel
            | DrawingTool::PriceLabel => 1,
            DrawingTool::Line
            | DrawingTool::Ray
            | DrawingTool::ExtendedLine
            | DrawingTool::FibRetracement
            | DrawingTool::Rectangle
            | DrawingTool::Ellipse
            | DrawingTool::Arrow
            | DrawingTool::PriceRange
            | DrawingTool::DateRange
            | DrawingTool::BuyCalculator
            | DrawingTool::SellCalculator => 2,
            DrawingTool::FibExtension | DrawingTool::ParallelChannel => 3,
        }
    }

    /// All available drawing tools (excluding None)
    pub const ALL: &'static [DrawingTool] = &[
        // Lines
        DrawingTool::Line,
        DrawingTool::Ray,
        DrawingTool::ExtendedLine,
        DrawingTool::HorizontalLine,
        DrawingTool::VerticalLine,
        // Fibonacci
        DrawingTool::FibRetracement,
        DrawingTool::FibExtension,
        // Channels
        DrawingTool::ParallelChannel,
        // Shapes
        DrawingTool::Rectangle,
        DrawingTool::Ellipse,
        // Annotations
        DrawingTool::TextLabel,
        DrawingTool::PriceLabel,
        DrawingTool::Arrow,
        // Measurement
        DrawingTool::PriceRange,
        DrawingTool::DateRange,
        // Trading
        DrawingTool::BuyCalculator,
        DrawingTool::SellCalculator,
    ];
}

impl std::fmt::Display for DrawingTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DrawingTool::None => write!(f, "Select"),
            DrawingTool::Line => write!(f, "Line"),
            DrawingTool::Ray => write!(f, "Ray"),
            DrawingTool::ExtendedLine => write!(f, "Extended Line"),
            DrawingTool::HorizontalLine => write!(f, "H-Line"),
            DrawingTool::VerticalLine => write!(f, "V-Line"),
            DrawingTool::FibRetracement => write!(f, "Fib Retracement"),
            DrawingTool::FibExtension => write!(f, "Fib Extension"),
            DrawingTool::ParallelChannel => write!(f, "Parallel Channel"),
            DrawingTool::Rectangle => write!(f, "Rectangle"),
            DrawingTool::Ellipse => write!(f, "Ellipse"),
            DrawingTool::TextLabel => write!(f, "Text"),
            DrawingTool::PriceLabel => write!(f, "Price Label"),
            DrawingTool::Arrow => write!(f, "Arrow"),
            DrawingTool::PriceRange => write!(f, "Price Range"),
            DrawingTool::DateRange => write!(f, "Date Range"),
            DrawingTool::BuyCalculator => write!(f, "Buy Calculator"),
            DrawingTool::SellCalculator => write!(f, "Sell Calculator"),
        }
    }
}

/// Calculator mode for stop/target distance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum CalcMode {
    #[default]
    Free,
    Ticks,
    Money,
}

impl CalcMode {
    pub const ALL: [CalcMode; 3] = [CalcMode::Free, CalcMode::Ticks, CalcMode::Money];
}

impl std::fmt::Display for CalcMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CalcMode::Free => write!(f, "Free"),
            CalcMode::Ticks => write!(f, "Ticks"),
            CalcMode::Money => write!(f, "Money"),
        }
    }
}

/// Position calculator configuration for Buy/Sell calculator tools
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionCalcConfig {
    pub quantity: u32,
    pub stop_mode: CalcMode,
    pub stop_value: f64,
    pub target_mode: CalcMode,
    pub target_value: f64,
    pub target_color: SerializableColor,
    pub target_opacity: f32,
    pub stop_color: SerializableColor,
    pub stop_opacity: f32,
    pub label_font_size: f32,
    pub show_target_label: bool,
    pub show_entry_label: bool,
    pub show_stop_label: bool,
    pub show_pnl: bool,
    pub show_ticks: bool,
}

impl PositionCalcConfig {
    /// Default target color — matches theme success (green).
    pub const DEFAULT_TARGET_COLOR: SerializableColor =
        SerializableColor::from_rgb8_const(81, 205, 160);
    /// Default stop color — matches theme danger (red).
    pub const DEFAULT_STOP_COLOR: SerializableColor =
        SerializableColor::from_rgb8_const(192, 80, 77);
}

impl Default for PositionCalcConfig {
    fn default() -> Self {
        Self {
            quantity: 1,
            stop_mode: CalcMode::Free,
            stop_value: 0.0,
            target_mode: CalcMode::Free,
            target_value: 0.0,
            target_color: Self::DEFAULT_TARGET_COLOR,
            target_opacity: 0.15,
            stop_color: Self::DEFAULT_STOP_COLOR,
            stop_opacity: 0.15,
            label_font_size: 11.0,
            show_target_label: true,
            show_entry_label: true,
            show_stop_label: true,
            show_pnl: true,
            show_ticks: true,
        }
    }
}

/// Label alignment on a drawing line
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum LabelAlignment {
    Left,
    Center,
    #[default]
    Right,
}

impl LabelAlignment {
    pub const ALL: [LabelAlignment; 3] = [
        LabelAlignment::Left,
        LabelAlignment::Center,
        LabelAlignment::Right,
    ];
}

impl std::fmt::Display for LabelAlignment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LabelAlignment::Left => write!(f, "Left"),
            LabelAlignment::Center => write!(f, "Center"),
            LabelAlignment::Right => write!(f, "Right"),
        }
    }
}

/// Line style for drawing strokes
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum LineStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
    DashDot,
}

impl LineStyle {
    pub const ALL: [LineStyle; 4] = [
        LineStyle::Solid,
        LineStyle::Dashed,
        LineStyle::Dotted,
        LineStyle::DashDot,
    ];
}

impl std::fmt::Display for LineStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LineStyle::Solid => write!(f, "Solid"),
            LineStyle::Dashed => write!(f, "Dashed"),
            LineStyle::Dotted => write!(f, "Dotted"),
            LineStyle::DashDot => write!(f, "Dash-Dot"),
        }
    }
}

/// Fibonacci retracement/extension level
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FibLevel {
    /// The ratio (e.g. 0.618)
    pub ratio: f64,
    /// Color for this level line
    pub color: SerializableColor,
    /// Display label (e.g. "61.8%")
    pub label: String,
    /// Whether this level is drawn
    pub visible: bool,
}

/// Fibonacci configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FibonacciConfig {
    /// The fibonacci levels to display
    pub levels: Vec<FibLevel>,
    /// Show price values at each level
    pub show_prices: bool,
    /// Show percentage labels
    pub show_percentages: bool,
    /// Extend level lines beyond the anchor points
    pub extend_lines: bool,
}

impl Default for FibonacciConfig {
    fn default() -> Self {
        Self {
            levels: vec![
                FibLevel {
                    ratio: 0.0,
                    color: SerializableColor::new(0.5, 0.5, 0.5, 1.0),
                    label: "0%".into(),
                    visible: true,
                },
                FibLevel {
                    ratio: 0.236,
                    color: SerializableColor::new(0.8, 0.2, 0.2, 1.0),
                    label: "23.6%".into(),
                    visible: true,
                },
                FibLevel {
                    ratio: 0.382,
                    color: SerializableColor::new(0.2, 0.8, 0.2, 1.0),
                    label: "38.2%".into(),
                    visible: true,
                },
                FibLevel {
                    ratio: 0.5,
                    color: SerializableColor::new(0.2, 0.2, 0.8, 1.0),
                    label: "50%".into(),
                    visible: true,
                },
                FibLevel {
                    ratio: 0.618,
                    color: SerializableColor::new(0.2, 0.8, 0.2, 1.0),
                    label: "61.8%".into(),
                    visible: true,
                },
                FibLevel {
                    ratio: 0.786,
                    color: SerializableColor::new(0.8, 0.2, 0.2, 1.0),
                    label: "78.6%".into(),
                    visible: true,
                },
                FibLevel {
                    ratio: 1.0,
                    color: SerializableColor::new(0.5, 0.5, 0.5, 1.0),
                    label: "100%".into(),
                    visible: true,
                },
            ],
            show_prices: true,
            show_percentages: true,
            extend_lines: false,
        }
    }
}

/// Serializable color (RGBA). Alias for config::color::Rgba; convert to/from iced::Color at GUI boundary.
pub use crate::config::color::Rgba as SerializableColor;

/// Drawing style configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawingStyle {
    /// Stroke color
    pub stroke_color: SerializableColor,
    /// Stroke width in pixels
    pub stroke_width: f32,
    /// Line style (solid, dashed, dotted)
    pub line_style: LineStyle,
    /// Fill color (for rectangles, ellipses, etc.)
    pub fill_color: Option<SerializableColor>,
    /// Whether to show price labels
    pub show_labels: bool,
    /// Fill opacity (0.0 - 1.0)
    pub fill_opacity: f32,
    /// Fibonacci configuration (for FibRetracement/FibExtension tools)
    pub fibonacci: Option<FibonacciConfig>,
    /// Text content (for TextLabel tool)
    pub text: Option<String>,
    /// Label alignment (left, center, right) for line-type drawings
    #[serde(default)]
    pub label_alignment: LabelAlignment,
    /// Position calculator config (for BuyCalculator/SellCalculator tools)
    #[serde(default)]
    pub position_calc: Option<PositionCalcConfig>,
}

impl Default for DrawingStyle {
    fn default() -> Self {
        Self {
            stroke_color: SerializableColor::default(),
            stroke_width: 1.5,
            line_style: LineStyle::Solid,
            fill_color: None,
            show_labels: true,
            fill_opacity: 0.2,
            fibonacci: None,
            text: None,
            label_alignment: LabelAlignment::default(),
            position_calc: None,
        }
    }
}

/// A point anchored to price and time coordinates
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SerializablePoint {
    /// Price in fixed-point units (i64 with 10^-8 precision)
    pub price_units: i64,
    /// Timestamp in milliseconds or tick index
    pub time: u64,
    /// Whether this point was snapped to a candle
    pub snapped: bool,
}

impl SerializablePoint {
    pub fn new(price_units: i64, time: u64) -> Self {
        Self {
            price_units,
            time,
            snapped: false,
        }
    }

    pub fn with_snap(mut self, snapped: bool) -> Self {
        self.snapped = snapped;
        self
    }
}

/// A complete drawing that can be serialized
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SerializableDrawing {
    /// Unique identifier
    pub id: DrawingId,
    /// Type of drawing
    pub tool: DrawingTool,
    /// Anchor points
    pub points: Vec<SerializablePoint>,
    /// Visual style
    pub style: DrawingStyle,
    /// Whether the drawing is visible
    pub visible: bool,
    /// Whether the drawing is locked (cannot be edited)
    pub locked: bool,
    /// Optional user label
    pub label: Option<String>,
}

impl SerializableDrawing {
    pub fn new(tool: DrawingTool) -> Self {
        Self {
            id: DrawingId::new(),
            tool,
            points: Vec::new(),
            style: DrawingStyle::default(),
            visible: true,
            locked: false,
            label: None,
        }
    }

    pub fn with_points(mut self, points: Vec<SerializablePoint>) -> Self {
        self.points = points;
        self
    }

    pub fn with_style(mut self, style: DrawingStyle) -> Self {
        self.style = style;
        self
    }

    /// Check if the drawing has all required points
    pub fn is_complete(&self) -> bool {
        self.points.len() >= self.tool.required_points()
    }
}
