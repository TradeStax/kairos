//! Chart State
//!
//! Chart-specific state that is kept in memory only (NOT persisted).
//! Chart data is derived from cache, not stored in saved state.

use crate::domain::FuturesTickerInfo;
use crate::domain::chart::{ChartConfig, ChartData, LoadingStatus};

/// Chart state (in-memory only, NOT persisted)
///
/// This represents the runtime state of a chart including configuration
/// and loaded data. Chart data is NEVER persisted - it's always derived
/// from cache when needed.
#[derive(Debug, Clone)]
pub struct ChartState {
    /// Chart configuration (what to display)
    pub config: ChartConfig,

    /// Loaded chart data (trades, candles, depth)
    pub data: ChartData,

    /// Current loading status
    pub loading_status: LoadingStatus,

    /// Ticker information (specs, tick size, etc.)
    pub ticker_info: FuturesTickerInfo,
}

impl ChartState {
    /// Create new chart state
    pub fn new(config: ChartConfig, ticker_info: FuturesTickerInfo) -> Self {
        Self {
            config,
            data: ChartData::from_trades(vec![], vec![]),
            loading_status: LoadingStatus::Idle,
            ticker_info,
        }
    }

    /// Update chart data
    pub fn set_data(&mut self, data: ChartData) {
        self.data = data;
        self.loading_status = LoadingStatus::Ready;
    }

    /// Update loading status
    pub fn set_status(&mut self, status: LoadingStatus) {
        self.loading_status = status;
    }

    /// Check if data is loaded
    pub fn is_loaded(&self) -> bool {
        !self.data.trades.is_empty() || !self.data.candles.is_empty()
    }

    /// Check if currently loading
    pub fn is_loading(&self) -> bool {
        self.loading_status.is_loading()
    }

    /// Get trade count
    pub fn trade_count(&self) -> usize {
        self.data.trade_count()
    }

    /// Get candle count
    pub fn candle_count(&self) -> usize {
        self.data.candle_count()
    }

    /// Clear all data (for refresh)
    pub fn clear_data(&mut self) {
        self.data = ChartData::from_trades(vec![], vec![]);
        self.loading_status = LoadingStatus::Idle;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::DateRange;
    use crate::domain::chart::{ChartBasis, ChartType};
    use crate::domain::{FuturesTicker, FuturesVenue, Timeframe};
    use chrono::NaiveDate;

    #[test]
    fn test_chart_state_lifecycle() {
        let ticker = FuturesTicker::new("ES.c.0", FuturesVenue::CMEGlobex);
        let ticker_info = FuturesTickerInfo::new(ticker, 0.25, 1.0, 50.0);

        let config = ChartConfig {
            ticker,
            basis: ChartBasis::Time(Timeframe::M5),
            date_range: DateRange::new(
                NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2025, 1, 7).unwrap(),
            ),
            chart_type: ChartType::Candlestick,
        };

        let mut state = ChartState::new(config, ticker_info);

        assert!(!state.is_loaded());
        assert_eq!(state.trade_count(), 0);

        state.set_status(LoadingStatus::Ready);
        assert!(state.loading_status.is_ready());
    }
}
