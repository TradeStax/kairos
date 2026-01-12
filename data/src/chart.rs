//! Chart module - Re-exports for UI compatibility

pub use crate::domain::chart_ui_types::{heatmap, kline};
pub use crate::domain::{
    Autoscale, ChartBasis, ClusterKind, ClusterScaling, FootprintStudy, KlineChartKind,
    KlineDataPoint, KlineIndicator, ViewConfig,
};

pub mod indicator {
    pub use crate::domain::{HeatmapIndicator, Indicator, KlineIndicator, UiIndicator};
}
