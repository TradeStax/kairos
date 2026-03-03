//! Domain model — pure types with no I/O and no async.
//!
//! - [`core`] — `Price` (fixed-point i64), `Timestamp`, `Side`, `Volume`, `DateRange`, color, errors
//! - [`market`] — `Trade`, `Candle`, `Depth`, `MarketData`
//! - [`instrument`] — `FuturesTicker`, `FuturesTickerInfo`, `Timeframe`, `ContractType`
//! - [`chart`] — `ChartConfig`, `ChartData`, `ChartBasis`, `LoadingStatus`
//! - [`data`] — `DataIndex`, `DownloadedTickersRegistry`
//! - [`replay`] — `ReplayState`, `PlaybackStatus`, `SpeedPreset`, `ReplayData`

pub mod chart;
pub mod core;
pub mod data;
pub mod instrument;
pub mod market;
pub mod replay;

// Re-export submodule paths so consumers can use short paths
// e.g. `crate::domain::types::FeedId` instead of `crate::domain::core::types::FeedId`
pub use self::core::error;
pub use self::core::price;
pub use self::core::types;
pub use self::data::index;
pub use self::instrument::futures;
pub use self::market::entities;

// Re-export commonly used types for ergonomic access
#[cfg(feature = "heatmap")]
pub use chart::HeatmapIndicator;
pub use chart::{
    Autoscale, ChartBasis, ChartConfig, ChartData, ChartType, DataGap, DataGapKind, DataSchema,
    DataSegment, KlineDataPoint, KlineTrades, LoadingStatus, MergeResult, NPoc, PointOfControl,
    ViewConfig,
};
pub use core::{
    color::{Rgba, SerializableColor, hex_to_rgba, rgba_to_hex},
    error::{AppError, ErrorSeverity},
    price::{ContractSize, MinQtySize, MinTicksize, Power10, PriceExt, PriceStep, ms_to_datetime},
    types::{DateRange, FeedId, Price, Quantity, Side, TimeRange, Timestamp, Volume},
};
pub use data::{DataIndex, DataKey, DownloadedTickersRegistry, FeedContribution};
pub use instrument::{
    ContractSpec, ContractType, FuturesTicker, FuturesTickerInfo, FuturesVenue, TickerStats,
    Timeframe,
};
pub use market::{Candle, Depth, MarketData, Trade};
pub use replay::{PlaybackStatus, ReplayData, ReplayState, SpeedPreset};
