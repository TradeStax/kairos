//! Data Feed Module
//!
//! Provides the data feed connection model where users configure persistent
//! data feed connections (Databento for historical, Rithmic for realtime).

pub mod manager;
pub mod types;

pub use manager::{DataFeedManager, ResolvedFeed};
pub use types::{
    DataFeed, DatabentoFeedConfig, FeedCapability, FeedConfig, FeedId, FeedKind, FeedProvider,
    FeedStatus, HistoricalDatasetInfo, RithmicEnvironment, RithmicFeedConfig, RithmicServer,
};
