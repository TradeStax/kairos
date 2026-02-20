//! Kline (candlestick) chart types: footprint, cluster scaling, data points

use crate::domain::entities::Candle;
use crate::domain::types::Timestamp;
use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum ClusterScaling {
    Linear,
    #[default]
    Sqrt,
    Log,
    VisibleRange,
    Datapoint,
    Hybrid {
        weight: f32,
    },
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
            ClusterScaling::Hybrid { weight } => {
                write!(f, "Hybrid ({:.1})", weight)
            }
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
