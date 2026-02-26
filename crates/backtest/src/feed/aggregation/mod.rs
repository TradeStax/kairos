pub mod candle;
pub mod multi_timeframe;

pub use candle::{CandleAggregator, PartialCandle};
pub use multi_timeframe::{AggregatorKey, MultiTimeframeAggregator};
