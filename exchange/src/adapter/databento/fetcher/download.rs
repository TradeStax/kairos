//! Single-day fetch and cache operations

use super::HistoricalDataManager;
use crate::adapter::databento::mapper::convert_databento_price;
use crate::adapter::databento::{DatabentoError, mapper::determine_stype};
use crate::{Kline, Trade};
use databento::{
    dbn::{Mbp10Msg, OhlcvMsg, Schema, TradeMsg, decode::AsyncDbnDecoder},
    historical::timeseries::GetRangeToFileParams,
};
use std::path::PathBuf;
use time::OffsetDateTime;

/// Maximum number of retries for failed API requests
const FETCH_MAX_RETRIES: u32 = 3;

impl HistoricalDataManager {
    /// Fetch a gap range and cache each day with DETAILED PROGRESS
    pub(super) async fn fetch_and_cache_range(
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

        log::debug!("Fetching {} days from databento...", total_gap_days);

        while current <= end_date {
            day_num += 1;
            match self.fetch_to_cache(symbol, schema, current).await {
                Ok(_) => {
                    days_saved += 1;
                    log::debug!(
                        "  Day {}/{}: {} cached successfully",
                        day_num,
                        total_gap_days,
                        current
                    );
                }
                Err(e) => {
                    log::error!(
                        "  FAILED: Day {}/{}: {} failed - {:?}",
                        day_num,
                        total_gap_days,
                        current,
                        e
                    );
                }
            }
            current += chrono::Duration::days(1);
        }

        log::debug!(
            "Gap fetch complete: {}/{} days successfully cached",
            days_saved,
            total_gap_days
        );
        Ok(days_saved)
    }

    /// Fetch data to a file and cache it (RECOMMENDED WORKFLOW)
    ///
    /// Downloads full day of data and caches locally. This is the most cost-effective approach:
    /// - Download once, use many times
    /// - No repeated API calls
    /// - Historical data doesn't change
    ///
    /// Based on programatic-batch.rs example
    pub(crate) async fn fetch_to_cache(
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
        if super::super::DatabentoConfig::is_expensive_schema(schema) {
            log::error!(
                "COST WARNING: Downloading {:?} is VERY expensive! \
                 Consider using MBP-10 instead of MBO.",
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
        let start_naive = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| {
                DatabentoError::Config(format!("Invalid start date: {}", date))
            })?;
        let start_time = OffsetDateTime::from_unix_timestamp(
            chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                start_naive,
                chrono::Utc,
            )
            .timestamp(),
        )
        .map_err(|e| DatabentoError::Config(format!("Invalid start time: {}", e)))?;

        let end_naive = (date + chrono::Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| {
                DatabentoError::Config(format!("Invalid end date: {}", date))
            })?;
        let end_time = OffsetDateTime::from_unix_timestamp(
            chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                end_naive,
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
        let mut last_err = None;
        for attempt in 0..FETCH_MAX_RETRIES {
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

                    if is_retriable && attempt + 1 < FETCH_MAX_RETRIES {
                        let delay = 1u64 << attempt; // 1s, 2s
                        log::warn!(
                            "Databento API error (attempt {}/{}), \
                             retrying in {}s: {}",
                            attempt + 1,
                            FETCH_MAX_RETRIES,
                            delay,
                            err_str
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(delay))
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

        log::debug!("Downloaded {} ({:?}) for {}", symbol, schema, date);

        // Move to cache
        self.cache.store(symbol, schema, date, &temp_path).await?;

        // Clean up temp file
        let _ = tokio::fs::remove_file(&temp_path).await;

        // Return cached path
        self.cache
            .get_cached(symbol, schema, date)
            .await
            .ok_or_else(|| {
                DatabentoError::Cache("Failed to retrieve cached file".to_string())
            })
    }

    /// Load a single day of OHLCV from cache
    pub(super) async fn load_day_from_cache(
        &mut self,
        symbol: &str,
        schema: Schema,
        date: chrono::NaiveDate,
    ) -> Result<Vec<Kline>, DatabentoError> {
        let cache_path = self
            .cache
            .get_cached(symbol, schema, date)
            .await
            .ok_or_else(|| {
                DatabentoError::Cache(format!(
                    "No cache for {} on {}",
                    symbol, date
                ))
            })?;

        let mut decoder = AsyncDbnDecoder::from_zstd_file(cache_path).await?;

        let mut klines = Vec::new();

        while let Some(ohlcv) = decoder.decode_record::<OhlcvMsg>().await? {
            let ts_event = ohlcv
                .hd
                .ts_event()
                .ok_or_else(|| {
                    DatabentoError::Config("missing ts_event".to_string())
                })?;
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

    /// Load a single day of trades from cache
    pub(super) async fn load_trades_day_from_cache(
        &mut self,
        symbol: &str,
        date: chrono::NaiveDate,
    ) -> Result<Vec<Trade>, DatabentoError> {
        let cache_path = self
            .cache
            .get_cached(symbol, Schema::Trades, date)
            .await
            .ok_or_else(|| {
                DatabentoError::Cache(format!(
                    "No cache for {} trades on {}",
                    symbol, date
                ))
            })?;

        let mut decoder = AsyncDbnDecoder::from_zstd_file(cache_path).await?;

        let mut trades = Vec::new();

        while let Some(trade_msg) = decoder.decode_record::<TradeMsg>().await? {
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

        Ok(trades)
    }

    /// Load a single day of MBP-10 depth from cache
    pub(super) async fn load_mbp10_day_from_cache(
        &mut self,
        symbol: &str,
        date: chrono::NaiveDate,
    ) -> Result<Vec<(u64, crate::types::Depth)>, DatabentoError> {
        let cache_path = self
            .cache
            .get_cached(symbol, Schema::Mbp10, date)
            .await
            .ok_or_else(|| {
                DatabentoError::Cache(format!(
                    "No cache for {} MBP-10 on {}",
                    symbol, date
                ))
            })?;

        let mut decoder = AsyncDbnDecoder::from_zstd_file(cache_path).await?;

        let mut snapshots = Vec::new();

        while let Some(mbp) = decoder.decode_record::<Mbp10Msg>().await? {
            let ts_recv = mbp
                .ts_recv()
                .ok_or_else(|| {
                    DatabentoError::Config("missing ts_recv".to_string())
                })?;
            let time_ms = (ts_recv.unix_timestamp_nanos() / 1_000_000) as u64;

            let mut depth = crate::types::Depth::new(time_ms);

            // Add bid levels
            for level in &mbp.levels {
                if level.bid_px != databento::dbn::UNDEF_PRICE && level.bid_sz > 0
                {
                    depth.bids.insert(
                        convert_databento_price(level.bid_px).units(),
                        level.bid_sz as f32,
                    );
                }
            }

            // Add ask levels
            for level in &mbp.levels {
                if level.ask_px != databento::dbn::UNDEF_PRICE && level.ask_sz > 0
                {
                    depth.asks.insert(
                        convert_databento_price(level.ask_px).units(),
                        level.ask_sz as f32,
                    );
                }
            }

            snapshots.push((time_ms, depth));
        }

        Ok(snapshots)
    }
}
