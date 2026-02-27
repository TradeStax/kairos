//! Candle aggregation from trade ticks.
//!
//! Provides time-based candle construction at arbitrary timeframes:
//!
//! - [`CandleAggregator`] handles a single instrument/timeframe pair
//! - [`MultiTimeframeAggregator`] manages many pairs simultaneously

pub mod candle;
pub mod multi_timeframe;

pub use candle::{CandleAggregator, PartialCandle};
pub use multi_timeframe::{AggregatorKey, MultiTimeframeAggregator};
