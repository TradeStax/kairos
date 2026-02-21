//! Chart Domain Model
//!
//! Domain types for chart configuration, data, and UI bridge types.

mod config;
mod data;
pub mod heatmap_types;
mod kline_types;
mod view;

// Re-export all public types at the chart module level
pub use config::{ChartBasis, ChartConfig, ChartType};
pub use data::{ChartData, DataGap, DataGapKind, DataSegment, MergeResult};
pub use heatmap_types::{HeatmapIndicator, heatmap};
pub use kline_types::{KlineDataPoint, KlineTrades, NPoc, PointOfControl, TradeCell};
pub use view::{Autoscale, DataSchema, LoadingStatus, ViewConfig};

/// Kline-specific types
pub mod kline {
    pub use super::kline_types::{KlineDataPoint, KlineTrades, NPoc, PointOfControl};

    // Re-export KlineConfig for UI
    pub use crate::state::pane::KlineConfig as Config;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{Price, Quantity, Side, Timestamp};
    use crate::domain::{Timeframe, Trade};

    #[test]
    fn test_chart_basis_display() {
        let time_basis = ChartBasis::Time(Timeframe::M5);
        assert_eq!(format!("{}", time_basis), "M5");

        let tick_basis = ChartBasis::Tick(50);
        assert_eq!(format!("{}", tick_basis), "50T");
    }

    #[test]
    fn test_chart_data_creation() {
        let trades = vec![
            Trade::new(
                Timestamp(1000),
                Price::from_f32(100.0),
                Quantity(10.0),
                Side::Buy,
            ),
            Trade::new(
                Timestamp(2000),
                Price::from_f32(101.0),
                Quantity(5.0),
                Side::Sell,
            ),
        ];

        let candles = vec![];
        let chart_data = ChartData::from_trades(trades.clone(), candles);

        assert!(chart_data.has_trades());
        assert!(!chart_data.has_candles());
        assert!(!chart_data.has_depth());
        assert_eq!(chart_data.trade_count(), 2);
    }

    #[test]
    fn test_loading_status() {
        let status = LoadingStatus::Downloading {
            schema: DataSchema::Trades,
            days_total: 10,
            days_complete: 5,
            current_day: "2025-01-15".to_string(),
        };

        assert!(status.is_loading());
        assert!(!status.is_ready());
        assert!(!status.is_error());
    }
}
