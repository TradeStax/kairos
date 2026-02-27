//! Single-day fetch-to-cache operations.
//!
//! Downloads a day of data from the Databento API to a temporary `.dbn.zst`
//! file, decodes the records into domain types, and persists them to the
//! unified [`CacheStore`]. Includes retry logic for transient HTTP errors.

use databento::dbn::{Mbp10Msg, Schema, TradeMsg, decode::AsyncDbnDecoder};
use databento::historical::timeseries::GetRangeToFileParams;
use time::OffsetDateTime;

use super::DatabentoAdapter;
use crate::adapter::databento::{DatabentoError, mapper::determine_stype};
use crate::cache::store::{CacheProvider, CacheSchema};
use crate::domain::{Depth, Trade};

/// Maximum number of retry attempts for transient API failures
const FETCH_MAX_RETRIES: u32 = 3;

impl DatabentoAdapter {
    /// Fetches a single day's data to a temp `.dbn.zst` file, decodes it,
    /// and writes the domain objects to the unified cache.
    ///
    /// Returns early if the day is already cached. Supports trades and
    /// MBP-10 depth schemas; other schemas are logged as warnings.
    pub(crate) async fn fetch_to_unified_cache(
        &mut self,
        symbol: &str,
        schema: Schema,
        date: chrono::NaiveDate,
    ) -> Result<(), DatabentoError> {
        let cache_schema = match schema {
            Schema::Trades => CacheSchema::Trades,
            Schema::Mbp10 => CacheSchema::Depth,
            Schema::Ohlcv1M => CacheSchema::Ohlcv,
            _ => CacheSchema::Trades,
        };

        if self
            .cache
            .has_day(CacheProvider::Databento, symbol, cache_schema, date)
            .await
        {
            log::debug!("Cache HIT: {} {:?} {}", symbol, schema, date);
            return Ok(());
        }

        if DatabentoAdapter::is_expensive_schema(schema) {
            log::error!("COST WARNING: Fetching {:?} is very expensive!", schema);
        }

        let temp_path = std::env::temp_dir().join(format!(
            "databento_{}_{}_{}.dbn.zst",
            symbol.replace('.', "-"),
            format!("{:?}", schema).to_lowercase(),
            date.format("%Y%m%d")
        ));

        let start_naive = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| DatabentoError::Config(format!("Invalid date: {}", date)))?;
        let start_time = OffsetDateTime::from_unix_timestamp(
            chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(start_naive, chrono::Utc)
                .timestamp(),
        )
        .map_err(|e| DatabentoError::Config(e.to_string()))?;

        let end_naive = (date + chrono::Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| DatabentoError::Config("Invalid end date".to_string()))?;
        let end_time = OffsetDateTime::from_unix_timestamp(
            chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(end_naive, chrono::Utc)
                .timestamp(),
        )
        .map_err(|e| DatabentoError::Config(e.to_string()))?;

        let params = GetRangeToFileParams::builder()
            .dataset(self.config.dataset)
            .schema(schema)
            .symbols(vec![symbol])
            .stype_in(determine_stype(symbol))
            .date_time_range((start_time, end_time))
            .path(&temp_path)
            .build();

        // Download with retries for transient HTTP errors
        let mut last_err = None;
        for attempt in 0..FETCH_MAX_RETRIES {
            match self.client.timeseries().get_range_to_file(&params).await {
                Ok(_) => {
                    last_err = None;
                    break;
                }
                Err(e) => {
                    let is_retriable = matches!(e, databento::Error::Http(_));
                    if is_retriable && attempt < FETCH_MAX_RETRIES - 1 {
                        log::warn!(
                            "Download attempt {} failed: {}. Retrying...",
                            attempt + 1,
                            e
                        );
                    } else {
                        last_err = Some(e);
                        break;
                    }
                }
            }
        }

        if let Some(e) = last_err {
            return Err(DatabentoError::Api(e));
        }

        // Decode the .dbn.zst file and write to unified cache
        match schema {
            Schema::Trades => {
                self.decode_and_cache_trades(symbol, date, &temp_path)
                    .await?;
            }
            Schema::Mbp10 => {
                self.decode_and_cache_depth(symbol, date, &temp_path)
                    .await?;
            }
            _ => {
                log::warn!("Unsupported schema {:?} for unified cache", schema);
            }
        }

        // Clean up temp file
        let _ = tokio::fs::remove_file(&temp_path).await;

