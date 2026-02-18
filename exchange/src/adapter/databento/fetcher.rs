//! Historical market data fetching with SMART per-day caching
//!
//! Caching Strategy:
//! - Saves data PER DAY for fine-grained reuse
//! - Fetches GAPS IN BATCHES to minimize API calls
//! - Example: Need 5/6-5/18, have 5/10-5/13 + 5/16 cached
//!   → Makes 3 API calls: [5/6-5/9], [5/14-5/15], [5/17-5/18]
//!   → Saves each day individually for future reuse

use super::{DatabentoConfig, DatabentoError, cache::CacheManager, client};
use super::mapper::{chrono_to_time, convert_databento_price, determine_stype};
use crate::{Kline, Timeframe, Trade};
use databento::{
    HistoricalClient,
    dbn::{Mbp10Msg, OhlcvMsg, Schema, TradeMsg, decode::AsyncDbnDecoder},
    historical::timeseries::{GetRangeParams, GetRangeToFileParams},
};
use std::collections::HashSet;
use std::path::PathBuf;
use time::OffsetDateTime;

/// Date range gap (consecutive uncached days)
#[derive(Debug, Clone)]
struct DateGap {
    start: chrono::NaiveDate,
    end: chrono::NaiveDate,
}

/// Manages historical data fetching and caching
pub struct HistoricalDataManager {
    pub(crate) client: HistoricalClient,
    pub(crate) cache: CacheManager, // Made pub(crate) for repository access
    pub(crate) config: DatabentoConfig,
}

