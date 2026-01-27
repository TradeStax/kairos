//! Chart UI Types
//!
//! UI-specific types for chart rendering and interaction.
//! These bridge the pure domain types with UI requirements.

use super::entities::Candle;
use super::types::Timestamp;
use serde::{Deserialize, Serialize};

/// View configuration for chart layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewConfig {
    pub splits: Vec<f32>,
    pub autoscale: Option<Autoscale>,
}

impl Default for ViewConfig {
    fn default() -> Self {
        Self {
            splits: vec![],
            autoscale: Some(Autoscale::CenterLatest),
        }
    }
}

/// Autoscale mode for charts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Autoscale {
    CenterLatest,
    FitAll,
    Disabled,
}

/// Kline chart kinds
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum KlineChartKind {
    Candles,
    Footprint {
        clusters: ClusterKind,
        scaling: ClusterScaling,
        studies: Vec<FootprintStudy>,
    },
}

impl Default for KlineChartKind {
    fn default() -> Self {
        KlineChartKind::Candles
    }
}

impl KlineChartKind {
    pub fn min_scaling(&self) -> f32 {
        match self {
            KlineChartKind::Candles => 0.25,
            KlineChartKind::Footprint { .. } => 0.1,
        }
    }

    pub fn max_scaling(&self) -> f32 {
        match self {
            KlineChartKind::Candles => 8.0,
            KlineChartKind::Footprint { .. } => 4.0,
        }
    }

    pub fn max_cell_width(&self) -> f32 {
        match self {
            KlineChartKind::Candles => 50.0,
            KlineChartKind::Footprint { .. } => 200.0,
        }
    }

    pub fn min_cell_width(&self) -> f32 {
        match self {
            KlineChartKind::Candles => 1.0,
            KlineChartKind::Footprint { .. } => 20.0,
        }
    }

    pub fn max_cell_height(&self) -> f32 {
        300.0 // Increased from 200.0 to allow more zoom in
    }

    pub fn min_cell_height(&self) -> f32 {
        0.1 // CRITICAL: Was 1.0, now 0.1 for truly infinite Y-axis zoom like X-axis
             // With initial cell_height ~4.0, this gives 40x zoom range (vs only 4x before)
             // At 0.1px/tick, 800px screen = 8000 price points visible (plenty for NQ!)
    }

    pub fn default_cell_width(&self) -> f32 {
        match self {
            KlineChartKind::Candles => 4.0,
            KlineChartKind::Footprint { .. } => 80.0,
        }
    }
}

/// Cluster kind for footprint charts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClusterKind {
    Delta,
    Volume,
    Trades,
    // Volume profile variants
    VolumeProfile,
    DeltaProfile,
    BidAsk,
}

impl ClusterKind {
    pub const ALL: &'static [ClusterKind] = &[
        ClusterKind::Delta,
        ClusterKind::Volume,
        ClusterKind::Trades,
        ClusterKind::VolumeProfile,
        ClusterKind::DeltaProfile,
        ClusterKind::BidAsk,
    ];
}

impl std::fmt::Display for ClusterKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClusterKind::Delta => write!(f, "Delta"),
            ClusterKind::Volume => write!(f, "Volume"),
            ClusterKind::Trades => write!(f, "Trades"),
            ClusterKind::VolumeProfile => write!(f, "Volume Profile"),
            ClusterKind::DeltaProfile => write!(f, "Delta Profile"),
            ClusterKind::BidAsk => write!(f, "Bid/Ask"),
        }
    }
}

impl Default for ClusterKind {
    fn default() -> Self {
        ClusterKind::Delta
    }
}

/// Cluster scaling for footprint charts
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ClusterScaling {
    Linear,
    Sqrt,
    Log,
    VisibleRange,
    Datapoint,
    Hybrid { weight: f32 },
}

impl Eq for ClusterScaling {}

impl ClusterScaling {
    pub const ALL: &'static [ClusterScaling] = &[
        ClusterScaling::Linear,
        ClusterScaling::Sqrt,
        ClusterScaling::Log,
        ClusterScaling::VisibleRange,
        ClusterScaling::Datapoint,
        ClusterScaling::Hybrid { weight: 0.5 },
    ];
}

impl std::fmt::Display for ClusterScaling {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClusterScaling::Linear => write!(f, "Linear"),
            ClusterScaling::Sqrt => write!(f, "Square Root"),
            ClusterScaling::Log => write!(f, "Logarithmic"),
            ClusterScaling::VisibleRange => write!(f, "Visible Range"),
            ClusterScaling::Datapoint => write!(f, "Datapoint"),
            ClusterScaling::Hybrid { weight } => write!(f, "Hybrid ({:.1})", weight),
        }
    }
}

