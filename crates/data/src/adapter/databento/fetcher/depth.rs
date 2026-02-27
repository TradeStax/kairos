//! Depth (MBP-10) fetch methods for [`DatabentoAdapter`].
//!
//! Follows the same cache-first strategy as trade fetching: cached days are
//! served directly, gaps are fetched from the API, and results are filtered
//! to the exact requested time range.

use std::collections::HashSet;

use databento::dbn::Schema;

use super::DatabentoAdapter;
use super::find_uncached_gaps;
use crate::adapter::databento::DatabentoError;
use crate::cache::store::{CacheProvider, CacheSchema};
use crate::domain::Depth;

impl DatabentoAdapter {
    /// Fetches MBP-10 depth snapshots for a date range using a cache-first strategy.
    ///
    /// Cached days are served directly; uncached gaps are fetched from the
    /// API and persisted before the combined result is filtered to the
    /// exact `range` boundaries.
    pub async fn get_depth(
        &mut self,
        symbol: &str,
        range: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>),
    ) -> Result<Vec<Depth>, DatabentoError> {
        let (start, end) = range;
        let start_date = start.date_naive();
        let end_date = end.date_naive();
        let schema = Schema::Mbp10;

        let mut cached_days = HashSet::new();
        let mut current = start_date;
        while current <= end_date {
            if self
                .cache
                .has_day(
                    CacheProvider::Databento,
                    symbol,
                    CacheSchema::Depth,
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

        let mut all_snapshots = Vec::new();
        let mut current = start_date;
        while current <= end_date {
            match self.load_depth_day(symbol, current).await {
                Ok(day_snaps) => all_snapshots.extend(day_snaps),
                Err(e) => {
                    log::warn!("Could not load depth {} on {}: {:?}", symbol, current, e);
                }
            }
            current += chrono::Duration::days(1);
        }

        if all_snapshots.is_empty() {
            return Err(DatabentoError::SymbolNotFound(format!(
                "No depth data found for {} in range {} to {}",
                symbol, start_date, end_date
            )));
        }

        let filtered: Vec<_> = all_snapshots
            .into_iter()
            .filter(|d| {
                let tms = d.time as i64;
                tms >= start.timestamp_millis() && tms <= end.timestamp_millis()
            })
            .collect();

        log::info!("get_depth: {} snapshots for {}", filtered.len(), symbol);
        Ok(filtered)
    }
}
