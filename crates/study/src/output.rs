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

    /// Trade markers (Big Trades bubbles) with render configuration
    Markers(MarkerData),

    /// Volume-by-Price profile (VBP study)
    Vbp(VbpData),

    /// Multiple outputs combined (e.g. MACD: Lines + Histogram)
    Composite(Vec<StudyOutput>),

    /// No output yet (not computed)
    #[default]
    Empty,
}

/// Trade markers with their render configuration bundled together.
#[derive(Debug, Clone)]
pub struct MarkerData {
    pub markers: Vec<TradeMarker>,
    pub render_config: MarkerRenderConfig,
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
    /// Pre-computed price in i64 units (10^-8) to avoid repeated
    /// f64→Price conversions during rendering.
    pub price_units: i64,
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
    /// Color for buy volume bars
    pub buy_color: SerializableColor,
    /// Color for sell volume bars
    pub sell_color: SerializableColor,
    /// Color for the POC line
    pub poc_color: SerializableColor,
    /// Color for value area highlighting
    pub value_area_color: SerializableColor,
    /// Width as percentage of chart width (0.0 - 1.0)
    pub width_pct: f32,
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

// ── Volume-by-Price (VBP) shared types ──────────────────────────────

/// Direction for extending horizontal lines beyond the profile bounds.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
pub enum ExtendDirection {
    #[default]
    None,
    Left,
    Right,
    Both,
}

impl std::fmt::Display for ExtendDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtendDirection::None => write!(f, "None"),
            ExtendDirection::Left => write!(f, "Left"),
            ExtendDirection::Right => write!(f, "Right"),
            ExtendDirection::Both => write!(f, "Both"),
        }
    }
}

/// Method for detecting high/low volume nodes.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
pub enum NodeDetectionMethod {
    #[default]
    Percentile,
    Relative,
    StdDev,
}

impl std::fmt::Display for NodeDetectionMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeDetectionMethod::Percentile => write!(f, "Percentile"),
            NodeDetectionMethod::Relative => write!(f, "Relative"),
            NodeDetectionMethod::StdDev => write!(f, "Std Dev"),
        }
    }
}

/// A detected high or low volume node in a profile.
#[derive(Debug, Clone)]
pub struct VolumeNode {
    /// Price level in fixed-point units (10^-8)
    pub price_units: i64,
    /// Price level as f64
    pub price: f64,
    /// Total volume at this level
    pub volume: f32,
}

/// POC configuration within VBP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VbpPocConfig {
    pub show_poc: bool,
    pub poc_color: SerializableColor,
    pub poc_line_width: f32,
    pub poc_line_style: LineStyleValue,
    pub poc_extend: ExtendDirection,
    pub show_poc_label: bool,
    pub show_developing_poc: bool,
    pub developing_poc_color: SerializableColor,
    pub developing_poc_line_width: f32,
    pub developing_poc_line_style: LineStyleValue,
}

/// Value Area configuration within VBP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VbpValueAreaConfig {
    pub show_value_area: bool,
    pub value_area_pct: f32,
    pub show_va_highlight: bool,
    pub vah_color: SerializableColor,
    pub vah_line_width: f32,
    pub vah_line_style: LineStyleValue,
    pub val_color: SerializableColor,
    pub val_line_width: f32,
    pub val_line_style: LineStyleValue,
    pub show_va_fill: bool,
    pub va_fill_color: SerializableColor,
    pub va_fill_opacity: f32,
    pub va_extend: ExtendDirection,
    pub show_va_labels: bool,
}

/// HVN/LVN (Peak & Valley) configuration within VBP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VbpNodeConfig {
    pub show_hvn: bool,
    pub show_lvn: bool,
    pub hvn_method: NodeDetectionMethod,
    pub hvn_threshold: f32,
    pub lvn_method: NodeDetectionMethod,
    pub lvn_threshold: f32,
    pub min_prominence: f32,
    pub hvn_color: SerializableColor,
    pub hvn_line_style: LineStyleValue,
    pub hvn_line_width: f32,
    pub hvn_extend: ExtendDirection,
    pub lvn_color: SerializableColor,
    pub lvn_line_style: LineStyleValue,
    pub lvn_line_width: f32,
    pub lvn_extend: ExtendDirection,
    pub show_hvn_labels: bool,
    pub show_lvn_labels: bool,
}

/// Anchored VWAP configuration within VBP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VbpVwapConfig {
    pub show_vwap: bool,
    pub vwap_color: SerializableColor,
    pub vwap_line_width: f32,
    pub vwap_line_style: LineStyleValue,
    pub show_vwap_label: bool,
    pub show_bands: bool,
    pub band_multiplier: f32,
    pub band_color: SerializableColor,
    pub band_line_style: LineStyleValue,
    pub band_line_width: f32,
}

// ── Volume-by-Price (VBP) output types ──────────────────────────────

/// Visualization type for VBP study.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
pub enum VbpType {
    #[default]
    Volume,
    BidAskVolume,
    Delta,
    DeltaAndTotalVolume,
    DeltaPercentage,
}

impl std::fmt::Display for VbpType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VbpType::Volume => write!(f, "Volume"),
            VbpType::BidAskVolume => write!(f, "Bid/Ask Volume"),
            VbpType::Delta => write!(f, "Delta"),
            VbpType::DeltaAndTotalVolume => {
                write!(f, "Delta & Total Volume")
            }
            VbpType::DeltaPercentage => write!(f, "Delta Percentage"),
        }
    }
}

