use crate::config::LineStyleValue;
use data::SerializableColor;
use serde::{Deserialize, Serialize};

/// Abstract render primitives output by studies.
/// The chart rendering layer converts these into canvas draw calls.
#[derive(Debug, Clone, Default)]
pub enum StudyOutput {
    /// Single line series (SMA, EMA)
    Lines(Vec<LineSeries>),

    /// Multiple lines with optional fill between (Bollinger Bands)
    Band {
        upper: LineSeries,
        middle: Option<LineSeries>,
        lower: LineSeries,
        fill_opacity: f32,
    },

    /// Bar chart (Volume, Delta)
    Bars(Vec<BarSeries>),

    /// Histogram (MACD histogram)
    Histogram(Vec<HistogramBar>),

    /// Horizontal levels (Fibonacci, Support/Resistance)
    Levels(Vec<PriceLevel>),

    /// Price profile (Volume Profile, Market Profile)
    Profile(ProfileData),

    /// Heatmap overlay (Footprint clusters)
    Clusters(Vec<ClusterRow>),

    /// Trade markers (Big Trades bubbles)
    Markers(Vec<TradeMarker>),

    /// No output yet (not computed)
    #[default]
    Empty,
}

/// A single trade marker (aggregated big trade bubble).
#[derive(Debug, Clone)]
pub struct TradeMarker {
    /// X position: timestamp_ms (time-based) or candle index (tick-based)
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
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
pub enum MarkerShape {
    #[default]
    Circle,
    Square,
    TextOnly,
}

impl std::fmt::Display for MarkerShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
    pub std_dev: f32,
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
            std_dev: 2.5,
            min_size: 6.0,
            max_size: 40.0,
            min_opacity: 0.4,
            max_opacity: 1.0,
            show_text: true,
            text_size: 11.0,
            text_color: SerializableColor::new(1.0, 1.0, 1.0, 1.0),
        }
    }
}

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

/// Which side a volume profile renders on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileSide {
    Left,
    Right,
    Both,
}

/// A single level within a volume profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileLevel {
    pub price: f64,
    pub buy_volume: f32,
    pub sell_volume: f32,
}

/// Volume profile data for rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileData {
    pub side: ProfileSide,
    pub levels: Vec<ProfileLevel>,
    /// Index of the Point of Control level
    pub poc: Option<usize>,
    /// Value area high and low indices (VAH, VAL)
    pub value_area: Option<(usize, usize)>,
}

/// A row of cluster data for footprint rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterRow {
    pub x: u64,
    pub price: f64,
    pub buy_volume: f32,
    pub sell_volume: f32,
    pub color: SerializableColor,
}