impl Default for ClusterScaling {
    fn default() -> Self {
        ClusterScaling::Sqrt
    }
}

/// Footprint study types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FootprintStudy {
    Imbalance {
        threshold: u8,
        ignore_zeros: bool,
        color_scale: bool,
    },
    NPoC { lookback: usize },
    PointOfControl,
    ValueArea,
}

impl FootprintStudy {
    pub const ALL: &'static [FootprintStudy] = &[
        FootprintStudy::Imbalance {
            threshold: 70,
            ignore_zeros: false,
            color_scale: true,
        },
        FootprintStudy::NPoC { lookback: 100 },
        FootprintStudy::PointOfControl,
        FootprintStudy::ValueArea,
    ];

    pub fn is_same_type(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

impl std::fmt::Display for FootprintStudy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FootprintStudy::Imbalance { threshold, .. } => write!(f, "Imbalance ({}%)", threshold),
            FootprintStudy::NPoC { lookback } => write!(f, "NPoC ({})", lookback),
            FootprintStudy::PointOfControl => write!(f, "Point of Control"),
            FootprintStudy::ValueArea => write!(f, "Value Area"),
        }
    }
}

/// Kline data point (candle with metadata)
#[derive(Debug, Clone)]
pub struct KlineDataPoint {
    pub kline: Candle,
    pub total_volume: f32,
}

impl KlineDataPoint {
    pub fn from_candle(candle: Candle) -> Self {
        Self {
            total_volume: (candle.buy_volume.0 + candle.sell_volume.0) as f32,
            kline: candle,
        }
    }
}

/// Kline trades (for footprint charts)
#[derive(Debug, Clone, Default)]
pub struct KlineTrades {
    pub trades: Vec<TradeCell>,
}

#[derive(Debug, Clone)]
pub struct TradeCell {
    pub price: i64,
    pub buy_volume: f32,
    pub sell_volume: f32,
}

/// Point of control
#[derive(Debug, Clone, Copy)]
pub struct PointOfControl {
    pub price: i64,
    pub volume: f32,
}

/// Naked point of control
#[derive(Debug, Clone, Copy)]
pub struct NPoc {
    pub price: i64,
    pub time: Timestamp,
}

/// Chart indicator trait
pub trait Indicator: Send + Sync {
    fn name(&self) -> &str;
    fn enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
}

/// Kline indicator types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, enum_map::Enum)]
pub enum KlineIndicator {
    Volume,
    OpenInterest,
    Sma20,
    Sma50,
    Sma200,
    Ema9,
    Ema21,
    Rsi14,
    Macd,
    BollingerBands,
}

impl std::fmt::Display for KlineIndicator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KlineIndicator::Volume => write!(f, "Volume"),
            KlineIndicator::OpenInterest => write!(f, "Open Interest"),
            KlineIndicator::Sma20 => write!(f, "SMA 20"),
            KlineIndicator::Sma50 => write!(f, "SMA 50"),
            KlineIndicator::Sma200 => write!(f, "SMA 200"),
            KlineIndicator::Ema9 => write!(f, "EMA 9"),
            KlineIndicator::Ema21 => write!(f, "EMA 21"),
            KlineIndicator::Rsi14 => write!(f, "RSI 14"),
            KlineIndicator::Macd => write!(f, "MACD"),
            KlineIndicator::BollingerBands => write!(f, "Bollinger Bands"),
        }
    }
}

impl Indicator for KlineIndicator {
    fn name(&self) -> &str {
        match self {
            KlineIndicator::Volume => "Volume",
            KlineIndicator::OpenInterest => "Open Interest",
            KlineIndicator::Sma20 => "SMA 20",
            KlineIndicator::Sma50 => "SMA 50",
            KlineIndicator::Sma200 => "SMA 200",
            KlineIndicator::Ema9 => "EMA 9",
            KlineIndicator::Ema21 => "EMA 21",
            KlineIndicator::Rsi14 => "RSI 14",
            KlineIndicator::Macd => "MACD",
            KlineIndicator::BollingerBands => "Bollinger Bands",
        }
    }

    fn enabled(&self) -> bool {
        false
    }

    fn set_enabled(&mut self, _enabled: bool) {
        // This is a Copy type, so mutation doesn't persist
        // Enabled state should be tracked separately (e.g., in an EnumMap)
    }
}

impl KlineIndicator {
    /// Get all indicators available for a given market
    /// For futures, all indicators are available
    pub fn for_market(_market_type: &str) -> Vec<KlineIndicator> {
        vec![
            KlineIndicator::Volume,
            KlineIndicator::OpenInterest,
            KlineIndicator::Sma20,
            KlineIndicator::Sma50,
            KlineIndicator::Sma200,
            KlineIndicator::Ema9,
            KlineIndicator::Ema21,
            KlineIndicator::Rsi14,
            KlineIndicator::Macd,
            KlineIndicator::BollingerBands,
        ]
    }

