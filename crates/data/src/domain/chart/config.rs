//! Chart Configuration Types

use crate::domain::core::types::DateRange;
use crate::domain::instrument::futures::{FuturesTicker, Timeframe};
use serde::{Deserialize, Serialize};

/// Chart configuration (what to display)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartConfig {
    pub ticker: FuturesTicker,
    pub basis: ChartBasis,
    pub date_range: DateRange,
    pub chart_type: ChartType,
}

/// Chart basis (time-based or tick-based)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChartBasis {
    Time(Timeframe),
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
            ChartBasis::Time(tf) => write!(f, "{}", tf),
            ChartBasis::Tick(count) => write!(f, "{}T", count),
        }
    }
}

impl From<Timeframe> for ChartBasis {
    fn from(tf: Timeframe) -> Self {
        ChartBasis::Time(tf)
    }
}

/// Chart type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartType {
    Candlestick,
    Line,
    HeikinAshi,
    #[cfg(feature = "heatmap")]
    Heatmap,
}

impl std::fmt::Display for ChartType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChartType::Candlestick => write!(f, "Candlestick"),
            ChartType::Line => write!(f, "Line"),
            ChartType::HeikinAshi => write!(f, "Heikin-Ashi"),
            #[cfg(feature = "heatmap")]
            ChartType::Heatmap => write!(f, "Heatmap"),
        }
    }
}

/// View configuration for chart layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewConfig {
    pub splits: Vec<f32>,
    pub autoscale: Option<Autoscale>,
    #[serde(default)]
    pub side_splits: Vec<f32>,
}

impl Default for ViewConfig {
    fn default() -> Self {
        Self {
            splits: vec![],
            autoscale: Some(Autoscale::CenterLatest),
            side_splits: vec![],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chart_basis_display() {
        let time_basis = ChartBasis::Time(Timeframe::M5);
        assert_eq!(format!("{}", time_basis), "5m");

        let tick_basis = ChartBasis::Tick(50);
        assert_eq!(format!("{}", tick_basis), "50T");
    }
}
