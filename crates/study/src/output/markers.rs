//! Trade marker output types for the Big Trades study.
//!
//! Markers represent aggregated large trade events rendered as sized bubbles
//! on the chart, with configurable shape, scaling, and opacity.

use data::SerializableColor;
use serde::{Deserialize, Serialize};

/// Collection of trade markers bundled with their render configuration.
#[derive(Debug, Clone)]
pub struct MarkerData {
    /// Individual trade markers to render.
    pub markers: Vec<TradeMarker>,
    /// Shared rendering configuration for all markers.
    pub render_config: MarkerRenderConfig,
}

/// A single trade marker representing an aggregated big trade.
#[derive(Debug, Clone)]
pub struct TradeMarker {
    /// X position: timestamp_ms (time-based) or candle index
    /// (tick-based).
    pub time: u64,
    /// Y position: VWAP in fixed-point Price units (10^-8).
    pub price: i64,
    /// Total contracts traded (determines bubble size).
    pub contracts: f64,
    /// `true` for buy-side, `false` for sell-side.
    pub is_buy: bool,
    /// Pre-computed color from study parameters.
    pub color: SerializableColor,
    /// Contract count label text. `None` when labels are disabled.
    pub label: Option<String>,
    /// Debug info for inspecting the underlying trade aggregation.
    pub debug: Option<TradeMarkerDebug>,
    /// Per-marker shape override. When `Some`, overrides the
    /// config-level shape for this specific marker.
    pub shape_override: Option<MarkerShape>,
}

/// Debug information for a trade marker's aggregation window.
///
/// Captures details about the individual fills that were combined into
/// a single marker, useful for debugging trade detection thresholds.
#[derive(Debug, Clone, Copy)]
pub struct TradeMarkerDebug {
    /// Number of individual fills aggregated into this marker.
    pub fill_count: u32,
    /// Timestamp of the first fill in the aggregation window.
    pub first_fill_time: u64,
    /// Timestamp of the last fill in the aggregation window.
    pub last_fill_time: u64,
    /// Lowest price across all fills, in fixed-point units (10^-8).
    pub price_min_units: i64,
    /// Highest price across all fills, in fixed-point units (10^-8).
    pub price_max_units: i64,
    /// Accumulated (price * qty) for VWAP computation.
    pub vwap_numerator: f64,
    /// Accumulated quantity for VWAP computation.
    pub vwap_denominator: f64,
}

/// Shape used for rendering trade markers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MarkerShape {
    /// Filled or hollow circle.
    #[default]
    Circle,
    /// Filled or hollow square.
    Square,
    /// Label text only, no shape.
    TextOnly,
    /// Small crosshair mark (horizontal + vertical lines).
    Cross,
}

impl std::fmt::Display for MarkerShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarkerShape::Circle => write!(f, "Circle"),
            MarkerShape::Square => write!(f, "Square"),
            MarkerShape::TextOnly => write!(f, "Text Only"),
            MarkerShape::Cross => write!(f, "Cross"),
        }
    }
}

/// Configuration for how trade markers are rendered on the chart.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MarkerRenderConfig {
    /// Bubble shape.
    pub shape: MarkerShape,
    /// Whether to render shapes as hollow outlines instead of filled.
    pub hollow: bool,
    /// Lower bound of the contract range for size scaling. Trades at
    /// this value get `min_size`.
    pub scale_min: f64,
    /// Upper bound of the contract range for size scaling. Trades at
    /// or above this value get `max_size`.
    pub scale_max: f64,
    /// Minimum bubble radius in logical pixels.
    pub min_size: f32,
    /// Maximum bubble radius in logical pixels.
    pub max_size: f32,
    /// Minimum bubble opacity.
    pub min_opacity: f32,
    /// Maximum bubble opacity.
    pub max_opacity: f32,
    /// Whether to show contract count text on each marker.
    pub show_text: bool,
    /// Font size for marker labels in logical pixels.
    pub text_size: f32,
    /// Color for marker label text.
    pub text_color: SerializableColor,
}

impl Default for MarkerRenderConfig {
    fn default() -> Self {
        Self {
            shape: MarkerShape::Circle,
            hollow: false,
            scale_min: 50.0,
            scale_max: 500.0,
            min_size: 8.0,
            max_size: 36.0,
            min_opacity: 0.10,
            max_opacity: 0.60,
            show_text: true,
            text_size: 10.0,
            text_color: SerializableColor::new(0.88, 0.88, 0.88, 0.9),
        }
    }
}
