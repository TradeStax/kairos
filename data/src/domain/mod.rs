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
pub mod chart_ui_types;
pub mod entities;
pub mod futures;
pub mod gex;
pub mod options;
pub mod panel;
pub mod types;

// Re-export commonly used types
pub use aggregation::{AggregationError, aggregate_trades_to_candles, aggregate_trades_to_ticks};
pub use chart::{
    ChartBasis, ChartConfig, ChartData, ChartType, DataGap, DataGapKind, DataSchema, DataSegment,
    LoadingStatus, MergeResult,
};
pub use chart_ui_types::{
    Autoscale, ClusterKind, ClusterScaling, FootprintStudy, HeatmapIndicator, Indicator,
    KlineChartKind, KlineDataPoint, KlineIndicator, KlineTrades, NPoc, PointOfControl,
    UiIndicator, ViewConfig,
};
pub use entities::{Candle, DepthSnapshot, MarketData, Trade};
pub use futures::{
    ContractSpec, ContractType, FuturesTicker, FuturesTickerInfo, FuturesVenue, TickerStats,
    Timeframe,
};
pub use gex::{GammaExposure, GexLevel, GexLevelType, GexProfile};
pub use options::{
    ExerciseStyle, Greek, OptionChain, OptionContract, OptionSnapshot, OptionType,
};
pub use types::{DateRange, Price, Quantity, Side, TimeRange, Timestamp, Volume};
