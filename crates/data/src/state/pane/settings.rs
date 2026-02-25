//! Pane settings and visual configuration.

use crate::domain::ChartBasis;
use serde::{Deserialize, Serialize};

use super::{
    ComparisonConfig, HeatmapConfig, KlineConfig, LadderConfig, ProfileConfig,
    TimeAndSalesConfig,
};

/// Link group for synchronized panes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LinkGroup(pub u8);

impl LinkGroup {
    pub const ALL: [LinkGroup; 9] = [
        LinkGroup(1),
        LinkGroup(2),
        LinkGroup(3),
        LinkGroup(4),
        LinkGroup(5),
        LinkGroup(6),
        LinkGroup(7),
        LinkGroup(8),
        LinkGroup(9),
    ];
}

impl std::fmt::Display for LinkGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Persisted configuration for a single study instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyInstanceConfig {
    pub study_id: String,
    pub enabled: bool,
    pub parameters: std::collections::HashMap<String, serde_json::Value>,
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
    Heatmap(HeatmapConfig),
    Kline(KlineConfig),
    TimeAndSales(TimeAndSalesConfig),
    Ladder(LadderConfig),
    Comparison(ComparisonConfig),
    Profile(ProfileConfig),
}

impl VisualConfig {
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

    pub fn time_and_sales(self) -> Option<TimeAndSalesConfig> {
        match self {
            VisualConfig::TimeAndSales(cfg) => Some(cfg),
            _ => None,
        }
    }

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
            VisualConfig::Profile(cfg) => Some(cfg),
            _ => None,
        }
    }
}
