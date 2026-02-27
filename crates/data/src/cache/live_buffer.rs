//! In-memory buffer for accumulating live streaming data per trading day.
//!
//! Trades and depth snapshots accumulate in memory keyed by date. On day-roll
//! or disconnect/shutdown the caller drains the buffer and writes to the
//! unified cache via [`super::store::CacheStore`].

use crate::domain::entities::{Depth, Trade};
use chrono::NaiveDate;
use std::collections::HashMap;

/// In-memory buffer for a single trading day.
#[derive(Debug, Default)]
struct DayBuffer {
    trades: Vec<Trade>,
    depth: Vec<Depth>,
}

/// Accumulates live streaming data per trading day for a single symbol.
///
/// Call [`push_trade`](Self::push_trade) / [`push_depth`](Self::push_depth)
/// for each incoming event. Use [`drain_trades`](Self::drain_trades) /
/// [`drain_depth`](Self::drain_depth) when flushing to cache, and
/// [`remove_day`](Self::remove_day) to free memory after a successful flush.
pub struct LiveDayBuffer {
    symbol: String,
    days: HashMap<NaiveDate, DayBuffer>,
}

impl LiveDayBuffer {
    /// Creates a new buffer for the given symbol
    #[must_use]
    pub fn new(symbol: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            days: HashMap::new(),
        }
    }

    /// Appends a trade to the buffer for the given date
    pub fn push_trade(&mut self, date: NaiveDate, trade: Trade) {
        self.days.entry(date).or_default().trades.push(trade);
    }

    /// Appends a depth snapshot to the buffer for the given date
    pub fn push_depth(&mut self, date: NaiveDate, depth: Depth) {
        self.days.entry(date).or_default().depth.push(depth);
    }

    /// Drains and returns all trades for a specific day
    pub fn drain_trades(&mut self, date: NaiveDate) -> Vec<Trade> {
        if let Some(buf) = self.days.get_mut(&date) {
            std::mem::take(&mut buf.trades)
        } else {
            vec![]
        }
    }

    /// Drains and returns all depth snapshots for a specific day
    pub fn drain_depth(&mut self, date: NaiveDate) -> Vec<Depth> {
        if let Some(buf) = self.days.get_mut(&date) {
            std::mem::take(&mut buf.depth)
        } else {
            vec![]
        }
    }

    /// Removes a day's buffer entirely, freeing memory
    pub fn remove_day(&mut self, date: NaiveDate) {
        self.days.remove(&date);
    }

    /// Returns all buffered dates in sorted order
    #[must_use]
    pub fn buffered_dates(&self) -> Vec<NaiveDate> {
        let mut dates: Vec<_> = self.days.keys().copied().collect();
        dates.sort();
        dates
    }

    /// Returns the symbol this buffer is tracking
    #[must_use]
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// Returns the total trade count across all buffered days
    #[must_use]
    pub fn total_trade_count(&self) -> usize {
        self.days.values().map(|b| b.trades.len()).sum()
    }

    /// Returns the total depth snapshot count across all buffered days
    #[must_use]
    pub fn total_depth_count(&self) -> usize {
        self.days.values().map(|b| b.depth.len()).sum()
    }
}
