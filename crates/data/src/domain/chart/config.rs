//! Chart configuration types

use crate::domain::types::DateRange;
use crate::domain::{FuturesTicker, Timeframe};
use serde::{Deserialize, Serialize};

/// Chart configuration (what to display)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartConfig {
    /// Ticker to display
    pub ticker: FuturesTicker,
    /// Timeframe (M1, M5, H1, etc.) or tick count
    pub basis: ChartBasis,
    /// Date range to load
    pub date_range: DateRange,
    /// Chart type
    pub chart_type: ChartType,
}

/// Chart basis (time-based or tick-based)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChartBasis {
    /// Time-based (M1, M5, H1, etc.)
    Time(Timeframe),
    /// Tick-based (50T, 100T, etc.)
    Tick(u32),
}

impl ChartBasis {
    pub fn is_time(&self) -> bool {
        matches!(self, ChartBasis::Time(_))
    }

    pub fn is_tick(&self) -> bool {
        matches!(self, ChartBasis::Tick(_))
    }

    pub fn timeframe(&self) -> Option<Timeframe> {
        match self {
            ChartBasis::Time(tf) => Some(*tf),
            ChartBasis::Tick(_) => None,
        }
    }

    pub fn tick_count(&self) -> Option<u32> {
        match self {
            ChartBasis::Time(_) => None,
            ChartBasis::Tick(count) => Some(*count),
        }
    }
}

impl std::fmt::Display for ChartBasis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChartBasis::Time(tf) => write!(f, "{:?}", tf),
            ChartBasis::Tick(count) => write!(f, "{}T", count),
        }
    }
}

impl From<Timeframe> for ChartBasis {
    fn from(tf: Timeframe) -> Self {
        ChartBasis::Time(tf)
    }
}

/// Chart type (candlestick, heatmap, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartType {
    /// Standard candlestick chart (also hosts footprint studies)
    Candlestick,
    /// Line chart
    Line,
    /// Heikin-Ashi candlestick chart
    HeikinAshi,
    /// Heatmap (orderbook visualization)
    Heatmap,
}

impl std::fmt::Display for ChartType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChartType::Candlestick => write!(f, "Candlestick"),
            ChartType::Line => write!(f, "Line"),
            ChartType::HeikinAshi => write!(f, "Heikin-Ashi"),
            ChartType::Heatmap => write!(f, "Heatmap"),
        }
    }
}
