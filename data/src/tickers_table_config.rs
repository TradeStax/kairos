//! Tickers Table Configuration

use serde::{Deserialize, Serialize};

/// Tickers table state (for UI)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TickersTable {
    pub selected: Option<usize>,
    pub filter: String,
}
