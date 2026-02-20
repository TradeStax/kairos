//! Download subsystem - shared types and re-exports for data management
//! and historical download modals.

pub mod data_management;
pub mod historical;
pub mod views;

// Convenience re-exports
pub use data_management::{DataManagementMessage, DataManagementPanel};
pub use historical::{HistoricalDownloadMessage, HistoricalDownloadModal};

/// Futures products for ticker dropdown (shared by data_management
/// and historical_download)
pub const FUTURES_PRODUCTS: &[(&str, &str)] = &[
    ("ES.c.0", "E-mini S&P 500"),
    ("NQ.c.0", "E-mini Nasdaq-100"),
    ("YM.c.0", "E-mini Dow"),
    ("RTY.c.0", "E-mini Russell 2000"),
    ("CL.c.0", "Crude Oil"),
    ("GC.c.0", "Gold"),
    ("SI.c.0", "Silver"),
    ("ZN.c.0", "10-Year T-Note"),
    ("ZB.c.0", "30-Year T-Bond"),
    ("ZF.c.0", "5-Year T-Note"),
    ("NG.c.0", "Natural Gas"),
    ("HG.c.0", "Copper"),
];

/// Schemas with display names and cost rating
pub const SCHEMAS: &[(exchange::DatabentoSchema, &str, u8)] = &[
    (exchange::DatabentoSchema::Trades, "Trades", 2),
    (exchange::DatabentoSchema::Mbp10, "MBP-10 (10 Levels)", 3),
    (exchange::DatabentoSchema::Mbp1, "MBP-1 (Top of Book)", 2),
    (exchange::DatabentoSchema::Ohlcv1M, "OHLCV-1M", 1),
    (exchange::DatabentoSchema::Tbbo, "TBBO (Top BBO)", 2),
    (exchange::DatabentoSchema::Mbo, "MBO (VERY EXPENSIVE)", 10),
];

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
    Downloading {
        current_day: usize,
        total_days: usize,
    },
    Complete {
        days_downloaded: usize,
    },
    Error(String),
}

/// Helper for building ticker/schema from index selections
pub struct DownloadConfig;

impl DownloadConfig {
    pub fn ticker_from_idx(idx: usize) -> data::FuturesTicker {
        let (sym, _) = FUTURES_PRODUCTS[idx];
        data::FuturesTicker::new(sym, exchange::FuturesVenue::CMEGlobex)
    }

    pub fn schema_from_idx(idx: usize) -> exchange::DatabentoSchema {
        SCHEMAS[idx].0
    }

    pub fn ticker_display(idx: usize) -> String {
        let (sym, name) = FUTURES_PRODUCTS[idx];
        format!("{} - {}", sym, name)
    }

    pub fn schema_display(idx: usize) -> String {
        let (_, name, rating) = SCHEMAS[idx];
        format!("{} (Cost: {}/10)", name, rating)
    }

    pub fn ticker_options() -> Vec<String> {
        FUTURES_PRODUCTS
            .iter()
            .map(|(sym, name)| format!("{} - {}", sym, name))
            .collect()
    }

    pub fn schema_options() -> Vec<String> {
        SCHEMAS
            .iter()
            .map(|(_, name, rating)| format!("{} (Cost: {}/10)", name, rating))
            .collect()
    }

    pub fn find_ticker_idx(selected: &str) -> usize {
        FUTURES_PRODUCTS
            .iter()
            .position(|(sym, n)| format!("{} - {}", sym, n) == selected)
            .unwrap_or(0)
    }

    pub fn find_schema_idx(selected: &str) -> usize {
        SCHEMAS
            .iter()
            .position(|(_, n, r)| format!("{} (Cost: {}/10)", n, r) == selected)
            .unwrap_or(0)
    }
}
