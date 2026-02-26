//! Depth (MBP-10) fetch methods

use super::DatabentoAdapter;
use super::find_uncached_gaps;
use crate::adapter::databento::DatabentoError;
use crate::cache::store::{CacheProvider, CacheSchema};
use crate::domain::Depth;
use databento::dbn::Schema;
use std::collections::HashSet;

impl DatabentoAdapter {
    /// Fetch MBP-10 depth for a date range — cache-first, gap-fill from API
    pub async fn get_depth(
        &mut self,
        symbol: &str,
        range: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>),
    ) -> Result<Vec<Depth>, DatabentoError> {
        let (start, end) = range;
        let start_date = start.date_naive();
        let end_date = end.date_naive();
        let schema = Schema::Mbp10;

        // Identify cached days
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

        // Fetch gaps
        let gaps = find_uncached_gaps((start_date, end_date), &cached_days);
        for gap in &gaps {
            self.fetch_and_cache_range(symbol, schema, gap.start, gap.end)
                .await?;
        }

        // Load all days from cache
        let mut all_snapshots = Vec::new();
        let mut current = start_date;
        while current <= end_date {
            match self.load_depth_day(symbol, current).await {
                Ok(day_snaps) => all_snapshots.extend(day_snaps),
                Err(e) => log::warn!("Could not load depth {} on {}: {:?}", symbol, current, e),
            }
            current += chrono::Duration::days(1);
        }

        if all_snapshots.is_empty() {
            return Err(DatabentoError::SymbolNotFound(format!(
                "No depth data found for {} in range {} to {}",
                symbol, start_date, end_date
            )));
        }

        // Filter to exact range
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
