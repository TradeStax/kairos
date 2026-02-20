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
pub mod chart;
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
    Autoscale, CandlePosition, ChartBasis, ChartConfig, ChartData, ChartType, ClusterScaling,
    DataGap, DataGapKind, DataSchema, DataSegment, FootprintMode, FootprintStudyConfig,
    FootprintType, HeatmapIndicator, KlineDataPoint, KlineTrades,
    LoadingStatus, MergeResult, NPoc, PointOfControl, ViewConfig,
};
pub use entities::{Candle, DepthSnapshot, MarketData, Trade};
pub use futures::{
    ContractSpec, ContractType, FuturesTicker, FuturesTickerInfo, FuturesVenue, TickerStats,
    Timeframe,
};
pub use gex::{GammaExposure, GexLevel, GexLevelType, GexProfile};
pub use options::{ExerciseStyle, Greek, OptionChain, OptionContract, OptionSnapshot, OptionType};
pub use types::{DateRange, Price, Quantity, Side, TimeRange, Timestamp, Volume};
