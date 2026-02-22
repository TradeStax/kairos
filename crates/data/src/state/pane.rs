//! Pane Configuration Types
//!
//! UI state types for pane configuration and settings.

use crate::config::panel::timeandsales::StackedBarRatio;
use crate::domain::{ChartBasis, ChartType};
use serde::{Deserialize, Serialize};

/// Content kind for a pane
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentKind {
    Starter,
    HeatmapChart,
    CandlestickChart,
    TimeAndSales,
    Ladder,
    ComparisonChart,
    ProfileChart,
}

// Custom Serialize that writes CandlestickChart as "CandlestickChart"
impl Serialize for ContentKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            ContentKind::Starter => serializer.serialize_str("Starter"),
            ContentKind::HeatmapChart => serializer.serialize_str("HeatmapChart"),
            ContentKind::CandlestickChart => serializer.serialize_str("CandlestickChart"),
            ContentKind::TimeAndSales => serializer.serialize_str("TimeAndSales"),
            ContentKind::Ladder => serializer.serialize_str("Ladder"),
            ContentKind::ComparisonChart => serializer.serialize_str("ComparisonChart"),
            ContentKind::ProfileChart => serializer.serialize_str("ProfileChart"),
        }
    }
}

// Custom Deserialize that maps "FootprintChart" -> CandlestickChart for backward compat
impl<'de> Deserialize<'de> for ContentKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "Starter" => Ok(ContentKind::Starter),
            "HeatmapChart" => Ok(ContentKind::HeatmapChart),
            "CandlestickChart" | "FootprintChart" => Ok(ContentKind::CandlestickChart),
            "TimeAndSales" => Ok(ContentKind::TimeAndSales),
            "Ladder" => Ok(ContentKind::Ladder),
            "ComparisonChart" => Ok(ContentKind::ComparisonChart),
            "ScriptEditor" => Ok(ContentKind::Starter),
            "ProfileChart" => Ok(ContentKind::ProfileChart),
            other => Err(serde::de::Error::unknown_variant(
                other,
                &[
                    "Starter",
                    "HeatmapChart",
                    "CandlestickChart",
                    "TimeAndSales",
                    "Ladder",
                    "ComparisonChart",
                    "ProfileChart",
                ],
            )),
        }
    }
}

impl ContentKind {
    pub const ALL: &'static [ContentKind] = &[
        ContentKind::HeatmapChart,
        ContentKind::CandlestickChart,
        ContentKind::ProfileChart,
        ContentKind::TimeAndSales,
        ContentKind::Ladder,
        ContentKind::ComparisonChart,
    ];

    pub fn to_chart_type(self) -> ChartType {
        match self {
            ContentKind::HeatmapChart => ChartType::Heatmap,
            ContentKind::CandlestickChart => ChartType::Candlestick,
            ContentKind::TimeAndSales => ChartType::Candlestick,
            ContentKind::Ladder => ChartType::Candlestick,
            ContentKind::ComparisonChart => ChartType::Candlestick,
            ContentKind::Starter => ChartType::Candlestick,
            ContentKind::ProfileChart => ChartType::Candlestick,
        }
    }
}

impl std::fmt::Display for ContentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentKind::Starter => write!(f, "Starter"),
            ContentKind::HeatmapChart => write!(f, "Heatmap"),
            ContentKind::CandlestickChart => write!(f, "Candlestick"),
            ContentKind::TimeAndSales => write!(f, "Time & Sales"),
            ContentKind::Ladder => write!(f, "Ladder"),
            ContentKind::ComparisonChart => write!(f, "Comparison"),
            ContentKind::ProfileChart => write!(f, "Profile"),
        }
    }
}

/// Link group for synchronized panes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LinkGroup(pub u8);

impl LinkGroup {
    pub const ALL: [LinkGroup; 9] = [
        LinkGroup(1),
        LinkGroup(2),
        LinkGroup(3),
        LinkGroup(4),
        LinkGroup(5),
        LinkGroup(6),
        LinkGroup(7),
        LinkGroup(8),
        LinkGroup(9),
    ];
}

impl std::fmt::Display for LinkGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Persisted configuration for a single study instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyInstanceConfig {
    pub study_id: String,
    pub enabled: bool,
    pub parameters: std::collections::HashMap<String, serde_json::Value>,
}

