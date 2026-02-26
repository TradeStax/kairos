//! Ladder panel configuration.

use serde::{Deserialize, Serialize};

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
            trade_retention_secs: 300,
        }
    }
}
