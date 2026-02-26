//! Downloaded Tickers Registry

use crate::domain::core::types::DateRange;
use crate::domain::instrument::futures::FuturesTicker;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Registry of explicitly downloaded tickers with their date ranges
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DownloadedTickersRegistry {
    tickers: HashMap<String, DateRange>,
}

impl DownloadedTickersRegistry {
    pub fn new() -> Self {
        Self {
            tickers: HashMap::new(),
        }
    }

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

    pub fn list_tickers(&self) -> Vec<String> {
        self.tickers.keys().cloned().collect()
    }

    pub fn get_range_by_ticker_str(&self, ticker_str: &str) -> Option<DateRange> {
        self.tickers.get(ticker_str).copied()
    }

    pub fn has_ticker(&self, ticker: &FuturesTicker) -> bool {
        self.tickers.contains_key(&ticker.to_string())
    }

    pub fn unregister(&mut self, ticker: &FuturesTicker) {
        let ticker_str = ticker.to_string();
        if self.tickers.remove(&ticker_str).is_some() {
            log::debug!("Registry: unregistered {}", ticker_str);
        }
    }

    pub fn count(&self) -> usize {
        self.tickers.len()
    }
}