/// Pane settings — PERSISTED to disk as part of the layout.
///
/// All fields in this struct are serialized and saved with the layout.
/// Runtime-only state (e.g. chart data, interaction state) lives in
/// `ChartState` and the GUI-layer `Content` enum instead.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    /// PERSISTED — chart basis (timeframe or tick count) selected by the user.
    pub selected_basis: Option<ChartBasis>,
    /// PERSISTED — content-type-specific visual configuration.
    pub visual_config: Option<VisualConfig>,
    /// PERSISTED — saved drawings (lines, boxes, fibs) for this pane.
    #[serde(default)]
    pub drawings: Vec<crate::drawing::SerializableDrawing>,
    /// PERSISTED — saved study (indicator) configurations for this pane.
    #[serde(default)]
    pub studies: Vec<StudyInstanceConfig>,
}

/// Visual configuration for different content types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VisualConfig {
    Heatmap(HeatmapConfig),
    Kline(KlineConfig),
    TimeAndSales(TimeAndSalesConfig),
    Ladder(LadderConfig),
    Comparison(ComparisonConfig),
    Profile(ProfileConfig),
}

impl VisualConfig {
    pub fn heatmap(self) -> Option<HeatmapConfig> {
        match self {
            VisualConfig::Heatmap(cfg) => Some(cfg),
            _ => None,
        }
    }

    pub fn kline(self) -> Option<KlineConfig> {
        match self {
            VisualConfig::Kline(cfg) => Some(cfg),
            _ => None,
        }
    }

    pub fn time_and_sales(self) -> Option<TimeAndSalesConfig> {
        match self {
            VisualConfig::TimeAndSales(cfg) => Some(cfg),
            _ => None,
        }
    }

    pub fn ladder(self) -> Option<LadderConfig> {
        match self {
            VisualConfig::Ladder(cfg) => Some(cfg),
            _ => None,
        }
    }

    pub fn comparison(self) -> Option<ComparisonConfig> {
        match self {
            VisualConfig::Comparison(cfg) => Some(cfg),
            _ => None,
        }
    }

    pub fn profile(self) -> Option<ProfileConfig> {
        match self {
            VisualConfig::Profile(cfg) => Some(cfg),
            _ => None,
        }
    }
}

// Heatmap visual configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatmapConfig {
    /// Minimum trade size in contracts (NOT dollar amount)
    /// Filters out trades smaller than this contract count
    /// Example: 5.0 = only show trades >= 5 contracts
    pub trade_size_filter: f32,
    /// Minimum orderbook order size to display (filter small orders)
    /// Value is in contracts
    pub order_size_filter: f32,
    /// Trade circle size scaling (None = fixed size, Some(100) = 100% scaling)
    pub trade_size_scale: Option<u16>,
    /// Coalescing strategy for merging similar-sized orders
    pub coalescing: Option<CoalesceKind>,
    /// Trade rendering mode (Sparse/Dense/Auto)
    #[serde(default)]
    pub rendering_mode: HeatmapRenderMode,
    /// Maximum trade markers to render (performance limit)
    #[serde(default = "default_max_trade_markers")]
    pub max_trade_markers: usize,
    /// Performance preset (auto-detected or manual)
    #[serde(default)]
    pub performance_preset: Option<String>,
}

fn default_max_trade_markers() -> usize {
    10_000
}

/// Heatmap rendering mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HeatmapRenderMode {
    /// Individual circles (best for low density)
    Sparse,
    /// Aggregated rectangles (best for high density)
    Dense,
    /// Automatically switch based on data density
    #[default]
    Auto,
}

impl std::fmt::Display for HeatmapRenderMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HeatmapRenderMode::Sparse => write!(f, "Sparse (Circles)"),
            HeatmapRenderMode::Dense => write!(f, "Dense (Rectangles)"),
            HeatmapRenderMode::Auto => write!(f, "Auto"),
        }
    }
}

impl Default for HeatmapConfig {
    fn default() -> Self {
        Self {
            trade_size_filter: 0.0,
            order_size_filter: 0.0,
            trade_size_scale: Some(100),
            coalescing: Some(CoalesceKind::None),
            rendering_mode: HeatmapRenderMode::Auto,
            max_trade_markers: 10_000,
            performance_preset: None,
        }
    }
}

use crate::domain::chart::heatmap::CoalesceKind;

/// Which candlestick color field is currently being edited in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandleColorField {
    BullBody,
    BearBody,
    BullWick,
    BearWick,
    BullBorder,
    BearBorder,
}

/// Candlestick visual style configuration.
///
/// Each field is `Option<Rgba>` — `None` means "use theme palette default".
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CandleStyle {
    pub bull_body_color: Option<crate::config::color::Rgba>,
    pub bear_body_color: Option<crate::config::color::Rgba>,
    pub bull_wick_color: Option<crate::config::color::Rgba>,
    pub bear_wick_color: Option<crate::config::color::Rgba>,
    pub bull_border_color: Option<crate::config::color::Rgba>,
    pub bear_border_color: Option<crate::config::color::Rgba>,
    /// When true, candle body opacity scales with volume (high volume = more opaque).
    #[serde(default)]
    pub volume_opacity: bool,
}

