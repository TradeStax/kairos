//! Pane settings and visual configuration.

use data::ChartBasis;
use serde::{Deserialize, Serialize};

#[cfg(feature = "heatmap")]
use super::HeatmapConfig;
use super::{ComparisonConfig, KlineConfig, ProfileConfig};
#[cfg(feature = "heatmap")]
use crate::screen::dashboard::ladder::config::LadderConfig;

/// Persisted configuration for a single study instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyInstanceConfig {
    pub study_id: String,
    pub enabled: bool,
    pub parameters: std::collections::HashMap<String, serde_json::Value>,
    /// Schema version when this config was saved. Compared against the study's
    /// current `metadata().config_version` on restore to detect migrations.
    #[serde(default)]
    pub config_version: u16,
}

/// Lenient deserializer for `Option<VisualConfig>` that silently maps
/// unknown or feature-gated variants to `None` instead of failing.
fn lenient_visual_config<'de, D>(deserializer: D) -> Result<Option<VisualConfig>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<VisualConfig>::deserialize(deserializer).unwrap_or(None))
}

/// Pane settings — PERSISTED to disk as part of the layout.
///
/// All fields in this struct are serialized and saved with the layout.
/// Runtime-only state (e.g. chart data, interaction state) lives in
/// `ChartState` and the GUI-layer `Content` enum instead.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    /// PERSISTED — chart basis (timeframe or tick count) selected by the user.
    pub selected_basis: Option<ChartBasis>,
    /// PERSISTED — content-type-specific visual configuration.
    #[serde(default, deserialize_with = "lenient_visual_config")]
    pub visual_config: Option<VisualConfig>,
    /// PERSISTED — saved drawings (lines, boxes, fibs) for this pane.
    #[serde(default)]
    pub drawings: Vec<crate::drawing::SerializableDrawing>,
    /// PERSISTED — saved study (indicator) configurations for this pane.
    #[serde(default)]
    pub studies: Vec<StudyInstanceConfig>,
}

/// Visual configuration for different content types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VisualConfig {
    #[cfg(feature = "heatmap")]
    Heatmap(HeatmapConfig),
    Kline(KlineConfig),
    #[cfg(feature = "heatmap")]
    Ladder(LadderConfig),
    Comparison(ComparisonConfig),
    Profile(Box<ProfileConfig>),
}

impl VisualConfig {
    #[cfg(feature = "heatmap")]
    pub fn heatmap(self) -> Option<HeatmapConfig> {
        match self {
            VisualConfig::Heatmap(cfg) => Some(cfg),
            _ => None,
        }
    }

    pub fn kline(self) -> Option<KlineConfig> {
        match self {
            VisualConfig::Kline(cfg) => Some(cfg),
            _ => None,
        }
    }

    #[cfg(feature = "heatmap")]
    pub fn ladder(self) -> Option<LadderConfig> {
        match self {
            VisualConfig::Ladder(cfg) => Some(cfg),
            _ => None,
        }
    }

    pub fn comparison(self) -> Option<ComparisonConfig> {
        match self {
            VisualConfig::Comparison(cfg) => Some(cfg),
            _ => None,
        }
    }

    pub fn profile(self) -> Option<ProfileConfig> {
        match self {
            VisualConfig::Profile(cfg) => Some(*cfg),
            _ => None,
        }
    }
}
