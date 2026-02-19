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

/// Display mode for footprint studies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FootprintMode {
    /// Grid cells with values at each price level
    Box,
    /// Horizontal bars extending from candle
    #[default]
    Profile,
}

impl std::fmt::Display for FootprintMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FootprintMode::Box => write!(f, "Box"),
            FootprintMode::Profile => write!(f, "Profile"),
        }
    }
}

/// Data type shown in the footprint study
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FootprintType {
    /// Total volume (buy + sell) at each price level
    #[default]
    Volume,
    /// Separate ask and bid volumes on each side
    BidAskSplit,
    /// Buy - Sell difference per price level
    Delta,
    /// Combined: total volume as bar size + delta as adjacent colored sub-bars
    DeltaAndVolume,
}

impl FootprintType {
    pub const ALL: &'static [FootprintType] = &[
        FootprintType::Volume,
        FootprintType::BidAskSplit,
        FootprintType::Delta,
        FootprintType::DeltaAndVolume,
    ];
}

impl std::fmt::Display for FootprintType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FootprintType::Volume => write!(f, "Volume"),
            FootprintType::BidAskSplit => write!(f, "Bid/Ask Split"),
            FootprintType::Delta => write!(f, "Delta"),
            FootprintType::DeltaAndVolume => write!(f, "Delta + Volume"),
        }
    }
}

/// Candle position relative to the study bars
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CandlePosition {
    /// No candle drawn, study bars only
    None,
    /// Candle on left, bars extend right
    #[default]
    Left,
    /// Candle centered, bars on both sides
    Center,
    /// Candle on right, bars extend left
    Right,
}

impl CandlePosition {
    pub const ALL: &'static [CandlePosition] = &[
        CandlePosition::None,
        CandlePosition::Left,
        CandlePosition::Center,
        CandlePosition::Right,
    ];
}

impl std::fmt::Display for CandlePosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CandlePosition::None => write!(f, "None"),
            CandlePosition::Left => write!(f, "Left"),
            CandlePosition::Center => write!(f, "Center"),
            CandlePosition::Right => write!(f, "Right"),
        }
    }
}

/// Active footprint study configuration (None = standard candles)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FootprintStudyConfig {
    pub mode: FootprintMode,
    pub study_type: FootprintType,
    pub scaling: ClusterScaling,
    pub candle_position: CandlePosition,
}

impl Default for FootprintStudyConfig {
    fn default() -> Self {
        Self {
            mode: FootprintMode::Profile,
            study_type: FootprintType::Volume,
            scaling: ClusterScaling::Sqrt,
            candle_position: CandlePosition::Left,
        }
    }
}

/// Cluster scaling for footprint charts
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[derive(Default)]
pub enum ClusterScaling {
    Linear,
    #[default]
    Sqrt,
    Log,
    VisibleRange,
    Datapoint,
    Hybrid { weight: f32 },
}

// SAFETY: Manual Eq is sound here because f32 `weight` in Hybrid variant
// is always a finite value (0.0..=1.0 percentage). NaN is never constructed.
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
}

/// Kline indicator types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, enum_map::Enum)]
pub enum KlineIndicator {
    Volume,
    Delta,
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
            KlineIndicator::Delta => write!(f, "Delta"),
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
            KlineIndicator::Delta => "Delta",
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
}

impl KlineIndicator {
    /// Get all indicators available for a given market
    /// For futures, all indicators are available
    pub fn for_market(_market_type: &str) -> Vec<KlineIndicator> {
        vec![
            KlineIndicator::Volume,
            KlineIndicator::Delta,
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
            KlineIndicator::Delta,
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

    /// Whether this indicator overlays on the main price chart
    /// rather than rendering in a separate panel below.
    ///
    /// Overlay indicators (SMA, EMA, Bollinger Bands) produce values
    /// in price space and are drawn directly on the candlestick chart.
    /// Panel indicators (Volume, Delta, RSI, MACD, OI) have their own
    /// Y-axis scale and are shown in separate sub-panels.
    pub fn is_overlay(&self) -> bool {
        matches!(
            self,
            KlineIndicator::Sma20
                | KlineIndicator::Sma50
                | KlineIndicator::Sma200
                | KlineIndicator::Ema9
                | KlineIndicator::Ema21
                | KlineIndicator::BollingerBands
        )
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

    // SAFETY: Manual Eq is sound here because f32 thresholds in Average/First/Max
    // are always finite values set via UI sliders. NaN is never constructed.
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
        CandlePosition, ClusterScaling, FootprintMode, FootprintStudyConfig, FootprintType,
        KlineDataPoint, KlineTrades, NPoc, PointOfControl,
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