impl CandleStyle {
    /// Get the color for a given field.
    pub fn get_color(&self, field: CandleColorField) -> Option<crate::config::color::Rgba> {
        match field {
            CandleColorField::BullBody => self.bull_body_color,
            CandleColorField::BearBody => self.bear_body_color,
            CandleColorField::BullWick => self.bull_wick_color,
            CandleColorField::BearWick => self.bear_wick_color,
            CandleColorField::BullBorder => self.bull_border_color,
            CandleColorField::BearBorder => self.bear_border_color,
        }
    }

    /// Set the color for a given field.
    pub fn set_color(&mut self, field: CandleColorField, color: Option<crate::config::color::Rgba>) {
        match field {
            CandleColorField::BullBody => self.bull_body_color = color,
            CandleColorField::BearBody => self.bear_body_color = color,
            CandleColorField::BullWick => self.bull_wick_color = color,
            CandleColorField::BearWick => self.bear_wick_color = color,
            CandleColorField::BullBorder => self.bull_border_color = color,
            CandleColorField::BearBorder => self.bear_border_color = color,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KlineConfig {
    /// PERSISTED — whether the volume sub-chart is visible.
    pub show_volume: bool,
    /// PERSISTED — color scheme identifier (e.g. "default").
    pub color_scheme: String,
    /// PERSISTED — candlestick visual style (body/wick/border colors).
    #[serde(default)]
    pub candle_style: CandleStyle,
    /// RUNTIME ONLY — which color field is currently being edited in the UI.
    /// Skipped during serialization; always `None` on load.
    #[serde(skip)]
    pub editing_color: Option<CandleColorField>,
    /// RUNTIME ONLY — whether to show debug performance overlay (FPS, frame time, etc.).
    #[serde(skip)]
    pub show_debug_info: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeAndSalesConfig {
    pub max_rows: usize,
    pub trade_size_filter: f32,
    pub trade_retention_secs: u64,
    pub show_delta: bool,
    pub stacked_bar: Option<(bool, StackedBarRatio)>, // (is_compact, ratio)
}

impl Default for TimeAndSalesConfig {
    fn default() -> Self {
        Self {
            max_rows: 100,
            trade_size_filter: 0.0,
            trade_retention_secs: 300, // 5 minutes
            show_delta: true,
            stacked_bar: Some((false, StackedBarRatio::Volume)), // Full mode, Volume
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LadderConfig {
    pub levels: usize,
    pub show_spread: bool,
    pub show_chase_tracker: bool,
    pub trade_retention_secs: u64,
}

impl Default for LadderConfig {
    fn default() -> Self {
        Self {
            levels: 20,
            show_spread: true,
            show_chase_tracker: true,
            trade_retention_secs: 300, // 5 minutes
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComparisonConfig {
    pub normalize: Option<bool>,
    /// Map of ticker symbol strings to colors (e.g., "ESH5" -> Rgba)
    #[serde(default)]
    pub colors: Vec<(String, crate::config::color::Rgba)>,
    /// Map of ticker symbol strings to custom names
    #[serde(default)]
    pub names: Vec<(String, String)>,
}

/// Profile line style — mirrors `LineStyleValue` in the study crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ProfileLineStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

impl ProfileLineStyle {
    pub const ALL: [ProfileLineStyle; 3] = [
        ProfileLineStyle::Solid,
        ProfileLineStyle::Dashed,
        ProfileLineStyle::Dotted,
    ];
}

impl std::fmt::Display for ProfileLineStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileLineStyle::Solid => write!(f, "Solid"),
            ProfileLineStyle::Dashed => write!(f, "Dashed"),
            ProfileLineStyle::Dotted => write!(f, "Dotted"),
        }
    }
}

/// Profile line extend direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ProfileExtendDirection {
    #[default]
    None,
    Left,
    Right,
    Both,
}

impl ProfileExtendDirection {
    pub const ALL: [ProfileExtendDirection; 4] = [
        ProfileExtendDirection::None,
        ProfileExtendDirection::Left,
        ProfileExtendDirection::Right,
        ProfileExtendDirection::Both,
    ];
}

impl std::fmt::Display for ProfileExtendDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileExtendDirection::None => write!(f, "None"),
            ProfileExtendDirection::Left => write!(f, "Left"),
            ProfileExtendDirection::Right => write!(f, "Right"),
            ProfileExtendDirection::Both => write!(f, "Both"),
        }
    }
}

/// Profile volume node detection method.
// TODO: unify with study::output::NodeDetectionMethod
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ProfileNodeDetectionMethod {
    Percentile,
    #[default]
    Relative,
    StdDev,
}

impl ProfileNodeDetectionMethod {
    pub const ALL: [ProfileNodeDetectionMethod; 3] = [
        ProfileNodeDetectionMethod::Percentile,
        ProfileNodeDetectionMethod::Relative,
        ProfileNodeDetectionMethod::StdDev,
    ];
}

impl std::fmt::Display for ProfileNodeDetectionMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileNodeDetectionMethod::Percentile => write!(f, "Percentile"),
            ProfileNodeDetectionMethod::Relative => write!(f, "Relative"),
            ProfileNodeDetectionMethod::StdDev => write!(f, "Std Dev"),
        }
    }
}

