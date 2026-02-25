//! Heatmap chart configuration.

use crate::domain::chart::heatmap::CoalesceKind;
use serde::{Deserialize, Serialize};

// Heatmap visual configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatmapConfig {
    /// Minimum trade size in contracts (NOT dollar amount)
    /// Filters out trades smaller than this contract count
    /// Example: 5.0 = only show trades >= 5 contracts
    pub trade_size_filter: f32,
    /// Minimum orderbook order size to display (filter small orders)
    /// Value is in contracts
    pub order_size_filter: f32,
    /// Trade circle size scaling (None = fixed size, Some(100) = 100% scaling)
    pub trade_size_scale: Option<u16>,
    /// Coalescing strategy for merging similar-sized orders
    pub coalescing: Option<CoalesceKind>,
    /// Trade rendering mode (Sparse/Dense/Auto)
    #[serde(default)]
    pub rendering_mode: HeatmapRenderMode,
    /// Maximum trade markers to render (performance limit)
    #[serde(default = "default_max_trade_markers")]
    pub max_trade_markers: usize,
    /// Performance preset (auto-detected or manual)
    #[serde(default)]
    pub performance_preset: Option<String>,
}

fn default_max_trade_markers() -> usize {
    10_000
}

/// Heatmap rendering mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HeatmapRenderMode {
    /// Individual circles (best for low density)
    Sparse,
    /// Aggregated rectangles (best for high density)
    Dense,
    /// Automatically switch based on data density
    #[default]
    Auto,
}

impl std::fmt::Display for HeatmapRenderMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HeatmapRenderMode::Sparse => write!(f, "Sparse (Circles)"),
            HeatmapRenderMode::Dense => write!(f, "Dense (Rectangles)"),
            HeatmapRenderMode::Auto => write!(f, "Auto"),
        }
    }
}

impl Default for HeatmapConfig {
    fn default() -> Self {
        Self {
            trade_size_filter: 0.0,
            order_size_filter: 0.0,
            trade_size_scale: Some(100),
            coalescing: Some(CoalesceKind::None),
            rendering_mode: HeatmapRenderMode::Auto,
            max_trade_markers: 10_000,
            performance_preset: None,
        }
    }
}