impl HistoricalDataManager {
    /// Create a new historical data manager
    pub async fn new(config: DatabentoConfig) -> Result<Self, DatabentoError> {
        let client = client::create_historical_client(&config)?;

        let cache = CacheManager::new(&config);
        cache.init().await?;

        log::info!("HistoricalDataManager initialized with smart per-day caching");

        Ok(Self {
            client,
            cache,
            config,
        })
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

        let total_days = (end_date - start_date).num_days() + 1;
        log::info!(
            "fetch_ohlcv: {} from {} to {} ({:?}) - {} days total",
            symbol,
            start_date,
            end_date,
            timeframe,
            total_days
        );

        // Always use OHLCV-1M for caching (most granular, can aggregate to any timeframe)
        let schema = Schema::Ohlcv1M;

        // Step 1: Identify which days are already cached
        let mut cached_days = HashSet::new();
        let mut current = start_date;
        while current <= end_date {
            if self.cache.has_cached(symbol, schema, current).await {
                cached_days.insert(current);
                log::debug!("Cache HIT: {} on {}", symbol, current);
            }
            current += chrono::Duration::days(1);
        }

        log::info!(
            "Found {}/{} days cached",
            cached_days.len(),
            (end_date - start_date).num_days() + 1
        );

        // Step 2: Find gaps (consecutive uncached days)
        let gaps = find_uncached_gaps((start_date, end_date), &cached_days);

        log::info!("Identified {} gap(s) to fetch from databento", gaps.len());
        for gap in &gaps {
            log::info!(
                "  Gap: {} to {} ({} days)",
                gap.start,
                gap.end,
                (gap.end - gap.start).num_days() + 1
            );
        }

        // Step 3: Fetch each gap, save per-day with PROGRESS REPORTING
        let num_gaps = gaps.len();
        for (gap_idx, gap) in gaps.iter().enumerate() {
            let gap_days = (gap.end - gap.start).num_days() + 1;
            log::info!(
                "Fetching gap {}/{}: {} to {} ({} days)",
                gap_idx + 1,
                num_gaps,
                gap.start,
                gap.end,
                gap_days
            );

            match self
                .fetch_and_cache_range(symbol, schema, gap.start, gap.end)
                .await
            {
                Ok(days_saved) => {
                    log::info!(
                        "✓ Gap {}/{} complete: Fetched and cached {} days",
                        gap_idx + 1,
                        num_gaps,
                        days_saved
                    );
                }
                Err(e) => {
                    log::error!(
                        "✗ Gap {}/{} failed: {} to {} - {:?}",
                        gap_idx + 1,
                        num_gaps,
                        gap.start,
                        gap.end,
                        e
                    );
                    // Continue with other gaps
                }
            }
        }

        // Step 4: Load all days from cache and assemble with PROGRESS REPORTING
        let mut all_klines = Vec::new();
        let mut current = start_date;
        let mut day_count = 0;

        log::info!("Loading {} days from cache...", total_days);

        while current <= end_date {
            day_count += 1;
            match self.load_day_from_cache(symbol, schema, current).await {
                Ok(day_klines) => {
                    all_klines.extend(day_klines);
                    if day_count % 5 == 0 || day_count == total_days as usize {
                        log::info!(
                            "Progress: {}/{} days loaded, {} bars total",
                            day_count,
                            total_days,
                            all_klines.len()
                        );
                    }
                }
                Err(e) => {
                    log::warn!("Could not load {} for {}: {:?}", symbol, current, e);
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
        log::info!(
            "Before aggregation: {} 1M bars for {:?} target",
            filtered.len(),
            timeframe
        );

        let final_klines = aggregate_to_timeframe(filtered, timeframe);

        log::info!(
            "✓ fetch_ohlcv COMPLETE: {} {:?} bars for {}",
            final_klines.len(),
            timeframe,
            symbol
        );
        log::info!(
            "  Cache efficiency: {}/{} days cached, {} gaps fetched",
            cached_days.len(),
            total_days,
            num_gaps
        );
        log::info!(
            "  Final output: {} to {} ({} bars)",
            final_klines.first().map(|k| k.time).unwrap_or(0),
            final_klines.last().map(|k| k.time).unwrap_or(0),
            final_klines.len()
        );

        // Log first few timestamps to verify spacing
        if final_klines.len() > 3 {
            let interval = timeframe.to_milliseconds();
            log::info!("First bar: t={}", final_klines[0].time);
            log::info!(
                "Second bar: t={} (Δ={}ms, expected={}ms)",
                final_klines[1].time,
                final_klines[1].time - final_klines[0].time,
                interval
            );
            log::info!(
                "Third bar: t={} (Δ={}ms)",
                final_klines[2].time,
                final_klines[2].time - final_klines[1].time
            );
        }

        Ok(final_klines)
    }

    /// Fetch a gap range and cache each day with DETAILED PROGRESS
    async fn fetch_and_cache_range(
        &mut self,
        symbol: &str,
        schema: Schema,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> Result<usize, DatabentoError> {
        let total_gap_days = (end_date - start_date).num_days() + 1;
        let mut days_saved = 0;
        let mut current = start_date;
        let mut day_num = 0;

        log::info!("Fetching {} days from databento...", total_gap_days);

        while current <= end_date {
            day_num += 1;
            match self.fetch_to_cache(symbol, schema, current).await {
                Ok(_) => {
                    days_saved += 1;
                    log::info!(
                        "  ✓ Day {}/{}: {} cached successfully",
                        day_num,
                        total_gap_days,
                        current
                    );
                }
                Err(e) => {
                    log::error!(
                        "  ✗ Day {}/{}: {} failed - {:?}",
                        day_num,
                        total_gap_days,
                        current,
                        e
                    );
                }
            }
            current += chrono::Duration::days(1);
        }

        log::info!(
            "Gap fetch complete: {}/{} days successfully cached",
            days_saved,
            total_gap_days
        );
        Ok(days_saved)
    }

    /// Load a single day from cache
    async fn load_day_from_cache(
        &mut self,
        symbol: &str,
        schema: Schema,
        date: chrono::NaiveDate,
    ) -> Result<Vec<Kline>, DatabentoError> {
        let cache_path = self
            .cache
            .get_cached(symbol, schema, date)
            .await
            .ok_or_else(|| DatabentoError::Cache(format!("No cache for {} on {}", symbol, date)))?;

        let mut decoder = AsyncDbnDecoder::from_zstd_file(cache_path).await?;

        let mut klines = Vec::new();

        while let Some(ohlcv) = decoder.decode_record::<OhlcvMsg>().await? {
            let ts_event = ohlcv
                .hd
                .ts_event()
                .ok_or_else(|| DatabentoError::Config("missing ts_event".to_string()))?;
            let time_ms = (ts_event.unix_timestamp_nanos() / 1_000_000) as u64;

            klines.push(Kline {
                time: time_ms,
                open: convert_databento_price(ohlcv.open).to_f32(),
                high: convert_databento_price(ohlcv.high).to_f32(),
                low: convert_databento_price(ohlcv.low).to_f32(),
                close: convert_databento_price(ohlcv.close).to_f32(),
                volume: ohlcv.volume as f32,
                buy_volume: 0.0, // OHLCV schema doesn't have buy/sell split
                sell_volume: 0.0,
            });
        }

        Ok(klines)
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
    /// - "✓ Gap 1/N complete: Fetched and cached 4 days"
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

        let total_days = (end_date - start_date).num_days() + 1;
        log::info!(
            "fetch_trades_cached: {} from {} to {} - {} days total",
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

        log::info!("Found {}/{} days cached", cached_days.len(), total_days);

        // Step 2: Find gaps (consecutive uncached days)
        let gaps = find_uncached_gaps((start_date, end_date), &cached_days);

        log::info!("Identified {} gap(s) to fetch from databento", gaps.len());
        for gap in &gaps {
            log::info!(
                "  Gap: {} to {} ({} days)",
                gap.start,
                gap.end,
                (gap.end - gap.start).num_days() + 1
            );
        }

        // Step 3: Fetch each gap, save per-day with PROGRESS REPORTING
        let num_gaps = gaps.len();
        for (gap_idx, gap) in gaps.iter().enumerate() {
            let gap_days = (gap.end - gap.start).num_days() + 1;
            log::info!(
                "Fetching gap {}/{}: {} to {} ({} days)",
                gap_idx + 1,
                num_gaps,
                gap.start,
                gap.end,
                gap_days
            );

            match self
                .fetch_and_cache_trades_range(symbol, gap.start, gap.end)
                .await
            {
                Ok(days_saved) => {
                    log::info!(
                        "✓ Gap {}/{} complete: Fetched and cached {} days",
                        gap_idx + 1,
                        num_gaps,
                        days_saved
                    );
                }
                Err(e) => {
                    log::error!(
                        "✗ Gap {}/{} failed: {} to {} - {:?}",
                        gap_idx + 1,
                        num_gaps,
                        gap.start,
                        gap.end,
                        e
                    );
                    // Continue with other gaps
                }
            }
        }

        // Step 4: Load all days from cache and assemble with PROGRESS REPORTING
        let mut all_trades = Vec::new();
        let mut current = start_date;
        let mut day_count = 0;

        log::info!("Loading {} days from cache...", total_days);

        while current <= end_date {
            day_count += 1;
            match self.load_trades_day_from_cache(symbol, current).await {
                Ok(day_trades) => {
                    all_trades.extend(day_trades);
                    if day_count % 5 == 0 || day_count == total_days as usize {
                        log::info!(
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
            "✓ fetch_trades_cached COMPLETE: {} trades for {}",
            filtered.len(),
            symbol
        );
        log::info!(
            "  Cache efficiency: {}/{} days cached, {} gaps fetched",
            cached_days.len(),
            total_days,
            num_gaps
        );
        log::info!(
            "  Final output: {} to {} ({} trades)",
            filtered.first().map(|t| t.time).unwrap_or(0),
            filtered.last().map(|t| t.time).unwrap_or(0),
            filtered.len()
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
        log::info!(
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

        log::info!("Found {}/{} days cached", cached_days.len(), total_days);

        // Step 2: Find gaps (consecutive uncached days)
        let gaps = find_uncached_gaps((start_date, end_date), &cached_days);

        log::info!("Identified {} gap(s) to fetch from databento", gaps.len());
        for gap in &gaps {
            log::info!(
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
            log::info!(
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
                match self.fetch_to_cache(symbol, Schema::Trades, gap_current).await {
                    Ok(_) => {
                        log::info!("  ✓ Downloaded {} trades successfully", gap_current);
                    }
                    Err(e) => {
                        log::error!("  ✗ Failed to download {} trades: {:?}", gap_current, e);
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

        log::info!("Loading {} days from cache...", total_days);

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
                log::info!(
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
            "✓ fetch_trades_cached_with_progress COMPLETE: {} trades for {}",
            filtered.len(),
            symbol
        );

        Ok(filtered)
    }

    /// Fetch a gap range of trades and cache each day with DETAILED PROGRESS
    async fn fetch_and_cache_trades_range(
        &mut self,
        symbol: &str,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> Result<usize, DatabentoError> {
        let total_gap_days = (end_date - start_date).num_days() + 1;
        let mut days_saved = 0;
        let mut current = start_date;
        let mut day_num = 0;

        log::info!(
            "Fetching {} days of trades from databento...",
            total_gap_days
        );

        while current <= end_date {
            day_num += 1;
            match self.fetch_to_cache(symbol, Schema::Trades, current).await {
                Ok(_) => {
                    days_saved += 1;
                    log::info!(
                        "  ✓ Day {}/{}: {} trades cached successfully",
                        day_num,
                        total_gap_days,
                        current
                    );
                }
                Err(e) => {
                    log::error!(
                        "  ✗ Day {}/{}: {} trades failed - {:?}",
                        day_num,
                        total_gap_days,
                        current,
                        e
                    );
                }
            }
            current += chrono::Duration::days(1);
        }

        log::info!(
            "Gap fetch complete: {}/{} days successfully cached",
            days_saved,
            total_gap_days
        );
        Ok(days_saved)
    }

    /// Load a single day of trades from cache
    async fn load_trades_day_from_cache(
        &mut self,
        symbol: &str,
        date: chrono::NaiveDate,
    ) -> Result<Vec<Trade>, DatabentoError> {
        let cache_path = self
            .cache
            .get_cached(symbol, Schema::Trades, date)
            .await
            .ok_or_else(|| {
                DatabentoError::Cache(format!("No cache for {} trades on {}", symbol, date))
            })?;

        let mut decoder = AsyncDbnDecoder::from_zstd_file(cache_path).await?;

        let mut trades = Vec::new();

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

        Ok(trades)
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
        if let Some(warning) = self.config.check_date_range_cost(start, end, Schema::Mbp10) {
            log::warn!("{}", warning);
        }

        log::info!(
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
                        convert_databento_price(level.bid_px).units,
                        level.bid_sz as f32,
                    );
                }
            }

            // Add ask levels
            for level in &mbp.levels {
                if level.ask_px != databento::dbn::UNDEF_PRICE && level.ask_sz > 0 {
                    depth.asks.insert(
                        convert_databento_price(level.ask_px).units,
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

        let total_days = (end_date - start_date).num_days() + 1;
        log::info!(
            "fetch_mbp10_cached: {} from {} to {} - {} days total",
            symbol,
            start_date,
            end_date,
            total_days
        );

        let schema = Schema::Mbp10;

        // Step 1: Identify which days are already cached
        let mut cached_days = HashSet::new();
        let mut current = start_date;
        while current <= end_date {
            if self.cache.has_cached(symbol, schema, current).await {
                cached_days.insert(current);
                log::debug!("Cache HIT: {} MBP-10 on {}", symbol, current);
            }
            current += chrono::Duration::days(1);
        }

        log::info!(
            "Found {}/{} days cached (MBP-10)",
            cached_days.len(),
            total_days
        );

        // Step 2: Find gaps (consecutive uncached days)
        let gaps = find_uncached_gaps((start_date, end_date), &cached_days);

        log::info!(
            "Identified {} MBP-10 gap(s) to fetch from databento",
            gaps.len()
        );
        for gap in &gaps {
            log::info!(
                "  MBP-10 Gap: {} to {} ({} days)",
                gap.start,
                gap.end,
                (gap.end - gap.start).num_days() + 1
            );
        }

        // Step 3: Fetch each gap, save per-day with PROGRESS REPORTING
        let num_gaps = gaps.len();
        for (gap_idx, gap) in gaps.iter().enumerate() {
            let gap_days = (gap.end - gap.start).num_days() + 1;
            log::info!(
                "Fetching MBP-10 gap {}/{}: {} to {} ({} days)",
                gap_idx + 1,
                num_gaps,
                gap.start,
                gap.end,
                gap_days
            );

            match self
                .fetch_and_cache_mbp10_range(symbol, gap.start, gap.end)
                .await
            {
                Ok(days_saved) => {
                    log::info!(
                        "✓ MBP-10 gap {}/{} complete: Fetched and cached {} days",
                        gap_idx + 1,
                        num_gaps,
                        days_saved
                    );
                }
                Err(e) => {
                    log::error!(
                        "✗ MBP-10 gap {}/{} failed: {} to {} - {:?}",
                        gap_idx + 1,
                        num_gaps,
                        gap.start,
                        gap.end,
                        e
                    );
                }
            }
        }

        // Step 4: Load all days from cache and assemble with PROGRESS REPORTING
        let mut all_snapshots = Vec::new();
        let mut current = start_date;
        let mut day_count = 0;

        log::info!("Loading {} days of MBP-10 from cache...", total_days);

        while current <= end_date {
            day_count += 1;
            match self.load_mbp10_day_from_cache(symbol, current).await {
                Ok(day_snapshots) => {
                    all_snapshots.extend(day_snapshots);
                    if day_count % 5 == 0 || day_count == total_days as usize {
                        log::info!(
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
            "✓ fetch_mbp10_cached COMPLETE: {} snapshots for {}",
            filtered.len(),
            symbol
        );
        log::info!(
            "  Cache efficiency: {}/{} days cached, {} gaps fetched",
            cached_days.len(),
            total_days,
            num_gaps
        );
        log::info!(
            "  Final output: {} to {} ({} snapshots)",
            filtered.first().map(|(t, _)| *t).unwrap_or(0),
            filtered.last().map(|(t, _)| *t).unwrap_or(0),
            filtered.len()
        );

        Ok(filtered)
    }

    /// Fetch a gap range of MBP-10 and cache each day with DETAILED PROGRESS
    async fn fetch_and_cache_mbp10_range(
        &mut self,
        symbol: &str,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> Result<usize, DatabentoError> {
        let total_gap_days = (end_date - start_date).num_days() + 1;
        let mut days_saved = 0;
        let mut current = start_date;
        let mut day_num = 0;

        log::info!(
            "Fetching {} days of MBP-10 from databento...",
            total_gap_days
        );

        while current <= end_date {
            day_num += 1;
            match self.fetch_to_cache(symbol, Schema::Mbp10, current).await {
                Ok(_) => {
                    days_saved += 1;
                    log::info!(
                        "  ✓ Day {}/{}: {} MBP-10 cached successfully",
                        day_num,
                        total_gap_days,
                        current
                    );
                }
                Err(e) => {
                    log::error!(
                        "  ✗ Day {}/{}: {} MBP-10 failed - {:?}",
                        day_num,
                        total_gap_days,
                        current,
                        e
                    );
                }
            }
            current += chrono::Duration::days(1);
        }

        log::info!(
            "MBP-10 gap fetch complete: {}/{} days successfully cached",
            days_saved,
            total_gap_days
        );
        Ok(days_saved)
    }

    /// Load a single day of MBP-10 depth from cache
    async fn load_mbp10_day_from_cache(
        &mut self,
        symbol: &str,
        date: chrono::NaiveDate,
    ) -> Result<Vec<(u64, crate::types::Depth)>, DatabentoError> {
        let cache_path = self
            .cache
            .get_cached(symbol, Schema::Mbp10, date)
            .await
            .ok_or_else(|| {
                DatabentoError::Cache(format!("No cache for {} MBP-10 on {}", symbol, date))
            })?;

        let mut decoder = AsyncDbnDecoder::from_zstd_file(cache_path).await?;

        let mut snapshots = Vec::new();

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
                        convert_databento_price(level.bid_px).units,
                        level.bid_sz as f32,
                    );
                }
            }

            // Add ask levels
            for level in &mbp.levels {
                if level.ask_px != databento::dbn::UNDEF_PRICE && level.ask_sz > 0 {
                    depth.asks.insert(
                        convert_databento_price(level.ask_px).units,
                        level.ask_sz as f32,
                    );
                }
            }

            snapshots.push((time_ms, depth));
        }

        Ok(snapshots)
    }

    /// Fetch data to a file and cache it (RECOMMENDED WORKFLOW)
    ///
    /// Downloads full day of data and caches locally. This is the most cost-effective approach:
    /// - Download once, use many times
    /// - No repeated API calls
    /// - Historical data doesn't change
    ///
    /// Based on programatic-batch.rs example
    pub async fn fetch_to_cache(
        &mut self,
        symbol: &str,
        schema: Schema,
        date: chrono::NaiveDate,
    ) -> Result<PathBuf, DatabentoError> {
        // Check if already cached
        if let Some(cached_path) = self.cache.get_cached(symbol, schema, date).await {
            log::debug!("Using cached data for {} on {} (no API cost)", symbol, date);
            return Ok(cached_path);
        }

        // Warn about expensive schemas
        if super::DatabentoConfig::is_expensive_schema(schema) {
            log::error!(
                "COST WARNING: Downloading {:?} is VERY expensive! Consider using MBP-10 instead of MBO.",
                schema
            );
        }

        // Create temp file path
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!(
            "databento_{}_{:?}_{}.dbn.zst",
            symbol.replace('.', "-"),
            schema,
            date.format("%Y%m%d")
        ));

        // Convert date to time OffsetDateTime (start of day UTC)
        let start_time = OffsetDateTime::from_unix_timestamp(
            chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                date.and_hms_opt(0, 0, 0).unwrap(),
                chrono::Utc,
            )
            .timestamp(),
        )
        .map_err(|e| DatabentoError::Config(format!("Invalid start time: {}", e)))?;

        let end_time = OffsetDateTime::from_unix_timestamp(
            chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                (date + chrono::Duration::days(1))
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
                chrono::Utc,
            )
            .timestamp(),
        )
        .map_err(|e| DatabentoError::Config(format!("Invalid end time: {}", e)))?;

        let params = GetRangeToFileParams::builder()
            .dataset(self.config.dataset)
            .schema(schema)
            .symbols(vec![symbol])
            .stype_in(determine_stype(symbol))
            .date_time_range((start_time, end_time))
            .path(&temp_path)
            .build();

        // Download to temp file with retry on transient errors
        let max_retries = 3u32;
        let mut last_err = None;
        for attempt in 0..max_retries {
            match self.client.timeseries().get_range_to_file(&params).await {
                Ok(_) => {
                    last_err = None;
                    break;
                }
                Err(e) => {
                    let err_str = e.to_string();
                    let is_retriable = err_str.contains("429")
                        || err_str.contains("rate")
                        || err_str.contains("too many")
                        || err_str.contains("503")
                        || err_str.contains("timeout");

                    if is_retriable && attempt + 1 < max_retries {
                        let delay = 1u64 << attempt; // 1s, 2s
                        log::warn!(
                            "Databento API error (attempt {}/{}), \
                             retrying in {}s: {}",
                            attempt + 1,
                            max_retries,
                            delay,
                            err_str
                        );
                        tokio::time::sleep(
                            std::time::Duration::from_secs(delay),
                        )
                        .await;
                        last_err = Some(e);
                    } else {
                        return Err(DatabentoError::from(e));
                    }
                }
            }
        }
        if let Some(e) = last_err {
            return Err(DatabentoError::from(e));
        }

        log::info!("Downloaded {} ({:?}) for {}", symbol, schema, date);

        // Move to cache
        self.cache.store(symbol, schema, date, &temp_path).await?;

        // Clean up temp file
        let _ = tokio::fs::remove_file(&temp_path).await;

        // Return cached path
        self.cache
            .get_cached(symbol, schema, date)
            .await
            .ok_or_else(|| DatabentoError::Cache("Failed to retrieve cached file".to_string()))
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> Result<super::cache::CacheStats, DatabentoError> {
        self.cache.stats().await
    }

    /// Get cached date ranges for a symbol
    pub async fn get_cached_date_ranges(&self, symbol: &str) -> Result<Vec<chrono::NaiveDate>, DatabentoError> {
        use tokio::fs;

        let safe_symbol = symbol.replace('.', "-");
        let symbol_dir = self.cache.cache_root.join(&safe_symbol);

        let mut dates = Vec::new();

        // Check if symbol directory exists
        if !symbol_dir.exists() {
            return Ok(dates);
        }

        // Walk through schema directories
        let mut schema_entries = fs::read_dir(&symbol_dir)
            .await
            .map_err(|e| DatabentoError::Cache(format!("Failed to read symbol cache dir: {}", e)))?;

        while let Some(schema_entry) = schema_entries.next_entry()
            .await
            .map_err(|e| DatabentoError::Cache(format!("Failed to read schema entry: {}", e)))?
        {
            if !schema_entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }

            // Read date files in schema directory
            let mut date_entries = fs::read_dir(schema_entry.path())
                .await
                .map_err(|e| DatabentoError::Cache(format!("Failed to read schema dir: {}", e)))?;

            while let Some(date_entry) = date_entries.next_entry()
                .await
                .map_err(|e| DatabentoError::Cache(format!("Failed to read date entry: {}", e)))?
            {
                // Parse date from filename (format: YYYY-MM-DD.dbn.zst)
                if let Some(filename) = date_entry.file_name().to_str()
                    && let Some(date_str) = filename.strip_suffix(".dbn.zst")
                        && let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                            dates.push(date);
                        }
            }
        }

        // Sort and deduplicate
        dates.sort();
        dates.dedup();

        Ok(dates)
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

        log::info!("fetch_open_interest: {} from {} to {}", symbol, start, end);

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

        log::info!("Fetching Statistics data for open interest...");

        // Get decoder for the data
        let mut decoder = self.client.timeseries().get_range(&params).await?;

        // Decode and convert to OpenInterest
        let mut open_interest_data = Vec::new();

        // Decode statistics records
        let oi_stat_type: u16 =
            databento::dbn::StatType::OpenInterest.into();
        while let Some(stat) = decoder.decode_record::<databento::dbn::StatMsg>().await? {
            if stat.stat_type == oi_stat_type {
                // Use ts_ref as the reference timestamp for the statistic
                let time_ms = stat.ts_ref / 1_000_000; // Convert nanoseconds to milliseconds

                // Convert quantity (open interest is stored in quantity field)
                let oi_value = if stat.quantity != databento::dbn::UNDEF_STAT_QUANTITY {
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
            "✓ fetch_open_interest COMPLETE: {} data points for {}",
            open_interest_data.len(),
            symbol
        );

        Ok(open_interest_data)
    }

    /// Clean up old cache files
    pub async fn cleanup_cache(&self) -> Result<usize, DatabentoError> {
        self.cache.cleanup_old_files().await
    }
}

/// Find consecutive gaps in cached days
fn find_uncached_gaps(
    requested_range: (chrono::NaiveDate, chrono::NaiveDate),
    cached_days: &HashSet<chrono::NaiveDate>,
) -> Vec<DateGap> {
    let (start, end) = requested_range;
    let mut gaps = Vec::new();
    let mut gap_start: Option<chrono::NaiveDate> = None;

    let mut current = start;
    while current <= end {
        if cached_days.contains(&current) {
            // This day is cached - close any open gap
            if let Some(gap_s) = gap_start {
                gaps.push(DateGap {
                    start: gap_s,
                    end: current - chrono::Duration::days(1),
                });
                gap_start = None;
            }
        } else {
            // This day is NOT cached - extend or start gap
            if gap_start.is_none() {
                gap_start = Some(current);
            }
        }
        current += chrono::Duration::days(1);
    }

    // Close final gap if exists
    if let Some(gap_s) = gap_start {
        gaps.push(DateGap { start: gap_s, end });
    }

    gaps
}

/// Aggregate 1-minute bars to target timeframe
fn aggregate_to_timeframe(klines_1m: Vec<Kline>, target: Timeframe) -> Vec<Kline> {
    match target {
        Timeframe::M1 => klines_1m, // No aggregation needed
        Timeframe::M3 => aggregate_minutes(klines_1m, 3),
        Timeframe::M5 => aggregate_minutes(klines_1m, 5),
        Timeframe::M15 => aggregate_minutes(klines_1m, 15),
        Timeframe::M30 => aggregate_minutes(klines_1m, 30),
        Timeframe::H1 => aggregate_minutes(klines_1m, 60),
        Timeframe::H4 => aggregate_minutes(klines_1m, 240),
        Timeframe::D1 => aggregate_minutes(klines_1m, 1440),
        _ => {
            log::warn!(
                "Unsupported timeframe for aggregation: {:?}, returning 1M bars",
                target
            );
            klines_1m
        }
    }
}

/// Aggregate 1-minute bars into N-minute bars
fn aggregate_minutes(bars_1m: Vec<Kline>, minutes: u32) -> Vec<Kline> {
    if bars_1m.is_empty() {
        return Vec::new();
    }

    let interval_ms = (minutes as u64) * 60 * 1000;
    let mut aggregated = Vec::new();
    let mut current_group = Vec::new();

    let mut group_start_time = (bars_1m[0].time / interval_ms) * interval_ms;

    for bar in bars_1m {
        let bar_group_time = (bar.time / interval_ms) * interval_ms;

        if bar_group_time != group_start_time {
            // New group - aggregate previous
            if !current_group.is_empty() {
                aggregated.push(aggregate_group(&current_group, group_start_time));
                current_group.clear();
            }
            group_start_time = bar_group_time;
        }

        current_group.push(bar);
    }

    // Aggregate final group
    if !current_group.is_empty() {
        aggregated.push(aggregate_group(&current_group, group_start_time));
    }

    aggregated
}

/// Aggregate a group of bars into one bar
fn aggregate_group(bars: &[Kline], time: u64) -> Kline {
    let open = bars.first().unwrap().open;
    let close = bars.last().unwrap().close;
    let high = bars
        .iter()
        .map(|b| b.high)
        .fold(f32::NEG_INFINITY, f32::max);
    let low = bars.iter().map(|b| b.low).fold(f32::INFINITY, f32::min);
    let volume = bars.iter().map(|b| b.volume).sum();
    let buy_volume = bars.iter().map(|b| b.buy_volume).sum();
    let sell_volume = bars.iter().map(|b| b.sell_volume).sum();

    Kline {
        time,
        open,
        high,
        low,
        close,
        volume,
        buy_volume,
        sell_volume,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gap_detection() {
        use chrono::NaiveDate;

        let start = NaiveDate::from_ymd_opt(2024, 5, 6).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 5, 18).unwrap();

        // Cached: 5/10-5/13 + 5/16
        let mut cached = HashSet::new();
        cached.insert(NaiveDate::from_ymd_opt(2024, 5, 10).unwrap());
        cached.insert(NaiveDate::from_ymd_opt(2024, 5, 11).unwrap());
        cached.insert(NaiveDate::from_ymd_opt(2024, 5, 12).unwrap());
        cached.insert(NaiveDate::from_ymd_opt(2024, 5, 13).unwrap());
        cached.insert(NaiveDate::from_ymd_opt(2024, 5, 16).unwrap());

        let gaps = find_uncached_gaps((start, end), &cached);

        // Should find 3 gaps: [5/6-5/9], [5/14-5/15], [5/17-5/18]
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0].start, NaiveDate::from_ymd_opt(2024, 5, 6).unwrap());
        assert_eq!(gaps[0].end, NaiveDate::from_ymd_opt(2024, 5, 9).unwrap());
        assert_eq!(gaps[1].start, NaiveDate::from_ymd_opt(2024, 5, 14).unwrap());
        assert_eq!(gaps[1].end, NaiveDate::from_ymd_opt(2024, 5, 15).unwrap());
        assert_eq!(gaps[2].start, NaiveDate::from_ymd_opt(2024, 5, 17).unwrap());
        assert_eq!(gaps[2].end, NaiveDate::from_ymd_opt(2024, 5, 18).unwrap());
    }

    #[test]
    fn test_aggregate_timeframe() {
        // Test aggregation from 1M to 5M
        let klines_1m = vec![
            // 5 consecutive 1-minute bars
            create_test_kline(0, 100.0, 102.0, 99.0, 101.0, 10.0),
            create_test_kline(60000, 101.0, 103.0, 100.0, 102.0, 15.0),
            create_test_kline(120000, 102.0, 104.0, 101.0, 103.0, 20.0),
            create_test_kline(180000, 103.0, 105.0, 102.0, 104.0, 25.0),
            create_test_kline(240000, 104.0, 106.0, 103.0, 105.0, 30.0),
        ];

        let result = aggregate_minutes(klines_1m, 5);

        assert_eq!(result.len(), 1);
        assert!((result[0].open - 100.0).abs() < 0.01); // First's open
        assert!((result[0].high - 106.0).abs() < 0.01); // Max high
        assert!((result[0].low - 99.0).abs() < 0.01); // Min low
        assert!((result[0].close - 105.0).abs() < 0.01); // Last's close
        assert!((result[0].volume - 100.0).abs() < 0.01); // Sum volume
    }

    fn create_test_kline(
        time: u64,
        open: f32,
        high: f32,
        low: f32,
        close: f32,
        volume: f32,
    ) -> Kline {
        Kline {
            time,
            open,
            high,
            low,
            close,
            volume,
            buy_volume: volume * 0.5,
            sell_volume: volume * 0.5,
        }
    }
}
