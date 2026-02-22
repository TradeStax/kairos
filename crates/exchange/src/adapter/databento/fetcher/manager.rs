//! Shared fetch orchestration methods for HistoricalDataManager
//!
//! Contains `ensure_cached` (shared by ohlcv, trades, depth modules) and
//! `fetch_open_interest`. Schema-specific fetch methods live in their own modules:
//! - `ohlcv.rs` — OHLCV fetch methods
//! - `trades.rs` — trade fetch methods
//! - `depth.rs` — MBP-10 depth fetch methods

use super::HistoricalDataManager;
use super::gaps::find_uncached_gaps;
use crate::adapter::databento::DatabentoError;
use crate::adapter::databento::mapper::{chrono_to_time, determine_stype};
use databento::dbn::Schema;
use std::collections::HashSet;

impl HistoricalDataManager {
    /// Identify cached days, find gaps, and fetch missing days for a schema.
    ///
    /// Returns the set of successfully cached days for step 4 (loading from cache).
    /// Returns `Err` immediately if any gap fetch fails (fail-fast).
    pub(super) async fn ensure_cached(
        &mut self,
        method_name: &str,
        symbol: &str,
        schema: Schema,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> Result<HashSet<chrono::NaiveDate>, DatabentoError> {
        let total_days = (end_date - start_date).num_days() + 1;
        log::debug!(
            "{}: {} from {} to {} ({:?}) - {} days total",
            method_name,
            symbol,
            start_date,
            end_date,
            schema,
            total_days
        );

        // Step 1: Identify cached days
        let mut cached_days = HashSet::new();
        let mut current = start_date;
        while current <= end_date {
            if self.cache.has_cached(symbol, schema, current).await {
                cached_days.insert(current);
            }
            current += chrono::Duration::days(1);
        }

        log::debug!("Found {}/{} days cached", cached_days.len(), total_days);

        // Step 2: Find gaps
        let gaps = find_uncached_gaps((start_date, end_date), &cached_days);

        if gaps.is_empty() {
            return Ok(cached_days);
        }

        log::debug!("Identified {} gap(s) to fetch", gaps.len());

        // Step 3: Fetch each gap — fail-fast on any error
        let num_gaps = gaps.len();
        for (gap_idx, gap) in gaps.iter().enumerate() {
            log::debug!(
                "Fetching gap {}/{}: {} to {} ({} days)",
                gap_idx + 1,
                num_gaps,
                gap.start,
                gap.end,
                (gap.end - gap.start).num_days() + 1
            );

            match self
                .fetch_and_cache_range(symbol, schema, gap.start, gap.end)
                .await
            {
                Ok(days_saved) => {
                    log::debug!(
                        "Gap {}/{} complete: cached {} days",
                        gap_idx + 1,
                        num_gaps,
                        days_saved
                    );
                    // Update cached_days with newly fetched days
                    let mut d = gap.start;
                    while d <= gap.end {
                        if self.cache.has_cached(symbol, schema, d).await {
                            cached_days.insert(d);
                        }
                        d += chrono::Duration::days(1);
                    }
                }
                Err(e) => {
                    log::error!(
                        "FAILED: Gap {}/{}: {} to {} - {:?}",
                        gap_idx + 1,
                        num_gaps,
                        gap.start,
                        gap.end,
                        e
                    );
                    return Err(e);
                }
            }
        }

        Ok(cached_days)
    }

    /// Fetch open interest data using Statistics schema
    ///
    /// Open Interest is reported daily, typically once per trading day.
    /// This uses Databento's Statistics schema to get open interest values.
    pub async fn fetch_open_interest(
        &mut self,
        symbol: &str,
        range: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>),
    ) -> Result<Vec<crate::types::OpenInterest>, DatabentoError> {
        let (start, end) = range;

        log::debug!("fetch_open_interest: {} from {} to {}", symbol, start, end);

        // Check cost estimate for date range
        if let Some(warning) = self
            .config
            .check_date_range_cost(start, end, Schema::Statistics)
        {
            log::warn!("{}", warning);
        }

        // Convert chrono DateTime to time OffsetDateTime
        let start_time = chrono_to_time(start)?;
        let end_time = chrono_to_time(end)?;

        // Build request parameters for Statistics schema
        let params = databento::historical::timeseries::GetRangeParams::builder()
            .dataset(self.config.dataset)
            .schema(Schema::Statistics)
            .symbols(vec![symbol])
            .stype_in(determine_stype(symbol))
            .date_time_range((start_time, end_time))
            .build();

        log::debug!("Fetching Statistics data for open interest...");

        // Get decoder for the data
        let mut decoder = self.client.timeseries().get_range(&params).await?;

        // Decode and convert to OpenInterest
        let mut open_interest_data = Vec::new();

        // Decode statistics records
        let oi_stat_type: u16 = databento::dbn::StatType::OpenInterest.into();
        while let Some(stat) = decoder
            .decode_record::<databento::dbn::StatMsg>()
            .await?
        {
            if stat.stat_type == oi_stat_type {
                // Use ts_ref as the reference timestamp for the statistic
                let time_ms = stat.ts_ref / 1_000_000; // Convert nanoseconds to milliseconds

                // Convert quantity (open interest is stored in quantity field)
                let oi_value =
                    if stat.quantity != databento::dbn::UNDEF_STAT_QUANTITY {
                        stat.quantity as f32
                    } else {
                        continue; // Skip undefined values
                    };

                open_interest_data.push(crate::types::OpenInterest {
                    time: time_ms,
                    open_interest: oi_value,
                });
            }
        }

        log::info!(
            "fetch_open_interest complete: {} data points for {}",
            open_interest_data.len(),
            symbol
        );

        Ok(open_interest_data)
    }
}
