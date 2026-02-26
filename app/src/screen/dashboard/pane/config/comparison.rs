//! Comparison chart configuration.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComparisonConfig {
    pub normalize: Option<bool>,
    /// Map of ticker symbol strings to colors (e.g., "ESH5" -> Rgba)
    #[serde(default)]
    pub colors: Vec<(String, data::Rgba)>,
    /// Map of ticker symbol strings to custom names
    #[serde(default)]
    pub names: Vec<(String, String)>,
}