/// Profile chart display type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ProfileDisplayType {
    #[default]
    Volume,
    BidAskVolume,
    Delta,
    DeltaAndTotal,
    DeltaPercentage,
}

impl std::fmt::Display for ProfileDisplayType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileDisplayType::Volume => write!(f, "Volume"),
            ProfileDisplayType::BidAskVolume => write!(f, "Bid/Ask Volume"),
            ProfileDisplayType::Delta => write!(f, "Delta"),
            ProfileDisplayType::DeltaAndTotal => write!(f, "Delta & Total"),
            ProfileDisplayType::DeltaPercentage => write!(f, "Delta %"),
        }
    }
}

/// Profile chart split unit
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ProfileSplitUnit {
    #[default]
    Days,
    Hours,
    Minutes,
}

impl ProfileSplitUnit {
    pub const ALL: &'static [Self] = &[
        Self::Days,
        Self::Hours,
        Self::Minutes,
    ];
}

impl std::fmt::Display for ProfileSplitUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileSplitUnit::Days => write!(f, "Days"),
            ProfileSplitUnit::Hours => write!(f, "Hours"),
            ProfileSplitUnit::Minutes => write!(f, "Minutes"),
        }
    }
}

/// Profile chart visual configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    // Display
    #[serde(default)]
    pub display_type: ProfileDisplayType,

    // Split interval
    #[serde(default)]
    pub split_unit: ProfileSplitUnit,
    #[serde(default = "default_split_value")]
    pub split_value: i64,
    #[serde(default = "default_max_profiles")]
    pub max_profiles: i64,

    // Tick grouping
    #[serde(default = "default_true")]
    pub auto_grouping: bool,
    #[serde(default = "default_one")]
    pub auto_group_factor: i64,
    #[serde(default = "default_one")]
    pub manual_ticks: i64,

    // Value Area
    #[serde(default = "default_va_pct")]
    pub value_area_pct: f32,
    #[serde(default = "default_true")]
    pub show_va_highlight: bool,
    pub vah_color: Option<crate::config::color::Rgba>,
    pub val_color: Option<crate::config::color::Rgba>,

    // Value Area (expanded)
    #[serde(default = "default_true")]
    pub show_va_fill: bool,
    pub va_fill_color: Option<crate::config::color::Rgba>,
    #[serde(default = "default_va_fill_opacity")]
    pub va_fill_opacity: f32,
    #[serde(default = "default_line_width")]
    pub vah_line_width: f32,
    #[serde(default)]
    pub vah_line_style: ProfileLineStyle,
    #[serde(default = "default_line_width")]
    pub val_line_width: f32,
    #[serde(default)]
    pub val_line_style: ProfileLineStyle,
    #[serde(default)]
    pub va_extend: ProfileExtendDirection,
    #[serde(default)]
    pub show_va_labels: bool,

    // POC
    #[serde(default = "default_true")]
    pub show_poc: bool,
    pub poc_color: Option<crate::config::color::Rgba>,
    #[serde(default = "default_poc_width")]
    pub poc_line_width: f32,
    #[serde(default)]
    pub poc_line_style: ProfileLineStyle,
    #[serde(default)]
    pub poc_extend: ProfileExtendDirection,
    #[serde(default)]
    pub show_poc_label: bool,

    // Volume Nodes
    #[serde(default)]
    pub show_hvn: bool,
    #[serde(default)]
    pub show_lvn: bool,
    #[serde(default = "default_hvn_threshold")]
    pub hvn_threshold: f32,
    #[serde(default = "default_lvn_threshold")]
    pub lvn_threshold: f32,
    pub hvn_color: Option<crate::config::color::Rgba>,
    pub lvn_color: Option<crate::config::color::Rgba>,

    // HVN expanded
    #[serde(default)]
    pub hvn_method: ProfileNodeDetectionMethod,
    #[serde(default)]
    pub show_hvn_zones: bool,
    pub hvn_zone_color: Option<crate::config::color::Rgba>,
    #[serde(default = "default_zone_opacity")]
    pub hvn_zone_opacity: f32,
    #[serde(default)]
    pub show_peak_line: bool,
    pub peak_color: Option<crate::config::color::Rgba>,
    #[serde(default)]
    pub peak_line_style: ProfileLineStyle,
    #[serde(default = "default_line_width")]
    pub peak_line_width: f32,
    #[serde(default)]
    pub show_peak_label: bool,

    // LVN expanded
    #[serde(default)]
    pub lvn_method: ProfileNodeDetectionMethod,
    #[serde(default)]
    pub show_lvn_zones: bool,
    pub lvn_zone_color: Option<crate::config::color::Rgba>,
    #[serde(default = "default_zone_opacity")]
    pub lvn_zone_opacity: f32,
    #[serde(default)]
    pub show_valley_line: bool,
    pub valley_color: Option<crate::config::color::Rgba>,
    #[serde(default)]
    pub valley_line_style: ProfileLineStyle,
    #[serde(default = "default_line_width")]
    pub valley_line_width: f32,
    #[serde(default)]
    pub show_valley_label: bool,

    // Colors
    pub volume_color: Option<crate::config::color::Rgba>,
    pub bid_color: Option<crate::config::color::Rgba>,
    pub ask_color: Option<crate::config::color::Rgba>,
    #[serde(default = "default_opacity")]
    pub opacity: f32,

    // Settings tab state
    #[serde(default)]
    pub settings_tab: u8,
}

