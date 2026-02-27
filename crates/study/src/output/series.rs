//! Primitive series types shared across study categories.
//!
//! Line series, bar series, histogram bars, and horizontal price levels.

use crate::config::LineStyleValue;
use data::SerializableColor;
use serde::{Deserialize, Serialize};

/// A series of connected line points (e.g. SMA, EMA).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineSeries {
    /// Display label for legends and tooltips.
    pub label: String,
    /// Line color.
    pub color: SerializableColor,
    /// Line width in logical pixels.
    pub width: f32,
    /// Solid, dashed, or dotted.
    pub style: LineStyleValue,
    /// Data points as `(x, y)` where x is timestamp_ms (time-based) or
    /// candle index (tick-based), and y is the computed value.
    pub points: Vec<(u64, f32)>,
}

/// A series of bar data points (e.g. Volume, Delta bars).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarSeries {
    /// Display label for legends and tooltips.
    pub label: String,
    /// Individual bar data points.
    pub points: Vec<BarPoint>,
}

/// A single bar data point with color and optional overlay.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BarPoint {
    /// X coordinate: timestamp_ms or candle index.
    pub x: u64,
    /// Bar height value.
    pub value: f32,
    /// Bar fill color.
    pub color: SerializableColor,
    /// Optional overlay value drawn on top of the bar (e.g. delta
    /// overlay on a volume bar).
    pub overlay: Option<f32>,
}

/// A single histogram bar (e.g. MACD histogram).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HistogramBar {
    /// X coordinate: timestamp_ms or candle index.
    pub x: u64,
    /// Bar value (positive above zero line, negative below).
    pub value: f32,
    /// Bar fill color.
    pub color: SerializableColor,
}

/// A horizontal price level line with optional fill regions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    /// Price value in f64 domain coordinates.
    pub price: f64,
    /// Display label (e.g. "VAH", "POC", "Fib 0.618").
    pub label: String,
    /// Line color.
    pub color: SerializableColor,
    /// Solid, dashed, or dotted.
    pub style: LineStyleValue,
    /// Line opacity in `[0.0, 1.0]`.
    pub opacity: f32,
    /// Whether to render the label next to the line.
    pub show_label: bool,
    /// Fill color and opacity for the region above this level.
    pub fill_above: Option<(SerializableColor, f32)>,
    /// Fill color and opacity for the region below this level.
    pub fill_below: Option<(SerializableColor, f32)>,
    /// Ray anchor X coordinate. When `Some`, the level renders as a ray
    /// from this point rightward. When `None`, renders as a full-width
    /// line.
    #[serde(default)]
    pub start_x: Option<u64>,
}
