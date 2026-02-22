//! Primitive output types shared across multiple study categories.
//!
//! Contains line series, bar series, histogram bars, and price
//! level types used by trend, momentum, volatility, and volume
//! studies.

use crate::config::LineStyleValue;
use data::SerializableColor;
use serde::{Deserialize, Serialize};

/// A series of connected line points.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineSeries {
    pub label: String,
    pub color: SerializableColor,
    pub width: f32,
    pub style: LineStyleValue,
    /// Points as (time_or_index, value)
    pub points: Vec<(u64, f32)>,
}

/// A series of bar data points.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarSeries {
    pub label: String,
    pub points: Vec<BarPoint>,
}

/// A single bar data point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarPoint {
    pub x: u64,
    pub value: f32,
    pub color: SerializableColor,
    /// For delta overlay on volume bars
    pub overlay: Option<f32>,
}

/// A single histogram bar (e.g. MACD histogram).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramBar {
    pub x: u64,
    pub value: f32,
    pub color: SerializableColor,
}

/// A horizontal price level line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: f64,
    pub label: String,
    pub color: SerializableColor,
    pub style: LineStyleValue,
    pub opacity: f32,
    pub show_label: bool,
    /// Fill color and opacity above this level
    pub fill_above: Option<(SerializableColor, f32)>,
    /// Fill color and opacity below this level
    pub fill_below: Option<(SerializableColor, f32)>,
}
