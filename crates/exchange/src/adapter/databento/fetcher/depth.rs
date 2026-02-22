//! Depth (MBP-10) fetch methods for HistoricalDataManager

use super::HistoricalDataManager;
use crate::adapter::databento::DatabentoError;
use crate::adapter::databento::mapper::{chrono_to_time, convert_databento_price, determine_stype};
use databento::{
    dbn::{Mbp10Msg, Schema},
    historical::timeseries::GetRangeParams,
};

impl HistoricalDataManager {
    /// Fetch MBP-10 depth without caching (for testing/debugging only)
    ///
    /// For production use, call `fetch_mbp10_cached()` instead which provides
    /// automatic per-day caching and gap detection.
    ///
    /// MBP-10 provides 10 aggregated price levels on each side.
    pub async fn fetch_mbp10_depth(
        &mut self,
        symbol: &str,
        range: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>),
    ) -> Result<Vec<(u64, crate::types::Depth)>, DatabentoError> {
        let (start, end) = range;

        // Cost warning check
        if let Some(warning) =
            self.config.check_date_range_cost(start, end, Schema::Mbp10)
        {
            log::warn!("{}", warning);
        }

        log::debug!(
            "Fetching MBP-10 depth for {} (cost-effective L2 data)",
            symbol
        );

        let start_time = chrono_to_time(start)?;
        let end_time = chrono_to_time(end)?;

        let params = GetRangeParams::builder()
            .dataset(self.config.dataset)
            .schema(Schema::Mbp10)
            .symbols(vec![symbol])
            .stype_in(determine_stype(symbol))
            .date_time_range((start_time, end_time))
            .build();

        let mut decoder = self.client.timeseries().get_range(&params).await?;

        let mut snapshots = Vec::new();

        // Decode MBP-10 records
        while let Some(mbp) = decoder.decode_record::<Mbp10Msg>().await? {
            let ts_recv = mbp
                .ts_recv()
                .ok_or_else(|| DatabentoError::Config("missing ts_recv".to_string()))?;
            let time_ms = (ts_recv.unix_timestamp_nanos() / 1_000_000) as u64;

            let mut depth = crate::types::Depth::new(time_ms);

            // Add bid levels
            for level in &mbp.levels {
                if level.bid_px != databento::dbn::UNDEF_PRICE && level.bid_sz > 0 {
                    depth.bids.insert(
                        convert_databento_price(level.bid_px).units(),
                        level.bid_sz as f32,
                    );
                }
            }

            // Add ask levels
            for level in &mbp.levels {
                if level.ask_px != databento::dbn::UNDEF_PRICE && level.ask_sz > 0 {
                    depth.asks.insert(
                        convert_databento_price(level.ask_px).units(),
                        level.ask_sz as f32,
                    );
                }
            }

            snapshots.push((time_ms, depth));
        }

        log::info!("Fetched {} MBP-10 snapshots for {}", snapshots.len(), symbol);

        Ok(snapshots)
    }

    /// Fetch MBP-10 depth with SMART per-day caching (RECOMMENDED)
    ///
    /// This is the PRIMARY method for fetching orderbook data for heatmaps.
    /// Provides per-day granular caching, gap detection, batch fetching,
    /// progress reporting, and minimal API costs.
    pub async fn fetch_mbp10_cached(
        &mut self,
        symbol: &str,
        range: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>),
    ) -> Result<Vec<(u64, crate::types::Depth)>, DatabentoError> {
        let (start, end) = range;
        let start_date = start.date_naive();
        let end_date = end.date_naive();

        let schema = Schema::Mbp10;

        // Steps 1-3: Ensure all days are cached (fail-fast on fetch error)
        let cached_days = self
            .ensure_cached(
                "fetch_mbp10_cached",
                symbol,
                schema,
                start_date,
                end_date,
            )
            .await?;

        // Verify coverage — all requested days must be cached
        let total_days = (end_date - start_date).num_days() + 1;
        if cached_days.len() < total_days as usize {
            return Err(DatabentoError::Cache(format!(
                "Incomplete coverage for {} MBP-10: only {}/{} days cached",
                symbol,
                cached_days.len(),
                total_days
            )));
        }

        // Step 4: Load all days from cache and assemble
        let mut all_snapshots = Vec::new();
        let mut current = start_date;
        let mut day_count = 0;

        while current <= end_date {
            day_count += 1;
            match self.load_mbp10_day_from_cache(symbol, current).await {
                Ok(day_snapshots) => {
                    all_snapshots.extend(day_snapshots);
                    if day_count % 5 == 0 || day_count == total_days as usize {
                        log::debug!(
                            "Progress: {}/{} days loaded, {} MBP-10 snapshots total",
                            day_count,
                            total_days,
                            all_snapshots.len()
                        );
                    }
                }
                Err(e) => {
                    log::warn!(
                        "Could not load MBP-10 for {} on {}: {:?}",
                        symbol,
                        current,
                        e
                    );
                }
            }
            current += chrono::Duration::days(1);
        }

        if all_snapshots.is_empty() {
            return Err(DatabentoError::SymbolNotFound(format!(
                "No MBP-10 data found for {} in range {} to {}",
                symbol, start_date, end_date
            )));
        }

        // Step 5: Filter to exact datetime range
        let filtered: Vec<_> = all_snapshots
            .into_iter()
            .filter(|(time, _)| {
                let ttime = *time as i64;
                let sms = start.timestamp_millis();
                let ems = end.timestamp_millis();
                ttime >= sms && ttime <= ems
            })
            .collect();

        log::info!(
            "fetch_mbp10_cached complete: {} snapshots for {}",
            filtered.len(),
            symbol
        );

        Ok(filtered)
    }
}
