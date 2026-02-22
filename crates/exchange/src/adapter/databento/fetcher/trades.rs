//! Trade fetch methods for HistoricalDataManager

use super::HistoricalDataManager;
use super::gaps::find_uncached_gaps;
use crate::adapter::databento::DatabentoError;
use crate::adapter::databento::mapper::{chrono_to_time, convert_databento_price, determine_stype};
use crate::Trade;
use databento::{
    dbn::{Schema, TradeMsg},
    historical::timeseries::GetRangeParams,
};
use std::collections::HashSet;

impl HistoricalDataManager {
    /// Fetch trade data without caching (for testing/debugging only)
    ///
    /// For production use, call `fetch_trades_cached()` instead which provides
    /// automatic per-day caching and gap detection.
    pub async fn fetch_trades(
        &mut self,
        symbol: &str,
        range: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>),
    ) -> Result<Vec<Trade>, DatabentoError> {
        let (start, end) = range;

        // Cost warning check
        if let Some(warning) = self
            .config
            .check_date_range_cost(start, end, Schema::Trades)
        {
            log::warn!("{}", warning);
        }

        // Convert chrono DateTime to time OffsetDateTime
        let start_time = chrono_to_time(start)?;
        let end_time = chrono_to_time(end)?;

        let params = GetRangeParams::builder()
            .dataset(self.config.dataset)
            .schema(Schema::Trades)
            .symbols(vec![symbol])
            .stype_in(determine_stype(symbol))
            .date_time_range((start_time, end_time))
            .build();

        let mut decoder = self.client.timeseries().get_range(&params).await?;

        let mut trades = Vec::new();

        // Decode trade records
        while let Some(trade_msg) = decoder.decode_record::<TradeMsg>().await? {
            let ts_recv = trade_msg
                .ts_recv()
                .ok_or_else(|| DatabentoError::Config("missing ts_recv".to_string()))?;
            let time_ms = (ts_recv.unix_timestamp_nanos() / 1_000_000) as u64;

            // Determine side from action/side field
            let dbn_side = trade_msg.side()?;
            let side = match dbn_side {
                databento::dbn::Side::Ask => crate::types::TradeSide::Sell,
                _ => crate::types::TradeSide::Buy,
            };

            trades.push(Trade {
                time: time_ms,
                price: convert_databento_price(trade_msg.price).to_f32(),
                qty: trade_msg.size as f32,
                side,
            });
        }

        log::info!("Fetched {} trades for {}", trades.len(), symbol);

        Ok(trades)
    }

    /// Fetch trades with SMART per-day caching AND progress reporting (RECOMMENDED)
    ///
    /// This is the PRIMARY method for fetching trade data in production.
    /// Provides per-day granular caching, gap detection, batch fetching,
    /// progress reporting, and minimal API costs.
    pub async fn fetch_trades_cached(
        &mut self,
        symbol: &str,
        range: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>),
    ) -> Result<Vec<Trade>, DatabentoError> {
        let (start, end) = range;
        let start_date = start.date_naive();
        let end_date = end.date_naive();

        let schema = Schema::Trades;

        // Steps 1-3: Ensure all days are cached (fail-fast on fetch error)
        let cached_days = self
            .ensure_cached(
                "fetch_trades_cached",
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
                "Incomplete coverage for {} trades: only {}/{} days cached",
                symbol,
                cached_days.len(),
                total_days
            )));
        }

        // Step 4: Load all days from cache and assemble
        let mut all_trades = Vec::new();
        let mut current = start_date;
        let mut day_count = 0;

        while current <= end_date {
            day_count += 1;
            match self.load_trades_day_from_cache(symbol, current).await {
                Ok(day_trades) => {
                    all_trades.extend(day_trades);
                    if day_count % 5 == 0 || day_count == total_days as usize {
                        log::debug!(
                            "Progress: {}/{} days loaded, {} trades total",
                            day_count,
                            total_days,
                            all_trades.len()
                        );
                    }
                }
                Err(e) => {
                    log::warn!(
                        "Could not load trades for {} on {}: {:?}",
                        symbol,
                        current,
                        e
                    );
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

        // Step 5: Filter to exact datetime range
        let filtered: Vec<_> = all_trades
            .into_iter()
            .filter(|t| {
                let ttime = t.time as i64;
                let sms = start.timestamp_millis();
                let ems = end.timestamp_millis();
                ttime >= sms && ttime <= ems
            })
            .collect();

        log::info!(
            "fetch_trades_cached complete: {} trades for {}",
            filtered.len(),
            symbol
        );

        Ok(filtered)
    }

    /// Fetch trades with SMART per-day caching AND progress callback (RECOMMENDED)
    ///
    /// Like `fetch_trades_cached`, but calls `progress_callback` after each day
    /// to enable UI progress updates during long downloads.
    pub async fn fetch_trades_cached_with_progress<F>(
        &mut self,
        symbol: &str,
        range: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>),
        progress_callback: F,
    ) -> Result<Vec<Trade>, DatabentoError>
    where
        F: Fn(usize, usize, chrono::NaiveDate, bool),
    {
        let (start, end) = range;
        let start_date = start.date_naive();
        let end_date = end.date_naive();

        let total_days = (end_date - start_date).num_days() + 1;
        log::debug!(
            "fetch_trades_cached_with_progress: {} from {} to {} - {} days total",
            symbol,
            start_date,
            end_date,
            total_days
        );

        let schema = Schema::Trades;

        // Step 1: Identify which days are already cached
        let mut cached_days = HashSet::new();
        let mut current = start_date;
        while current <= end_date {
            if self.cache.has_cached(symbol, schema, current).await {
                cached_days.insert(current);
                log::debug!("Cache HIT: {} trades on {}", symbol, current);
            }
            current += chrono::Duration::days(1);
        }

        log::debug!("Found {}/{} days cached", cached_days.len(), total_days);

        // Step 2: Find gaps (consecutive uncached days)
        let gaps = find_uncached_gaps((start_date, end_date), &cached_days);

        log::debug!("Identified {} gap(s) to fetch from databento", gaps.len());
        for gap in &gaps {
            log::debug!(
                "  Gap: {} to {} ({} days)",
                gap.start,
                gap.end,
                (gap.end - gap.start).num_days() + 1
            );
        }

        // Step 3: Fetch each gap, save per-day with PROGRESS CALLBACK
        let num_gaps = gaps.len();
        let mut days_processed = 0usize;
        let total_days_usize = total_days as usize;

        for (gap_idx, gap) in gaps.iter().enumerate() {
            let gap_days = (gap.end - gap.start).num_days() + 1;
            log::debug!(
                "Fetching gap {}/{}: {} to {} ({} days)",
                gap_idx + 1,
                num_gaps,
                gap.start,
                gap.end,
                gap_days
            );

            // Fetch each day in the gap individually with progress reporting
            let mut gap_current = gap.start;
            while gap_current <= gap.end {
                match self
                    .fetch_to_cache(symbol, Schema::Trades, gap_current)
                    .await
                {
                    Ok(_) => {
                        log::debug!("  Downloaded {} trades successfully", gap_current);
                    }
                    Err(e) => {
                        log::error!("  FAILED: download {} trades: {:?}", gap_current, e);
                    }
                }

                // Update progress after each download
                days_processed += 1;
                progress_callback(days_processed, total_days_usize, gap_current, false);

                gap_current += chrono::Duration::days(1);
            }
        }

        // Step 4: Load all days from cache and assemble with PROGRESS CALLBACK
        let mut all_trades = Vec::new();
        let mut current = start_date;
        let mut load_days_count = 0;

        log::debug!("Loading {} days from cache...", total_days);

        while current <= end_date {
            let was_cached = cached_days.contains(&current);

            match self.load_trades_day_from_cache(symbol, current).await {
                Ok(day_trades) => {
                    all_trades.extend(day_trades);
                }
                Err(e) => {
                    log::warn!(
                        "Could not load trades for {} on {}: {:?}",
                        symbol,
                        current,
                        e
                    );
                }
            }

            // Report progress for cached days (downloaded days were already reported)
            if was_cached {
                days_processed += 1;
                progress_callback(days_processed, total_days_usize, current, true);
            }

            load_days_count += 1;
            if load_days_count % 5 == 0 || load_days_count == total_days as usize {
                log::debug!(
                    "Progress: {}/{} days loaded, {} trades total",
                    load_days_count,
                    total_days,
                    all_trades.len()
                );
            }

            current += chrono::Duration::days(1);
        }

        if all_trades.is_empty() {
            return Err(DatabentoError::SymbolNotFound(format!(
                "No trade data found for {} in range {} to {}",
                symbol, start_date, end_date
            )));
        }

        // Step 5: Filter to exact datetime range
        let filtered: Vec<_> = all_trades
            .into_iter()
            .filter(|t| {
                let ttime = t.time as i64;
                let sms = start.timestamp_millis();
                let ems = end.timestamp_millis();
                ttime >= sms && ttime <= ems
            })
            .collect();

        log::info!(
            "fetch_trades_cached_with_progress complete: {} trades for {}",
            filtered.len(),
            symbol
        );

        Ok(filtered)
    }
}
