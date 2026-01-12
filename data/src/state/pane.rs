//! Pane module - Re-export pane types

pub use super::pane_config::{
    ComparisonConfig, ContentKind, HeatmapConfig, KlineConfig, LadderConfig, LinkGroup, Settings,
    TimeAndSalesConfig, VisualConfig,
};

/// Axis for pane splitting
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Axis {
    Horizontal,
    Vertical,
}
