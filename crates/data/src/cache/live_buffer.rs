//! LiveDayBuffer — accumulates Rithmic streaming data per trading day
//!
//! Trades and depth snapshots accumulate in memory. On day-roll or
//! `flush()` (disconnect / shutdown) the buffer writes to unified cache.

use crate::domain::entities::{Depth, Trade};
use chrono::NaiveDate;
use std::collections::HashMap;

/// In-memory buffer for a single trading day
#[derive(Debug, Default)]
struct DayBuffer {
    trades: Vec<Trade>,
    depth: Vec<Depth>,
}

/// Accumulates live streaming data per trading day.
///
/// Call `push_trade` / `push_depth` for each incoming event.
/// Call `flush_day(date)` when the trading day rolls over.
/// Call `flush_all()` on disconnect / shutdown.
pub struct LiveDayBuffer {
    symbol: String,
    days: HashMap<NaiveDate, DayBuffer>,
}

impl LiveDayBuffer {
    pub fn new(symbol: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            days: HashMap::new(),
        }
    }

    pub fn push_trade(&mut self, date: NaiveDate, trade: Trade) {
        self.days.entry(date).or_default().trades.push(trade);
    }

    pub fn push_depth(&mut self, date: NaiveDate, depth: Depth) {
        self.days.entry(date).or_default().depth.push(depth);
    }

    /// Drain trades for a specific day (returns ownership for writing to cache)
    pub fn drain_trades(&mut self, date: NaiveDate) -> Vec<Trade> {
        if let Some(buf) = self.days.get_mut(&date) {
            std::mem::take(&mut buf.trades)
        } else {
            vec![]
        }
    }

    /// Drain depth snapshots for a specific day
    pub fn drain_depth(&mut self, date: NaiveDate) -> Vec<Depth> {
        if let Some(buf) = self.days.get_mut(&date) {
            std::mem::take(&mut buf.depth)
        } else {
            vec![]
        }
    }

    /// Remove a day's buffer entirely
    pub fn remove_day(&mut self, date: NaiveDate) {
        self.days.remove(&date);
    }

    /// All buffered dates
    pub fn buffered_dates(&self) -> Vec<NaiveDate> {
        let mut dates: Vec<_> = self.days.keys().copied().collect();
        dates.sort();
        dates
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// Total trades buffered across all days
    pub fn total_trade_count(&self) -> usize {
        self.days.values().map(|b| b.trades.len()).sum()
    }

    /// Total depth snapshots buffered across all days
    pub fn total_depth_count(&self) -> usize {
        self.days.values().map(|b| b.depth.len()).sum()
    }
}
