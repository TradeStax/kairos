//! Trade-to-candle aggregation with time-based and tick-based modes.
//!
//! - [`aggregate_trades_to_candles`] — group trades by time interval into OHLCV candles
//! - [`aggregate_trades_to_ticks`] — group trades by count (N trades per candle)
//! - [`aggregate_candles_to_timeframe`] — re-aggregate candles to a higher timeframe
//!
//! Input trades must be sorted by time. In debug builds, unsorted input
//! returns [`AggregationError::UnsortedTrades`].

mod tick_based;
mod time_based;

pub use tick_based::aggregate_trades_to_ticks;
pub use time_based::{aggregate_candles_to_timeframe, aggregate_trades_to_candles};

use thiserror::Error;

/// Errors that can occur during trade/candle aggregation.
#[derive(Error, Debug, Clone)]
pub enum AggregationError {
    /// No trades were provided for aggregation
    #[error("No trades provided")]
    NoTrades,
    /// The specified timeframe in milliseconds is invalid (zero or too small)
    #[error("Invalid timeframe: {0}")]
    InvalidTimeframe(u64),
    /// The specified tick count is invalid (zero)
    #[error("Invalid tick count: {0}")]
    InvalidTickCount(u32),
    /// Input trades are not sorted by ascending time
    #[error("Trades not sorted by time")]
    UnsortedTrades,
}
