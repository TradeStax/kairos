//! Feed module — historical data ingestion and candle aggregation.
//!
//! This module provides the data pipeline for backtesting:
//!
//! 1. [`TradeProvider`] fetches historical trades for a date range
//! 2. [`DataFeed`] merges multiple instrument streams into a single
//!    time-ordered sequence of [`FeedEvent`]s
//! 3. [`CandleAggregator`] converts trade ticks into OHLCV candles
//! 4. [`MultiTimeframeAggregator`] manages aggregators across multiple
//!    instrument/timeframe combinations

pub mod aggregation;
pub mod data_feed;
pub mod provider;

pub use aggregation::candle::{CandleAggregator, PartialCandle};
pub use aggregation::multi_timeframe::{AggregatorKey, MultiTimeframeAggregator};
pub use data_feed::{DataFeed, FeedEvent};
pub use provider::TradeProvider;
