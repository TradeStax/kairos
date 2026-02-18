//! Download subsystem - shared types and re-exports for data management
//! and historical download modals.

pub mod data_management;
pub mod historical;
pub mod views;

// Convenience re-exports
pub use data_management::{DataManagementPanel, DataManagementMessage};
pub use historical::{HistoricalDownloadModal, HistoricalDownloadMessage};

/// Cache coverage status for a date range
#[derive(Debug, Clone, PartialEq)]
pub struct CacheStatus {
    pub total_days: usize,
    pub cached_days: usize,
    pub uncached_days: usize,
    /// Optional description of gaps (used by data_management panel)
    pub gaps_description: Option<String>,
}

/// Download progress state shared by both download modals
#[derive(Debug, Clone, PartialEq)]
pub enum DownloadProgress {
    Idle,
    CheckingCost,
    Downloading { current_day: usize, total_days: usize },
    Complete { days_downloaded: usize },
    Error(String),
}

/// Helper for building ticker/schema from index selections
pub struct DownloadConfig;

impl DownloadConfig {
    pub fn ticker_from_idx(idx: usize) -> data::FuturesTicker {
        let (sym, _) = super::FUTURES_PRODUCTS[idx];
        data::FuturesTicker::new(sym, exchange::FuturesVenue::CMEGlobex)
    }

    pub fn schema_from_idx(idx: usize) -> exchange::DatabentoSchema {
        super::SCHEMAS[idx].0
    }

    pub fn ticker_display(idx: usize) -> String {
        let (sym, name) = super::FUTURES_PRODUCTS[idx];
        format!("{} - {}", sym, name)
    }

    pub fn schema_display(idx: usize) -> String {
        let (_, name, rating) = super::SCHEMAS[idx];
        format!("{} (Cost: {}/10)", name, rating)
    }

    pub fn ticker_options() -> Vec<String> {
        super::FUTURES_PRODUCTS
            .iter()
            .map(|(sym, name)| format!("{} - {}", sym, name))
            .collect()
    }

    pub fn schema_options() -> Vec<String> {
        super::SCHEMAS
            .iter()
            .map(|(_, name, rating)| format!("{} (Cost: {}/10)", name, rating))
            .collect()
    }

    pub fn find_ticker_idx(selected: &str) -> usize {
        super::FUTURES_PRODUCTS
            .iter()
            .position(|(sym, n)| format!("{} - {}", sym, n) == selected)
            .unwrap_or(0)
    }

    pub fn find_schema_idx(selected: &str) -> usize {
        super::SCHEMAS
            .iter()
            .position(|(_, n, r)| format!("{} (Cost: {}/10)", n, r) == selected)
            .unwrap_or(0)
    }
}
