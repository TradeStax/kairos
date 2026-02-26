//! Chart configuration and data containers.
//!
//! - [`config`] — `ChartConfig`, `ChartBasis` (time or tick), `ChartType`, `ViewConfig`, `Autoscale`
//! - [`data`] — `ChartData` (trades + derived candles + gaps), `LoadingStatus`, `DataSegment`, `MergeResult`
//! - [`kline`] — `KlineDataPoint`, `KlineTrades`, `PointOfControl`, `NPoc`
//! - `heatmap` — (feature `heatmap`) `HeatmapIndicator`, `HeatmapStudy`, `CoalesceKind`

pub mod config;
pub mod data;
#[cfg(feature = "heatmap")]
pub mod heatmap;
pub mod kline;

// Re-export commonly used types
pub use config::{Autoscale, ChartBasis, ChartConfig, ChartType, ViewConfig};
pub use data::{
    ChartData, DataGap, DataGapKind, DataSchema, DataSegment, LoadingStatus, MergeResult,
};
#[cfg(feature = "heatmap")]
pub use heatmap::HeatmapIndicator;
pub use kline::{KlineDataPoint, KlineTrades, NPoc, PointOfControl};
