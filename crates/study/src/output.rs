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

    /// Footprint: per-candle trade-level data replacing standard candle rendering
    Footprint(FootprintData),

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
            std_dev: 2.0,
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

// ── CandleReplace configuration ──────────────────────────────────────

/// Layout constants for a CandleReplace study.
/// Overrides the chart's default cell sizing, zoom bounds,
/// and initial candle window.
#[derive(Debug, Clone, Copy)]
pub struct CandleRenderConfig {
    pub default_cell_width: f32,
    pub max_cell_width: f32,
    pub min_cell_width: f32,
    pub cell_height_ratio: f32,
    pub initial_candle_window: usize,
    pub autoscale_x_cells: f32,
}

// ── Footprint output types ──────────────────────────────────────────

/// How the renderer should handle footprint level grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FootprintGroupingMode {
    /// Renderer merges levels dynamically based on y-axis scale * factor.
    Automatic { factor: i64 },
    /// Levels are pre-grouped by the study; renderer uses as-is.
    Manual,
}

/// Style for the thin candle marker drawn outside footprint bars.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
pub enum OutsideBarStyle {
    #[default]
    Body,
    Candle,
    None,
}

impl std::fmt::Display for OutsideBarStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutsideBarStyle::Body => write!(f, "Body"),
            OutsideBarStyle::Candle => write!(f, "Candle"),
            OutsideBarStyle::None => write!(f, "None"),
        }
    }
}

/// Text formatting mode for footprint level values.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
pub enum TextFormat {
    #[default]
    Automatic,
    Normal,
    K,
}

impl std::fmt::Display for TextFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextFormat::Automatic => write!(f, "Automatic"),
            TextFormat::Normal => write!(f, "Normal"),
            TextFormat::K => write!(f, "K"),
        }
    }
}

/// Background coloring mode for footprint cells (Box mode).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
pub enum BackgroundColorMode {
    #[default]
    VolumeIntensity,
    DeltaIntensity,
    None,
}

impl std::fmt::Display for BackgroundColorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackgroundColorMode::VolumeIntensity => {
                write!(f, "Volume Intensity")
            }
            BackgroundColorMode::DeltaIntensity => {
                write!(f, "Delta Intensity")
            }
            BackgroundColorMode::None => write!(f, "None"),
        }
    }
}

/// Top-level footprint data produced by `FootprintStudy::output()`.
#[derive(Debug, Clone)]
pub struct FootprintData {
    // Core
    pub mode: FootprintRenderMode,
    pub data_type: FootprintDataType,
    pub scaling: FootprintScaling,
    pub candle_position: FootprintCandlePosition,
    pub candles: Vec<FootprintCandle>,
    // Bar Marker
    pub bar_marker_width: f32,
    pub outside_bar_style: OutsideBarStyle,
    pub show_outside_border: bool,
    pub max_bars_to_show: usize,
    // Background
    pub bg_color_mode: BackgroundColorMode,
    pub bg_max_alpha: f32,
    pub bg_buy_color: Option<SerializableColor>,
    pub bg_sell_color: Option<SerializableColor>,
    pub show_grid_lines: bool,
    // Text
    pub font_size: f32,
    pub text_format: TextFormat,
    pub dynamic_text_size: bool,
    pub show_zero_values: bool,
    pub text_color: Option<SerializableColor>,
    // Tick grouping
    pub grouping_mode: FootprintGroupingMode,
}

impl Default for FootprintData {
    fn default() -> Self {
        Self {
            mode: FootprintRenderMode::default(),
            data_type: FootprintDataType::default(),
            scaling: FootprintScaling::default(),
            candle_position: FootprintCandlePosition::default(),
            candles: Vec::new(),
            bar_marker_width: 0.25,
            outside_bar_style: OutsideBarStyle::default(),
            show_outside_border: false,
            max_bars_to_show: 200,
            bg_color_mode: BackgroundColorMode::default(),
            bg_max_alpha: 0.6,
            bg_buy_color: None,
            bg_sell_color: None,
            show_grid_lines: true,
            font_size: 11.0,
            text_format: TextFormat::default(),
            dynamic_text_size: true,
            show_zero_values: false,
            text_color: None,
            grouping_mode: FootprintGroupingMode::Manual,
        }
    }
}

/// Per-candle footprint data.
#[derive(Debug, Clone)]
pub struct FootprintCandle {
    /// X coordinate: timestamp_ms (time-based) or candle index (tick-based)
    pub x: u64,
    pub open: i64,
    pub high: i64,
    pub low: i64,
    pub close: i64,
    pub levels: Vec<FootprintLevel>,
    pub poc_index: Option<usize>,
    /// Grouping quantum used for this candle (price units per row)
    pub quantum: i64,
}

/// Per-price-level trade data within a footprint candle.
#[derive(Debug, Clone)]
pub struct FootprintLevel {
    pub price: i64,
    pub buy_volume: f32,
    pub sell_volume: f32,
}

impl FootprintLevel {
    pub fn total_qty(&self) -> f32 {
        self.buy_volume + self.sell_volume
    }

    pub fn delta_qty(&self) -> f32 {
        self.buy_volume - self.sell_volume
    }
}

/// Rendering mode for footprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FootprintRenderMode {
    Box,
    #[default]
    Profile,
}

impl std::fmt::Display for FootprintRenderMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FootprintRenderMode::Box => write!(f, "Box"),
            FootprintRenderMode::Profile => write!(f, "Profile"),
        }
    }
}

/// Data type shown in the footprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FootprintDataType {
    #[default]
    Volume,
    BidAskSplit,
    Delta,
    DeltaAndVolume,
}

impl std::fmt::Display for FootprintDataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FootprintDataType::Volume => write!(f, "Volume"),
            FootprintDataType::BidAskSplit => write!(f, "Bid/Ask Split"),
            FootprintDataType::Delta => write!(f, "Delta"),
            FootprintDataType::DeltaAndVolume => write!(f, "Delta + Volume"),
        }
    }
}

/// Candle body position relative to the footprint bars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FootprintCandlePosition {
    None,
    #[default]
    Left,
    Center,
    Right,
}

impl std::fmt::Display for FootprintCandlePosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FootprintCandlePosition::None => write!(f, "None"),
            FootprintCandlePosition::Left => write!(f, "Left"),
            FootprintCandlePosition::Center => write!(f, "Center"),
            FootprintCandlePosition::Right => write!(f, "Right"),
        }
    }
}

/// Cluster scaling strategy for footprint bar widths.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum FootprintScaling {
    Linear,
    #[default]
    Sqrt,
    Log,
    VisibleRange,
    Datapoint,
    Hybrid { weight: f32 },
}

// SAFETY: Manual Eq is sound — `weight` is always finite (0.0..=1.0).
impl Eq for FootprintScaling {}

impl std::fmt::Display for FootprintScaling {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FootprintScaling::Linear => write!(f, "Linear"),
            FootprintScaling::Sqrt => write!(f, "Square Root"),
            FootprintScaling::Log => write!(f, "Logarithmic"),
            FootprintScaling::VisibleRange => write!(f, "Visible Range"),
            FootprintScaling::Datapoint => write!(f, "Datapoint"),
            FootprintScaling::Hybrid { weight } => write!(f, "Hybrid ({weight:.1})"),
        }
    }
}
