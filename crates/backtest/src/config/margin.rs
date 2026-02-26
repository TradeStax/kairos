use serde::{Deserialize, Serialize};

/// Margin enforcement configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MarginConfig {
    /// Whether to enforce margin checks
    /// (reject orders that exceed buying power).
    #[serde(default)]
    pub enforce: bool,
    /// Initial margin per contract override
    /// (if None, uses InstrumentSpec).
    pub initial_margin_override: Option<f64>,
    /// Maintenance margin per contract override.
    pub maintenance_margin_override: Option<f64>,
}
