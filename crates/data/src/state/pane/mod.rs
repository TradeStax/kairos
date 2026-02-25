//! Pane Configuration Types
//!
//! UI state types for pane configuration and settings.

mod content_kind;
mod settings;
mod heatmap;
mod candlestick;
mod panels;
mod profile;

pub use content_kind::ContentKind;
pub use settings::{LinkGroup, StudyInstanceConfig, Settings, VisualConfig};
pub use heatmap::{HeatmapConfig, HeatmapRenderMode};
pub use candlestick::{CandleColorField, CandleStyle, KlineConfig};
pub use panels::{TimeAndSalesConfig, LadderConfig, ComparisonConfig};
pub use profile::{
    ProfileLineStyle, ProfileExtendDirection, ProfileNodeDetectionMethod,
    ProfileDisplayType, ProfileSplitUnit, ProfileConfig,
};
