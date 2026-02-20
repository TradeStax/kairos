//! Fetch orchestration methods for HistoricalDataManager
//!
//! Contains the primary fetch methods that coordinate caching, gap detection,
//! downloading, and assembly of OHLCV, trade, and depth data.

use super::HistoricalDataManager;
use super::aggregation::aggregate_to_timeframe;
use super::gaps::find_uncached_gaps;
use crate::adapter::databento::DatabentoError;
use crate::adapter::databento::mapper::{chrono_to_time, convert_databento_price, determine_stype};
use crate::{Kline, Timeframe, Trade};
use databento::{
    dbn::{Mbp10Msg, Schema, TradeMsg},
    historical::timeseries::GetRangeParams,
};
use std::collections::HashSet;

impl HistoricalDataManager {
    /// Identify cached days, find gaps, and fetch missing days for a schema.
    ///
    /// Returns the set of cached days for step 4 (loading from cache).
    async fn ensure_cached(
        &mut self,
        method_name: &str,
        symbol: &str,
        schema: Schema,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> HashSet<chrono::NaiveDate> {
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
            return cached_days;
        }

        log::debug!("Identified {} gap(s) to fetch", gaps.len());

        // Step 3: Fetch each gap
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
                }
            }
        }

        cached_days
    }

    /// Fetch OHLCV data with SMART gap-based batching AND progress reporting
    ///
    /// Identifies gaps in cached data and fetches them in batched requests,
    /// then saves each day individually for future reuse.
    ///
    /// Progress is logged at each step for UI tracking.
    pub async fn fetch_ohlcv(
        &mut self,
        symbol: &str,
        timeframe: Timeframe,
        range: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>),
    ) -> Result<Vec<Kline>, DatabentoError> {
        let (start, end) = range;
        let start_date = start.date_naive();
        let end_date = end.date_naive();

        // Always use OHLCV-1M for caching (most granular, can aggregate to any timeframe)
        let schema = Schema::Ohlcv1M;

        // Steps 1-3: Ensure all days are cached
        self.ensure_cached("fetch_ohlcv", symbol, schema, start_date, end_date)
            .await;

        // Step 4: Load all days from cache and assemble
        let total_days = (end_date - start_date).num_days() + 1;
        let mut all_klines = Vec::new();
        let mut current = start_date;
        let mut day_count = 0;

        while current <= end_date {
            day_count += 1;
            match self.load_day_from_cache(symbol, schema, current).await {
                Ok(day_klines) => {
                    all_klines.extend(day_klines);
                    if day_count % 5 == 0 || day_count == total_days as usize {
                        log::debug!(
                            "Progress: {}/{} days loaded, {} bars total",
                            day_count,
                            total_days,
                            all_klines.len()
                        );
                    }
                }
                Err(e) => {
                    log::warn!(
                        "Could not load {} for {}: {:?}",
                        symbol,
                        current,
                        e
                    );
                }
            }
            current += chrono::Duration::days(1);
        }

        if all_klines.is_empty() {
            return Err(DatabentoError::SymbolNotFound(format!(
                "No data found for {} in range {} to {}",
                symbol, start_date, end_date
            )));
        }

        // Step 5: Filter to exact datetime range
        let filtered: Vec<_> = all_klines
            .into_iter()
            .filter(|k| {
                let ktime = k.time as i64;
                let sms = start.timestamp_millis();
                let ems = end.timestamp_millis();
                ktime >= sms && ktime <= ems
            })
            .collect();

        // Step 6: Aggregate to target timeframe if needed
        log::debug!(
            "Before aggregation: {} 1M bars for {:?} target",
            filtered.len(),
            timeframe
        );

        let final_klines = aggregate_to_timeframe(filtered, timeframe);

        log::info!(
            "fetch_ohlcv complete: {} {:?} bars for {}",
            final_klines.len(),
            timeframe,
            symbol
        );

        // Log first few timestamps to verify spacing
        if final_klines.len() > 3 {
            let interval = timeframe.to_milliseconds();
            log::debug!("First bar: t={}", final_klines[0].time);
            log::debug!(
                "Second bar: t={} (delta={}ms, expected={}ms)",
                final_klines[1].time,
                final_klines[1].time - final_klines[0].time,
                interval
            );
            log::debug!(
                "Third bar: t={} (delta={}ms)",
                final_klines[2].time,
                final_klines[2].time - final_klines[1].time
            );
        }

        Ok(final_klines)
    }

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
        while let Some(trade_msg) =
            decoder.decode_record::<TradeMsg>().await?
        {
            let ts_recv = trade_msg
                .ts_recv()
                .ok_or_else(|| {
                    DatabentoError::Config("missing ts_recv".to_string())
                })?;
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
    /// It provides:
    /// - Per-day granular caching (.dbn.zst files)
    /// - Gap detection and batch fetching
    /// - Progress reporting at each step
    /// - Minimal API costs
    ///
    /// ## Cache Strategy
    /// - Cache key: "symbol/trades/YYYY-MM-DD.dbn.zst"
    /// - Checks cache per-day
    /// - Identifies gaps
    /// - Fetches only missing days
    /// - Saves per-day for future reuse
    ///
    /// ## Progress Reporting
    /// Logs detailed progress:
    /// - "Found X/Y days cached"
    /// - "Identified N gap(s) to fetch"
    /// - "Fetching gap 1/N: 2024-12-15 to 2024-12-18 (4 days)"
    /// - "Gap 1/N complete: Fetched and cached 4 days"
    /// - "Loading 10 days from cache..."
    /// - "Progress: 5/10 days loaded, 50,000 trades total"
    ///
    /// ## Example
    /// ```rust,ignore
    /// let trades = manager.fetch_trades_cached(
    ///     "ES.c.0",
    ///     (start, end)
    /// ).await?;
    /// // All trades from start to end, cached per-day
    /// ```
    pub async fn fetch_trades_cached(
        &mut self,
        symbol: &str,
        range: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>),
    ) -> Result<Vec<Trade>, DatabentoError> {
        let (start, end) = range;
        let start_date = start.date_naive();
        let end_date = end.date_naive();

        let schema = Schema::Trades;

        // Steps 1-3: Ensure all days are cached
        self.ensure_cached(
            "fetch_trades_cached",
            symbol,
            schema,
            start_date,
            end_date,
        )
        .await;

        // Step 4: Load all days from cache and assemble
        let total_days = (end_date - start_date).num_days() + 1;
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
    ///
    /// ## Progress Callback
    /// Called with `(days_complete, days_total, current_day)` after each day:
    /// - `days_complete`: Number of days processed so far
    /// - `days_total`: Total days in the requested range
    /// - `current_day`: The date that was just processed
    /// - `from_cache`: Whether the day was loaded from cache (true) or downloaded (false)
    ///
    /// ## Example
    /// ```rust,ignore
    /// let trades = manager.fetch_trades_cached_with_progress(
    ///     "ES.c.0",
    ///     (start, end),
    ///     |complete, total, day, from_cache| {
    ///         println!("Progress: {}/{} - {} (cached: {})", complete, total, day, from_cache);
    ///     }
    /// ).await?;
    /// ```
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

        log::debug!(
            "Found {}/{} days cached",
            cached_days.len(),
            total_days
        );

        // Step 2: Find gaps (consecutive uncached days)
        let gaps = find_uncached_gaps((start_date, end_date), &cached_days);

        log::debug!(
            "Identified {} gap(s) to fetch from databento",
            gaps.len()
        );
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
                        log::debug!(
                            "  Downloaded {} trades successfully",
                            gap_current
                        );
                    }
                    Err(e) => {
                        log::error!(
                            "  FAILED: download {} trades: {:?}",
                            gap_current,
                            e
                        );
                    }
                }

                // Update progress after each download
                days_processed += 1;
                progress_callback(
                    days_processed,
                    total_days_usize,
                    gap_current,
                    false,
                );

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
                progress_callback(
                    days_processed,
                    total_days_usize,
                    current,
                    true,
                );
            }

            load_days_count += 1;
            if load_days_count % 5 == 0
                || load_days_count == total_days as usize
            {
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
        while let Some(mbp) =
            decoder.decode_record::<Mbp10Msg>().await?
        {
            let ts_recv = mbp
                .ts_recv()
                .ok_or_else(|| {
                    DatabentoError::Config("missing ts_recv".to_string())
                })?;
            let time_ms = (ts_recv.unix_timestamp_nanos() / 1_000_000) as u64;

            let mut depth = crate::types::Depth::new(time_ms);

            // Add bid levels
            for level in &mbp.levels {
                if level.bid_px != databento::dbn::UNDEF_PRICE
                    && level.bid_sz > 0
                {
                    depth.bids.insert(
                        convert_databento_price(level.bid_px).units(),
                        level.bid_sz as f32,
                    );
                }
            }

            // Add ask levels
            for level in &mbp.levels {
                if level.ask_px != databento::dbn::UNDEF_PRICE
                    && level.ask_sz > 0
                {
                    depth.asks.insert(
                        convert_databento_price(level.ask_px).units(),
                        level.ask_sz as f32,
                    );
                }
            }

            snapshots.push((time_ms, depth));
        }

        log::info!(
            "Fetched {} MBP-10 snapshots for {}",
            snapshots.len(),
            symbol
        );

        Ok(snapshots)
    }

    /// Fetch MBP-10 depth with SMART per-day caching (RECOMMENDED)
    ///
    /// This is the PRIMARY method for fetching orderbook data for heatmaps.
    /// Provides:
    /// - Per-day granular caching (.dbn.zst files)
    /// - Gap detection and batch fetching
    /// - Progress reporting at each step
    /// - Minimal API costs
    ///
    /// ## Cache Strategy
    /// - Cache key: "symbol/mbp10/YYYY-MM-DD.dbn.zst"
    /// - Checks cache per-day
    /// - Identifies gaps
    /// - Fetches only missing days
    /// - Saves per-day for future reuse
    ///
    /// ## Example
    /// ```rust,ignore
    /// let depth = manager.fetch_mbp10_cached("ES.c.0", (start, end)).await?;
    /// // All depth snapshots from start to end, cached per-day
    /// ```
    pub async fn fetch_mbp10_cached(
        &mut self,
        symbol: &str,
        range: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>),
    ) -> Result<Vec<(u64, crate::types::Depth)>, DatabentoError> {
        let (start, end) = range;
        let start_date = start.date_naive();
        let end_date = end.date_naive();

        let schema = Schema::Mbp10;

        // Steps 1-3: Ensure all days are cached
        self.ensure_cached(
            "fetch_mbp10_cached",
            symbol,
            schema,
            start_date,
            end_date,
        )
        .await;

        // Step 4: Load all days from cache and assemble
        let total_days = (end_date - start_date).num_days() + 1;
        let mut all_snapshots = Vec::new();
        let mut current = start_date;
        let mut day_count = 0;

        while current <= end_date {
            day_count += 1;
            match self
                .load_mbp10_day_from_cache(symbol, current)
                .await
            {
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

        log::debug!(
            "fetch_open_interest: {} from {} to {}",
            symbol,
            start,
            end
        );

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
        let params = GetRangeParams::builder()
            .dataset(self.config.dataset)
            .schema(Schema::Statistics)
            .symbols(vec![symbol])
            .stype_in(determine_stype(symbol))
            .date_time_range((start_time, end_time))
            .build();

        log::debug!("Fetching Statistics data for open interest...");

        // Get decoder for the data
        let mut decoder =
            self.client.timeseries().get_range(&params).await?;

        // Decode and convert to OpenInterest
        let mut open_interest_data = Vec::new();

        // Decode statistics records
        let oi_stat_type: u16 =
            databento::dbn::StatType::OpenInterest.into();
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
