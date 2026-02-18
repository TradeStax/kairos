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
    /// Two-point line segment
    Line,
    /// Line infinite in one direction from first point
    Ray,
    /// Horizontal price level line (1 point)
    HorizontalLine,
    /// Vertical time/bar line (1 point)
    VerticalLine,
    /// Two-point rectangle
    Rectangle,
    /// Two-point trend line with optional extensions
    TrendLine,
}

impl DrawingTool {
    /// Number of points required to complete this drawing
    pub fn required_points(&self) -> usize {
        match self {
            DrawingTool::None => 0,
            DrawingTool::HorizontalLine | DrawingTool::VerticalLine => 1,
            DrawingTool::Line | DrawingTool::Ray | DrawingTool::Rectangle | DrawingTool::TrendLine => 2,
        }
    }

    /// All available drawing tools (excluding None)
    pub const ALL: &'static [DrawingTool] = &[
        DrawingTool::Line,
        DrawingTool::Ray,
        DrawingTool::HorizontalLine,
        DrawingTool::VerticalLine,
        DrawingTool::Rectangle,
        DrawingTool::TrendLine,
    ];
}

impl std::fmt::Display for DrawingTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DrawingTool::None => write!(f, "Select"),
            DrawingTool::Line => write!(f, "Line"),
            DrawingTool::Ray => write!(f, "Ray"),
            DrawingTool::HorizontalLine => write!(f, "H-Line"),
            DrawingTool::VerticalLine => write!(f, "V-Line"),
            DrawingTool::Rectangle => write!(f, "Rectangle"),
            DrawingTool::TrendLine => write!(f, "Trend Line"),
        }
    }
}

/// Line style for drawing strokes
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[derive(Default)]
pub enum LineStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}


/// Serializable color (RGBA)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SerializableColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for SerializableColor {
    fn default() -> Self {
        // Default to a visible blue color
        Self {
            r: 0.3,
            g: 0.6,
            b: 1.0,
            a: 1.0,
        }
    }
}

impl From<iced_core::Color> for SerializableColor {
    fn from(color: iced_core::Color) -> Self {
        Self {
            r: color.r,
            g: color.g,
            b: color.b,
            a: color.a,
        }
    }
}

impl From<SerializableColor> for iced_core::Color {
    fn from(color: SerializableColor) -> Self {
        iced_core::Color::from_rgba(color.r, color.g, color.b, color.a)
    }
}

/// Drawing style configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawingStyle {
    /// Stroke color
    pub stroke_color: SerializableColor,
    /// Stroke width in pixels
    pub stroke_width: f32,
    /// Line style (solid, dashed, dotted)
    pub line_style: LineStyle,
    /// Fill color (for rectangles)
    pub fill_color: Option<SerializableColor>,
    /// Whether to show price labels
    pub show_labels: bool,
}

impl Default for DrawingStyle {
    fn default() -> Self {
        Self {
            stroke_color: SerializableColor::default(),
            stroke_width: 1.5,
            line_style: LineStyle::Solid,
            fill_color: None,
            show_labels: true,
        }
    }
}

/// A point anchored to price and time coordinates
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