        Ok(())
    }

    /// Decodes trade records from a temp file and writes them to the cache
    async fn decode_and_cache_trades(
        &self,
        symbol: &str,
        date: chrono::NaiveDate,
        temp_path: &std::path::Path,
    ) -> Result<(), DatabentoError> {
        let mut decoder = AsyncDbnDecoder::from_zstd_file(temp_path).await?;
        let mut trades = Vec::new();
        let mut skipped = 0usize;
        while let Some(msg) = decoder.decode_record::<TradeMsg>().await? {
            if let Ok(trade) = crate::adapter::databento::mapper::trade_msg_to_domain(msg) {
                trades.push(trade);
            } else {
                skipped += 1;
            }
        }
        if skipped > 0 {
            log::debug!(
                "Skipped {} unmappable trade messages for {} on {}",
                skipped,
                symbol,
                date
            );
        }
        self.cache
            .write_day(
                CacheProvider::Databento,
                symbol,
                CacheSchema::Trades,
                date,
                &trades,
            )
            .await
            .map_err(|e| DatabentoError::Cache(e.to_string()))?;
        log::info!("Cached {} trades for {} on {}", trades.len(), symbol, date);
        Ok(())
    }

    /// Decodes MBP-10 depth records from a temp file and writes them to the cache
    async fn decode_and_cache_depth(
        &self,
        symbol: &str,
        date: chrono::NaiveDate,
        temp_path: &std::path::Path,
    ) -> Result<(), DatabentoError> {
        let mut decoder = AsyncDbnDecoder::from_zstd_file(temp_path).await?;
        let mut snapshots: Vec<Depth> = Vec::new();
        let mut skipped = 0usize;
        while let Some(msg) = decoder.decode_record::<Mbp10Msg>().await? {
            if let Ok(depth) = crate::adapter::databento::mapper::mbp10_to_domain(msg) {
                snapshots.push(depth);
            } else {
                skipped += 1;
            }
        }
        if skipped > 0 {
            log::debug!(
                "Skipped {} unmappable depth messages for {} on {}",
                skipped,
                symbol,
                date
            );
        }
        self.cache
            .write_day(
                CacheProvider::Databento,
                symbol,
                CacheSchema::Depth,
                date,
                &snapshots,
            )
            .await
            .map_err(|e| DatabentoError::Cache(e.to_string()))?;
        log::info!(
            "Cached {} depth snapshots for {} on {}",
            snapshots.len(),
            symbol,
            date
        );
        Ok(())
    }

    /// Returns `true` if the schema is known to be very expensive (e.g. MBO)
    fn is_expensive_schema(schema: Schema) -> bool {
        matches!(schema, Schema::Mbo)
    }

    /// Loads a single day's trades from the unified cache
    pub(crate) async fn load_trades_day(
        &self,
        symbol: &str,
        date: chrono::NaiveDate,
    ) -> Result<Vec<Trade>, DatabentoError> {
        self.cache
            .read_day::<Trade>(CacheProvider::Databento, symbol, CacheSchema::Trades, date)
            .await
            .map_err(|e| DatabentoError::Cache(e.to_string()))
    }

    /// Loads a single day's depth snapshots from the unified cache
    pub(crate) async fn load_depth_day(
        &self,
        symbol: &str,
        date: chrono::NaiveDate,
    ) -> Result<Vec<Depth>, DatabentoError> {
        self.cache
            .read_day::<Depth>(CacheProvider::Databento, symbol, CacheSchema::Depth, date)
            .await
            .map_err(|e| DatabentoError::Cache(e.to_string()))
    }

    /// Fetches and caches each day in a date range.
    ///
    /// Returns the number of days successfully cached. Individual day
    /// failures are logged but do not abort the overall operation.
    pub(super) async fn fetch_and_cache_range(
        &mut self,
        symbol: &str,
        schema: Schema,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> Result<usize, DatabentoError> {
        let total = (end_date - start_date).num_days() + 1;
        let mut saved = 0;
        let mut current = start_date;
        let mut day_num = 0;

        while current <= end_date {
            day_num += 1;
            match self.fetch_to_unified_cache(symbol, schema, current).await {
                Ok(_) => {
                    saved += 1;
                    log::debug!("Day {}/{}: {} cached", day_num, total, current);
                }
                Err(e) => {
                    log::error!("FAILED day {}/{}: {} — {:?}", day_num, total, current, e);
                }
            }
            current += chrono::Duration::days(1);
        }

        Ok(saved)
    }
}
