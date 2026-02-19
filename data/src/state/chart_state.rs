//! Chart State
//!
//! Chart-specific state that is kept in memory only (NOT persisted).
//! Chart data is derived from cache, not stored in saved state.

use crate::domain::chart::{ChartBasis, ChartConfig, ChartData, LoadingStatus};
use crate::domain::{Candle, FuturesTickerInfo, Side, Trade, Timestamp, Volume};

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

    /// Append a live trade and update the latest candle.
    ///
    /// For time-based charts, trades are bucketed into candle periods.
    /// For tick-based charts, a new candle starts every N trades.
    pub fn append_live_trade(&mut self, trade: Trade) {
        self.data.trades.push(trade);

        let (buy_vol, sell_vol) = match trade.side {
            Side::Buy | Side::Bid => (Volume(trade.quantity.0), Volume(0.0)),
            Side::Sell | Side::Ask => (Volume(0.0), Volume(trade.quantity.0)),
        };

        match self.config.basis {
            ChartBasis::Time(tf) => {
                let interval = tf.to_milliseconds();
                if interval == 0 {
                    return;
                }
                let bucket_time = (trade.time.to_millis() / interval) * interval;

                if let Some(last_candle) = self.data.candles.last_mut() {
                    if last_candle.time.to_millis() == bucket_time {
                        last_candle.high = last_candle.high.max(trade.price);
                        last_candle.low = last_candle.low.min(trade.price);
                        last_candle.close = trade.price;
                        last_candle.buy_volume = Volume(
                            last_candle.buy_volume.0 + buy_vol.0,
                        );
                        last_candle.sell_volume = Volume(
                            last_candle.sell_volume.0 + sell_vol.0,
                        );
                        return;
                    }
                }
                // New candle period
                self.data.candles.push(Candle {
                    time: Timestamp::from_millis(bucket_time),
                    open: trade.price,
                    high: trade.price,
                    low: trade.price,
                    close: trade.price,
                    buy_volume: buy_vol,
                    sell_volume: sell_vol,
                });
            }
            ChartBasis::Tick(count) => {
                let count = count as usize;
                if count == 0 {
                    return;
                }

                // Count trades in current candle: total trades minus
                // trades accounted for by previous completed candles
                let num_candles = self.data.candles.len();
                let num_trades = self.data.trades.len();
                let completed_candles = if num_candles > 0 { num_candles - 1 } else { 0 };
                let trades_in_current =
                    num_trades.saturating_sub(completed_candles * count);

                if let Some(last_candle) = self.data.candles.last_mut() {
                    if trades_in_current <= count {
                        last_candle.high = last_candle.high.max(trade.price);
                        last_candle.low = last_candle.low.min(trade.price);
                        last_candle.close = trade.price;
                        last_candle.buy_volume = Volume(
                            last_candle.buy_volume.0 + buy_vol.0,
                        );
                        last_candle.sell_volume = Volume(
                            last_candle.sell_volume.0 + sell_vol.0,
                        );
                        return;
                    }
                }
                // Start new tick candle
                self.data.candles.push(Candle {
                    time: trade.time,
                    open: trade.price,
                    high: trade.price,
                    low: trade.price,
                    close: trade.price,
                    buy_volume: buy_vol,
                    sell_volume: sell_vol,
                });
            }
        }
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
