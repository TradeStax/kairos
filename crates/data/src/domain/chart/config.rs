//! Chart configuration types — what a chart displays and how it scales.

use serde::{Deserialize, Serialize};

use crate::domain::core::types::DateRange;
use crate::domain::instrument::futures::{FuturesTicker, Timeframe};

/// Top-level chart configuration describing what data to display.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartConfig {
    /// Instrument to chart
    pub ticker: FuturesTicker,
    /// Time-based or tick-based aggregation
    pub basis: ChartBasis,
    /// Date range of data to load
    pub date_range: DateRange,
    /// Visual chart style
    pub chart_type: ChartType,
}

/// Chart aggregation basis — either time-based or tick-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChartBasis {
    /// Aggregate by wall-clock duration
    Time(Timeframe),
    /// Aggregate by trade count
    Tick(u32),
}

impl ChartBasis {
    /// Return `true` for time-based aggregation
    #[must_use]
    pub fn is_time(&self) -> bool {
        matches!(self, ChartBasis::Time(_))
    }

    /// Return `true` for tick-based aggregation
    #[must_use]
    pub fn is_tick(&self) -> bool {
        matches!(self, ChartBasis::Tick(_))
    }

    /// Return the timeframe if time-based, `None` otherwise
    #[must_use]
    pub fn timeframe(&self) -> Option<Timeframe> {
        match self {
            ChartBasis::Time(tf) => Some(*tf),
            ChartBasis::Tick(_) => None,
        }
    }

    /// Return the tick count if tick-based, `None` otherwise
    #[must_use]
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

/// Visual chart rendering style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartType {
    /// Traditional OHLC candlesticks
    Candlestick,
    /// Close-only line chart
    Line,
    /// Heikin-Ashi smoothed candles
    HeikinAshi,
    /// Order book depth heatmap
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

/// View layout and autoscale configuration for a chart pane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewConfig {
    /// Horizontal split ratios for sub-panes
    pub splits: Vec<f32>,
    /// Autoscale mode
    pub autoscale: Option<Autoscale>,
    /// Vertical split ratios for side panels
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

/// Autoscale mode for the price axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Autoscale {
    /// Keep the latest candle centered
    CenterLatest,
    /// Fit all visible data
    FitAll,
    /// Manual scaling only
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
