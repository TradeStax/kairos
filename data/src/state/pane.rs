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
            other => Err(serde::de::Error::unknown_variant(
                other,
                &[
                    "Starter",
                    "HeatmapChart",
                    "CandlestickChart",
                    "TimeAndSales",
                    "Ladder",
                    "ComparisonChart",
                ],
            )),
        }
    }
}

impl ContentKind {
    pub const ALL: &'static [ContentKind] = &[
        ContentKind::HeatmapChart,
        ContentKind::CandlestickChart,
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

/// Pane settings (basis, visual config)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    pub selected_basis: Option<ChartBasis>,
    pub visual_config: Option<VisualConfig>,
    /// Saved drawings for this pane
    #[serde(default)]
    pub drawings: Vec<crate::drawing::SerializableDrawing>,
}

/// Visual configuration for different content types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VisualConfig {
    Heatmap(HeatmapConfig),
    Kline(KlineConfig),
    TimeAndSales(TimeAndSalesConfig),
    Ladder(LadderConfig),
    Comparison(ComparisonConfig),
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
/// Each field is `Option<Color>` — `None` means "use theme palette default".
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CandleStyle {
    pub bull_body_color: Option<iced_core::Color>,
    pub bear_body_color: Option<iced_core::Color>,
    pub bull_wick_color: Option<iced_core::Color>,
    pub bear_wick_color: Option<iced_core::Color>,
    pub bull_border_color: Option<iced_core::Color>,
    pub bear_border_color: Option<iced_core::Color>,
}

impl CandleStyle {
    /// Get the color for a given field.
    pub fn get_color(&self, field: CandleColorField) -> Option<iced_core::Color> {
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
    pub fn set_color(&mut self, field: CandleColorField, color: Option<iced_core::Color>) {
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
    pub show_volume: bool,
    pub color_scheme: String,
    /// Candlestick visual style
    #[serde(default)]
    pub candle_style: CandleStyle,
    /// Which color field is currently being edited (UI-only, not persisted)
    #[serde(skip)]
    pub editing_color: Option<CandleColorField>,
    /// Active footprint study (None = standard candles only)
    #[serde(default)]
    pub footprint: Option<crate::domain::chart::FootprintStudyConfig>,
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
    /// Map of ticker symbol strings to colors (e.g., "ESH5" -> Color)
    #[serde(default)]
    pub colors: Vec<(String, iced_core::Color)>,
    /// Map of ticker symbol strings to custom names
    #[serde(default)]
    pub names: Vec<(String, String)>,
}