fn default_true() -> bool {
    true
}
fn default_one() -> i64 {
    1
}
fn default_split_value() -> i64 {
    1
}
fn default_max_profiles() -> i64 {
    20
}
fn default_va_pct() -> f32 {
    0.7
}
fn default_poc_width() -> f32 {
    1.5
}
fn default_hvn_threshold() -> f32 {
    0.85
}
fn default_lvn_threshold() -> f32 {
    0.15
}
fn default_opacity() -> f32 {
    0.7
}
fn default_va_fill_opacity() -> f32 {
    0.08
}
fn default_line_width() -> f32 {
    1.0
}
fn default_zone_opacity() -> f32 {
    0.15
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            display_type: ProfileDisplayType::default(),
            split_unit: ProfileSplitUnit::default(),
            split_value: default_split_value(),
            max_profiles: default_max_profiles(),
            auto_grouping: true,
            auto_group_factor: 1,
            manual_ticks: 1,
            value_area_pct: default_va_pct(),
            show_va_highlight: true,
            vah_color: None,
            val_color: None,
            show_va_fill: true,
            va_fill_color: None,
            va_fill_opacity: default_va_fill_opacity(),
            vah_line_width: default_line_width(),
            vah_line_style: ProfileLineStyle::default(),
            val_line_width: default_line_width(),
            val_line_style: ProfileLineStyle::default(),
            va_extend: ProfileExtendDirection::default(),
            show_va_labels: false,
            show_poc: true,
            poc_color: None,
            poc_line_width: default_poc_width(),
            poc_line_style: ProfileLineStyle::default(),
            poc_extend: ProfileExtendDirection::default(),
            show_poc_label: false,
            show_hvn: false,
            show_lvn: false,
            hvn_threshold: default_hvn_threshold(),
            lvn_threshold: default_lvn_threshold(),
            hvn_color: None,
            lvn_color: None,
            hvn_method: ProfileNodeDetectionMethod::default(),
            show_hvn_zones: false,
            hvn_zone_color: None,
            hvn_zone_opacity: default_zone_opacity(),
            show_peak_line: false,
            peak_color: None,
            peak_line_style: ProfileLineStyle::default(),
            peak_line_width: default_line_width(),
            show_peak_label: false,
            lvn_method: ProfileNodeDetectionMethod::default(),
            show_lvn_zones: false,
            lvn_zone_color: None,
            lvn_zone_opacity: default_zone_opacity(),
            show_valley_line: false,
            valley_color: None,
            valley_line_style: ProfileLineStyle::default(),
            valley_line_width: default_line_width(),
            show_valley_label: false,
            volume_color: None,
            bid_color: None,
            ask_color: None,
            opacity: default_opacity(),
            settings_tab: 0,
        }
    }
}
