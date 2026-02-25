//! Domain Model - Core Business Logic
//!
//! This module contains the pure domain model with:
//! - Value objects (Price, Volume, Timestamp, etc.)
//! - Entities (Trade, Candle, DepthSnapshot)
//! - Aggregation logic (single source of truth)
//! - Chart domain models
//!
//! NO infrastructure dependencies - pure business logic only.

pub mod aggregation;
pub mod assistant;
pub mod chart;
pub mod data_index;
pub mod entities;
pub mod error;
pub mod futures;
pub mod gex;
pub mod options;
pub mod panel;
pub mod types;

// Re-export commonly used types
pub use aggregation::{AggregationError, aggregate_trades_to_candles, aggregate_trades_to_ticks};
pub use chart::{
    Autoscale, ChartBasis, ChartConfig, ChartData, ChartType, DataGap, DataGapKind, DataSchema,
    DataSegment, HeatmapIndicator, KlineDataPoint, KlineTrades, LoadingStatus, MergeResult, NPoc,
    PointOfControl, ViewConfig,
};
pub use entities::{Candle, DepthSnapshot, MarketData, Trade};
pub use futures::{
    ContractSpec, ContractType, FuturesTicker, FuturesTickerInfo, FuturesVenue, TickerStats,
    Timeframe,
};
pub use gex::{GammaExposure, GexLevel, GexLevelType, GexProfile};
pub use options::{ExerciseStyle, Greek, OptionChain, OptionContract, OptionSnapshot, OptionType};
pub use data_index::{DataIndex, DataKey, FeedContribution};
pub use types::{DateRange, Price, Quantity, Side, TimeRange, Timestamp, Volume};
