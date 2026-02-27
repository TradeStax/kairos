//! Downloaded tickers registry — persisted record of explicitly downloaded
//! ticker date ranges used by the data management UI.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::domain::core::types::DateRange;
use crate::domain::instrument::futures::FuturesTicker;

/// Registry of explicitly downloaded tickers with their date ranges.
///
/// Persisted to disk so the download manager can show which tickers
/// have been fetched and what date ranges are available offline.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DownloadedTickersRegistry {
    tickers: HashMap<String, DateRange>,
}

impl DownloadedTickersRegistry {
    /// Create an empty registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            tickers: HashMap::new(),
        }
    }

    /// Register a downloaded ticker with its date range
    pub fn register(&mut self, ticker: FuturesTicker, date_range: DateRange) {
        let ticker_str = ticker.to_string();
        log::debug!(
            "Registry: registered {} ({} to {})",
            ticker_str,
            date_range.start,
            date_range.end
        );
        self.tickers.insert(ticker_str, date_range);
    }

    /// Look up the downloaded date range for a ticker
    #[must_use]
    pub fn get_range(&self, ticker: &FuturesTicker) -> Option<DateRange> {
        let ticker_str = ticker.to_string();
        let range = self.tickers.get(&ticker_str).copied();
        log::trace!(
            "Registry: get_range('{}') → {}",
            ticker_str,
            if range.is_some() {
                "found"
            } else {
                "not found"
            }
        );
        range
    }

    /// Return all registered ticker symbols
    #[must_use]
    pub fn list_tickers(&self) -> Vec<String> {
        self.tickers.keys().cloned().collect()
    }

    /// Look up the downloaded date range by raw ticker string
    #[must_use]
    pub fn get_range_by_ticker_str(&self, ticker_str: &str) -> Option<DateRange> {
        self.tickers.get(ticker_str).copied()
    }

    /// Return `true` if the ticker has been downloaded
    #[must_use]
    pub fn has_ticker(&self, ticker: &FuturesTicker) -> bool {
        self.tickers.contains_key(&ticker.to_string())
    }

    /// Remove a ticker from the registry
    pub fn unregister(&mut self, ticker: &FuturesTicker) {
        let ticker_str = ticker.to_string();
        if self.tickers.remove(&ticker_str).is_some() {
            log::debug!("Registry: unregistered {}", ticker_str);
        }
    }

    /// Return the number of registered tickers
    #[must_use]
    pub fn count(&self) -> usize {
        self.tickers.len()
    }
}