    /// Get all available indicators
    pub fn all_indicators() -> Vec<KlineIndicator> {
        vec![
            KlineIndicator::Volume,
            KlineIndicator::OpenInterest,
            KlineIndicator::Sma20,
            KlineIndicator::Sma50,
            KlineIndicator::Sma200,
            KlineIndicator::Ema9,
            KlineIndicator::Ema21,
            KlineIndicator::Rsi14,
            KlineIndicator::Macd,
            KlineIndicator::BollingerBands,
        ]
    }
}

/// Heatmap indicator types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, enum_map::Enum)]
pub enum HeatmapIndicator {
    Volume,
    Delta,
    Trades,
}

impl std::fmt::Display for HeatmapIndicator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HeatmapIndicator::Volume => write!(f, "Volume"),
            HeatmapIndicator::Delta => write!(f, "Delta"),
            HeatmapIndicator::Trades => write!(f, "Trades"),
        }
    }
}

impl Indicator for HeatmapIndicator {
    fn name(&self) -> &str {
        match self {
            HeatmapIndicator::Volume => "Volume",
            HeatmapIndicator::Delta => "Delta",
            HeatmapIndicator::Trades => "Trades",
        }
    }

    fn enabled(&self) -> bool {
        false
    }

    fn set_enabled(&mut self, _enabled: bool) {
        // Copy type - state tracked separately
    }
}

impl HeatmapIndicator {
    /// Get all available indicators
    pub fn all_indicators() -> Vec<HeatmapIndicator> {
        vec![
            HeatmapIndicator::Volume,
            HeatmapIndicator::Delta,
            HeatmapIndicator::Trades,
        ]
    }
}

/// Heatmap study types
pub mod heatmap {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum HeatmapStudy {
        VolumeProfile(ProfileKind),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum ProfileKind {
        VisibleRange,
        FixedWindow { candles: usize },
        Fixed(usize),  // Alias for FixedWindow
    }

    #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
    pub enum CoalesceKind {
        None,
        Adjacent,
        All,
        Average(f32),
        First(f32),
        Max(f32),
    }

    // Manual Eq implementation - safe because we only use f32 values that are finite
    impl Eq for CoalesceKind {}

    impl CoalesceKind {
        pub fn threshold(&self) -> f32 {
            match self {
                CoalesceKind::Average(t) | CoalesceKind::First(t) | CoalesceKind::Max(t) => *t,
                _ => 0.0,
            }
        }

        pub fn with_threshold(&self, threshold: f32) -> Self {
            match self {
                CoalesceKind::Average(_) => CoalesceKind::Average(threshold),
                CoalesceKind::First(_) => CoalesceKind::First(threshold),
                CoalesceKind::Max(_) => CoalesceKind::Max(threshold),
                other => *other,
            }
        }
    }

    impl HeatmapStudy {
        pub const ALL: &'static [HeatmapStudy] = &[
            HeatmapStudy::VolumeProfile(ProfileKind::VisibleRange),
        ];

        pub fn is_same_type(&self, other: &Self) -> bool {
            std::mem::discriminant(self) == std::mem::discriminant(other)
        }
    }

    impl std::fmt::Display for HeatmapStudy {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                HeatmapStudy::VolumeProfile(kind) => write!(f, "Volume Profile ({})", kind),
            }
        }
    }

    impl std::fmt::Display for ProfileKind {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                ProfileKind::VisibleRange => write!(f, "Visible Range"),
                ProfileKind::FixedWindow { candles } => write!(f, "Fixed Window ({})", candles),
                ProfileKind::Fixed(n) => write!(f, "Fixed ({})", n),
            }
        }
    }

    pub const CLEANUP_THRESHOLD: usize = 1000;

    // Re-export HeatmapConfig for UI
    pub use crate::state::pane_config::HeatmapConfig as Config;
}

/// Kline-specific types
pub mod kline {
    pub use super::{
        ClusterKind, ClusterScaling, FootprintStudy, KlineDataPoint, KlineTrades, NPoc,
        PointOfControl,
    };

    // Re-export KlineConfig for UI
    pub use crate::state::pane_config::KlineConfig as Config;
}

/// UI indicator wrapper
#[derive(Debug, Clone, Copy)]
pub enum UiIndicator {
    Kline(KlineIndicator),
    Heatmap(HeatmapIndicator),
}

impl From<KlineIndicator> for UiIndicator {
    fn from(indicator: KlineIndicator) -> Self {
        UiIndicator::Kline(indicator)
    }
}

impl From<HeatmapIndicator> for UiIndicator {
    fn from(indicator: HeatmapIndicator) -> Self {
        UiIndicator::Heatmap(indicator)
    }
}
