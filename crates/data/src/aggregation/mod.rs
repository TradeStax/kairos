//! Trade-to-candle aggregation.
//!
//! Two modes:
//! - `aggregate_trades_to_candles` — group by time interval
//! - `aggregate_trades_to_ticks` — group by N trades
//! - `aggregate_candles_to_timeframe` — re-aggregate candles to a higher timeframe
//!
//! Input trades must be sorted by time. Returns `AggregationError::UnsortedTrades` otherwise.

mod tick_based;
mod time_based;

pub use tick_based::aggregate_trades_to_ticks;
pub use time_based::{aggregate_candles_to_timeframe, aggregate_trades_to_candles};

use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum AggregationError {
    #[error("No trades provided")]
    NoTrades,
    #[error("Invalid timeframe: {0}")]
    InvalidTimeframe(u64),
    #[error("Invalid tick count: {0}")]
    InvalidTickCount(u32),
    #[error("Trades not sorted by time")]
    UnsortedTrades,
}
