//! Trade marker output types for the Big Trades study.
//!
//! Contains marker data, individual trade markers, debug info,
//! shape configuration, and render configuration.

use data::SerializableColor;
use serde::{Deserialize, Serialize};

/// Trade markers with their render configuration bundled together.
#[derive(Debug, Clone)]
pub struct MarkerData {
    pub markers: Vec<TradeMarker>,
    pub render_config: MarkerRenderConfig,
}

/// A single trade marker (aggregated big trade bubble).
#[derive(Debug, Clone)]
pub struct TradeMarker {
    /// X position: timestamp_ms (time-based) or candle index
    /// (tick-based)
    pub time: u64,
    /// Y position: VWAP in domain Price units (10^-8)
    pub price: i64,
    /// Total contracts (for sizing)
    pub contracts: f64,
    /// Trade side
    pub is_buy: bool,
    /// Pre-computed color from study params
    pub color: SerializableColor,
    /// Contract count text (None if show_labels=false)
    pub label: Option<String>,
    /// Debug info for trade aggregation inspection
    pub debug: Option<TradeMarkerDebug>,
}

/// Debug information for a trade marker's aggregation.
#[derive(Debug, Clone)]
pub struct TradeMarkerDebug {
    pub fill_count: u32,
    pub first_fill_time: u64,
    pub last_fill_time: u64,
    pub price_min_units: i64,
    pub price_max_units: i64,
    pub vwap_numerator: f64,
    pub vwap_denominator: f64,
}

/// Shape used for rendering trade markers.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize,
    Deserialize,
)]
pub enum MarkerShape {
    #[default]
    Circle,
    Square,
    TextOnly,
}

impl std::fmt::Display for MarkerShape {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            MarkerShape::Circle => write!(f, "Circle"),
            MarkerShape::Square => write!(f, "Square"),
            MarkerShape::TextOnly => write!(f, "Text Only"),
        }
    }
}

/// Configuration for how trade markers are rendered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkerRenderConfig {
    pub shape: MarkerShape,
    pub hollow: bool,
    /// Lower bound of the contract range for size scaling
    /// (typically filter_min). Trades at this value get min_size.
    pub scale_min: f64,
    /// Upper bound of the contract range for size scaling.
    /// Trades at or above this value get max_size.
    pub scale_max: f64,
    pub min_size: f32,
    pub max_size: f32,
    pub min_opacity: f32,
    pub max_opacity: f32,
    pub show_text: bool,
    pub text_size: f32,
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
            text_color: SerializableColor::new(
                0.88, 0.88, 0.88, 0.9,
            ),
        }
    }
}
