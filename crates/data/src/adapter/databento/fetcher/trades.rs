//! Trade fetch methods for [`DatabentoAdapter`].
//!
//! Implements cache-first fetching: identifies which days are already cached,
//! fetches only the gaps from the Databento API, then loads and filters
//! results to the exact requested time range.

use std::collections::HashSet;

use databento::dbn::Schema;

use super::DatabentoAdapter;
use super::find_uncached_gaps;
use crate::adapter::databento::DatabentoError;
use crate::cache::store::{CacheProvider, CacheSchema};
use crate::domain::Trade;

impl DatabentoAdapter {
    /// Fetches trades for a date range using a cache-first strategy.
    ///
    /// Cached days are served directly; uncached gaps are fetched from the
    /// API and persisted before the combined result is filtered to the
    /// exact `range` boundaries.
    pub async fn get_trades(
        &mut self,
        symbol: &str,
        range: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>),
    ) -> Result<Vec<Trade>, DatabentoError> {
        let (start, end) = range;
        let start_date = start.date_naive();
        let end_date = end.date_naive();
        let schema = Schema::Trades;

        let mut cached_days = HashSet::new();
        let mut current = start_date;
        while current <= end_date {
            if self
                .cache
                .has_day(
                    CacheProvider::Databento,
                    symbol,
                    CacheSchema::Trades,
                    current,
                )
                .await
            {
                cached_days.insert(current);
            }
            current += chrono::Duration::days(1);
        }

        let gaps = find_uncached_gaps((start_date, end_date), &cached_days);
        for gap in &gaps {
            self.fetch_and_cache_range(symbol, schema, gap.start, gap.end)
                .await?;
        }

        let mut all_trades = Vec::new();
        let mut current = start_date;
        while current <= end_date {
            match self.load_trades_day(symbol, current).await {
                Ok(day_trades) => all_trades.extend(day_trades),
                Err(e) => {
                    log::warn!("Could not load trades {} on {}: {:?}", symbol, current, e);
                }
            }
            current += chrono::Duration::days(1);
        }

        if all_trades.is_empty() {
            return Err(DatabentoError::SymbolNotFound(format!(
                "No trade data found for {} in range {} to {}",
                symbol, start_date, end_date
            )));
        }

        let mut filtered: Vec<_> = all_trades
            .into_iter()
            .filter(|t| {
                let tms = t.time.to_millis() as i64;
                tms >= start.timestamp_millis() && tms <= end.timestamp_millis()
            })
            .collect();
        filtered.sort_by_key(|t| t.time.0);

        log::info!("get_trades: {} trades for {}", filtered.len(), symbol);
        Ok(filtered)
    }

    /// Fetches trades with a progress callback for UI feedback.
    ///
    /// The callback receives `(days_processed, total_days, current_date, was_cached)`.
    pub async fn get_trades_with_progress<F>(
        &mut self,
        symbol: &str,
        range: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>),
        progress: F,
    ) -> Result<Vec<Trade>, DatabentoError>
    where
        F: Fn(usize, usize, chrono::NaiveDate, bool),
    {
        let (start, end) = range;
        let start_date = start.date_naive();
        let end_date = end.date_naive();
        let total_days = ((end_date - start_date).num_days() + 1) as usize;
        let schema = Schema::Trades;

        let mut cached_days = HashSet::new();
        let mut current = start_date;
        while current <= end_date {
            if self
                .cache
                .has_day(
                    CacheProvider::Databento,
                    symbol,
                    CacheSchema::Trades,
                    current,
                )
                .await
            {
                cached_days.insert(current);
            }
            current += chrono::Duration::days(1);
        }

        let gaps = find_uncached_gaps((start_date, end_date), &cached_days);
        let mut days_processed = 0;

        for gap in &gaps {
            let mut gap_current = gap.start;
            while gap_current <= gap.end {
                let _ = self
                    .fetch_to_unified_cache(symbol, schema, gap_current)
                    .await;
                days_processed += 1;
                progress(days_processed, total_days, gap_current, false);
                gap_current += chrono::Duration::days(1);
            }
        }

        let mut all_trades = Vec::new();
        let mut current = start_date;
        while current <= end_date {
            let was_cached = cached_days.contains(&current);
            match self.load_trades_day(symbol, current).await {
                Ok(day_trades) => all_trades.extend(day_trades),
                Err(e) => {
                    log::warn!("Could not load {} on {}: {:?}", symbol, current, e);
                }
            }
            if was_cached {
                days_processed += 1;
                progress(days_processed, total_days, current, true);
            }
            current += chrono::Duration::days(1);
        }

        if all_trades.is_empty() {
            return Err(DatabentoError::SymbolNotFound(format!(
                "No trade data for {} between {} and {}",
                symbol, start_date, end_date
            )));
        }

        let mut filtered: Vec<_> = all_trades
            .into_iter()
            .filter(|t| {
                let tms = t.time.to_millis() as i64;
                tms >= start.timestamp_millis() && tms <= end.timestamp_millis()
            })
            .collect();
        filtered.sort_by_key(|t| t.time.0);

        log::info!(
            "get_trades_with_progress complete: {} trades for {}",
            filtered.len(),
            symbol
        );
        Ok(filtered)
    }
}
