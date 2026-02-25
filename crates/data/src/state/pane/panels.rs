//! Panel configurations (Time & Sales, Ladder, Comparison).

use crate::config::panel::timeandsales::StackedBarRatio;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeAndSalesConfig {
    pub max_rows: usize,
    pub trade_size_filter: f32,
    pub trade_retention_secs: u64,
    pub show_delta: bool,
    pub stacked_bar: Option<(bool, StackedBarRatio)>, // (is_compact, ratio)
}

impl Default for TimeAndSalesConfig {
    fn default() -> Self {
        Self {
            max_rows: 100,
            trade_size_filter: 0.0,
            trade_retention_secs: 300, // 5 minutes
            show_delta: true,
            stacked_bar: Some((false, StackedBarRatio::Volume)), // Full mode, Volume
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LadderConfig {
    pub levels: usize,
    pub show_spread: bool,
    pub show_chase_tracker: bool,
    pub trade_retention_secs: u64,
}

impl Default for LadderConfig {
    fn default() -> Self {
        Self {
            levels: 20,
            show_spread: true,
            show_chase_tracker: true,
            trade_retention_secs: 300, // 5 minutes
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComparisonConfig {
    pub normalize: Option<bool>,
    /// Map of ticker symbol strings to colors (e.g., "ESH5" -> Rgba)
    #[serde(default)]
    pub colors: Vec<(String, crate::config::color::Rgba)>,
    /// Map of ticker symbol strings to custom names
    #[serde(default)]
    pub names: Vec<(String, String)>,
}