/// Time period mode for VBP computation range.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
pub enum VbpPeriod {
    #[default]
    Auto,
    Length,
    Custom,
}

impl std::fmt::Display for VbpPeriod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VbpPeriod::Auto => write!(f, "Auto"),
            VbpPeriod::Length => write!(f, "Length"),
            VbpPeriod::Custom => write!(f, "Custom"),
        }
    }
}

/// Unit for VBP length-based period.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
pub enum VbpLengthUnit {
    #[default]
    Days,
    Minutes,
    Contracts,
}

impl std::fmt::Display for VbpLengthUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VbpLengthUnit::Days => write!(f, "Days"),
            VbpLengthUnit::Minutes => write!(f, "Minutes"),
            VbpLengthUnit::Contracts => write!(f, "Contracts"),
        }
    }
}

/// How the renderer should handle VBP level grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VbpGroupingMode {
    /// Renderer merges levels dynamically based on y-axis scale * factor.
    Automatic { factor: i64 },
    /// Levels are pre-grouped by the study; renderer uses as-is.
    Manual,
}

impl Default for VbpGroupingMode {
    fn default() -> Self {
        Self::Manual
    }
}

/// Cached resolved VBP levels after dynamic merging.
///
/// Stored alongside `VbpData` to avoid recomputing the merge
/// on every render frame. Only rebuilt when the dynamic quantum
/// changes (i.e., zoom level changes).
#[derive(Debug, Clone, Default)]
pub struct VbpResolvedCache {
    /// Dynamic quantum used to produce this cache.
    pub quantum: i64,
    /// Merged levels at the cached quantum.
    pub levels: Vec<ProfileLevel>,
    /// POC index in `levels`.
    pub poc: Option<usize>,
    /// Value area (VAH idx, VAL idx) in `levels`.
    pub value_area: Option<(usize, usize)>,
}

/// Volume-by-Price output data for rendering.
#[derive(Debug, Serialize, Deserialize)]
pub struct VbpData {
    pub vbp_type: VbpType,
    pub side: ProfileSide,
    pub levels: Vec<ProfileLevel>,
    /// Grouping quantum used for the levels (price units per row)
    pub quantum: i64,
    /// Index of the Point of Control level
    pub poc: Option<usize>,
    /// Value area (VAH index, VAL index)
    pub value_area: Option<(usize, usize)>,
    /// Time range of captured data (start_ms, end_ms) for bounding rect
    pub time_range: Option<(u64, u64)>,
    // Style config (volume bars)
    pub volume_color: SerializableColor,
    pub bid_color: SerializableColor,
    pub ask_color: SerializableColor,
    pub width_pct: f32,
    pub opacity: f32,
    // Nested feature configs
    pub poc_config: VbpPocConfig,
    pub va_config: VbpValueAreaConfig,
    pub node_config: VbpNodeConfig,
    pub vwap_config: VbpVwapConfig,
    // Computed data for new features
    /// Developing POC: (timestamp_ms, poc_price_units) per candle
    #[serde(skip)]
    pub developing_poc_points: Vec<(u64, i64)>,
    /// Detected high volume nodes
    #[serde(skip)]
    pub hvn_nodes: Vec<VolumeNode>,
    /// Detected low volume nodes
    #[serde(skip)]
    pub lvn_nodes: Vec<VolumeNode>,
    /// Anchored VWAP: (timestamp_ms, vwap_price)
    #[serde(skip)]
    pub vwap_points: Vec<(u64, f32)>,
    /// VWAP upper band: (timestamp_ms, price)
    #[serde(skip)]
    pub vwap_upper_points: Vec<(u64, f32)>,
    /// VWAP lower band: (timestamp_ms, price)
    #[serde(skip)]
    pub vwap_lower_points: Vec<(u64, f32)>,
    /// Tick grouping mode for renderer
    #[serde(skip)]
    pub grouping_mode: VbpGroupingMode,
    /// Renderer-side cache for resolved (merged) levels.
    /// Populated lazily by the renderer; avoids per-frame merging.
    #[serde(skip)]
    pub resolved_cache: std::sync::Mutex<Option<VbpResolvedCache>>,
}

impl Clone for VbpData {
    fn clone(&self) -> Self {
        Self {
            vbp_type: self.vbp_type,
            side: self.side,
            levels: self.levels.clone(),
            quantum: self.quantum,
            poc: self.poc,
            value_area: self.value_area,
            time_range: self.time_range,
            volume_color: self.volume_color,
            bid_color: self.bid_color,
            ask_color: self.ask_color,
            width_pct: self.width_pct,
            opacity: self.opacity,
            poc_config: self.poc_config.clone(),
            va_config: self.va_config.clone(),
            node_config: self.node_config.clone(),
            vwap_config: self.vwap_config.clone(),
            developing_poc_points: self
                .developing_poc_points
                .clone(),
            hvn_nodes: self.hvn_nodes.clone(),
            lvn_nodes: self.lvn_nodes.clone(),
            vwap_points: self.vwap_points.clone(),
            vwap_upper_points: self.vwap_upper_points.clone(),
            vwap_lower_points: self.vwap_lower_points.clone(),
            grouping_mode: self.grouping_mode,
            // Cache is lazily rebuilt — no need to clone
            resolved_cache: std::sync::Mutex::new(None),
        }
    }
}
