pub mod aggregation;
pub mod data_feed;
pub mod provider;

pub use aggregation::candle::{CandleAggregator, PartialCandle};
pub use aggregation::multi_timeframe::{AggregatorKey, MultiTimeframeAggregator};
pub use data_feed::{DataFeed, FeedEvent};
pub use provider::TradeProvider;
