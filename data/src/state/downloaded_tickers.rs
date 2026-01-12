//! Downloaded Tickers Registry
//!
//! Tracks which tickers have been explicitly downloaded via Data Management
//! and remembers the date range for each ticker.

use crate::domain::{DateRange, FuturesTicker};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Registry of explicitly downloaded tickers with their date ranges
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DownloadedTickersRegistry {
    /// Maps ticker symbol (ES.c.0) to downloaded date range
    tickers: HashMap<String, DateRange>,
}

impl DownloadedTickersRegistry {
    pub fn new() -> Self {
        Self {
            tickers: HashMap::new(),
        }
    }

    /// Register a ticker with its downloaded date range
    ///
    /// This is called when user explicitly downloads data via Data Management modal.
    /// The ticker will then appear in the ticker list and use this date range for charts.
    pub fn register(&mut self, ticker: FuturesTicker, date_range: DateRange) {
        let ticker_str = ticker.to_string();
        log::info!("REGISTRY: Registering {} with range {} to {}", ticker_str, date_range.start, date_range.end);
        self.tickers.insert(ticker_str, date_range);
    }

    /// Get the downloaded date range for a ticker
    ///
    /// Returns the date range that was explicitly downloaded for this ticker.
    /// Used when loading charts to ensure we use the correct range.
    pub fn get_range(&self, ticker: &FuturesTicker) -> Option<DateRange> {
        let ticker_str = ticker.to_string();
        log::info!("REGISTRY: get_range() looking for '{}'", ticker_str);
        log::info!("REGISTRY: Current registry has {} entries:", self.tickers.len());
        for (key, range) in &self.tickers {
            log::info!("REGISTRY:   '{}' → {} to {}", key, range.start, range.end);
        }

        let range = self.tickers.get(&ticker_str).copied();
        if let Some(ref r) = range {
            log::info!("REGISTRY: FOUND range for {}: {} to {}", ticker_str, r.start, r.end);
        } else {
            log::warn!("REGISTRY: NOT FOUND for '{}'", ticker_str);
        }
        range
    }

    /// Get list of all registered ticker symbols
    ///
    /// Used to populate the ticker list - only registered tickers are shown.
    pub fn list_tickers(&self) -> Vec<String> {
        self.tickers.keys().cloned().collect()
    }

    /// Check if a ticker has been registered
    pub fn has_ticker(&self, ticker: &FuturesTicker) -> bool {
        self.tickers.contains_key(&ticker.to_string())
    }

    /// Remove a ticker from the registry
    pub fn unregister(&mut self, ticker: &FuturesTicker) {
        let ticker_str = ticker.to_string();
        if self.tickers.remove(&ticker_str).is_some() {
            log::info!("REGISTRY: Unregistered {}", ticker_str);
        }
    }

    /// Get number of registered tickers
    pub fn count(&self) -> usize {
        self.tickers.len()
    }
}
