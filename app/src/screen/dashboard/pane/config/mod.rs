//! Pane Configuration Types
//!
//! UI state types for pane configuration and settings.

pub mod candlestick;
pub mod comparison;
pub mod content_kind;
#[cfg(feature = "heatmap")]
pub mod heatmap;
pub mod link;
pub mod profile;
pub mod visual;

pub use candlestick::{CandleColorField, CandleStyle, KlineConfig};
pub use comparison::ComparisonConfig;
pub use content_kind::ContentKind;
#[cfg(feature = "heatmap")]
pub use heatmap::{HeatmapConfig, HeatmapRenderMode};
pub use link::LinkGroup;
pub use profile::{
    ProfileConfig, ProfileDisplayType, ProfileExtendDirection, ProfileLineStyle,
    ProfileNodeDetectionMethod, ProfileSplitUnit,
};
pub use visual::{Settings, StudyInstanceConfig, VisualConfig};

// Re-export ladder config types for convenience (used by VisualConfig)
#[cfg(feature = "heatmap")]
pub use crate::screen::dashboard::ladder::config::LadderConfig;
