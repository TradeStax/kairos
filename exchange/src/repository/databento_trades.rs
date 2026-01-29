//! Databento Trade Repository Implementation
//!
//! Implements TradeRepository using Databento's HistoricalDataManager.
//! Provides automatic per-day caching, gap detection, and batch fetching.

use crate::adapter::databento::{DatabentoConfig, HistoricalDataManager};
use crate::types::TradeSide;
use chrono::NaiveDate;
use databento::dbn::Schema;
use databento::historical::metadata::GetCostParams;
use flowsurface_data::domain::chart::{DataSchema, LoadingStatus};
use flowsurface_data::domain::{DateRange, FuturesTicker, Price, Quantity, Side, Timestamp, Trade};
use flowsurface_data::repository::{
    RepositoryError, RepositoryResult, RepositoryStats, TradeRepository,
};
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::sync::Mutex;

/// Databento trade repository
///
/// Wraps HistoricalDataManager to implement the repository pattern.
/// Handles per-day caching and automatic gap detection.
pub struct DatabentoTradeRepository {
    manager: Arc<Mutex<HistoricalDataManager>>,
}

impl DatabentoTradeRepository {
    /// Create a new Databento trade repository
    pub async fn new(config: DatabentoConfig) -> Result<Self, RepositoryError> {
        let manager = HistoricalDataManager::new(config)
            .await
            .map_err(|e| RepositoryError::Remote(format!("Failed to create manager: {:?}", e)))?;

        Ok(Self {
            manager: Arc::new(Mutex::new(manager)),
        })
    }

    /// Create from existing manager (for testing)
    pub fn from_manager(manager: HistoricalDataManager) -> Self {
        Self {
            manager: Arc::new(Mutex::new(manager)),
        }
    }

    /// Convert exchange::types::Trade to domain::Trade
    fn convert_trade(trade: &crate::types::Trade) -> Trade {
        Trade {
            time: Timestamp(trade.time),
            price: Price::from_f32(trade.price),
            quantity: Quantity(trade.qty as f64),
            side: match trade.side {
                TradeSide::Buy => Side::Buy,
                TradeSide::Sell => Side::Sell,
            },
        }
    }
}

#[async_trait::async_trait]
impl TradeRepository for DatabentoTradeRepository {
    async fn get_trades(
        &self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<Trade>> {
        let mut manager = self.manager.lock().await;

        // Convert DateRange to chrono DateTime range
        let start = date_range
            .start
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| RepositoryError::InvalidData("Invalid start date".to_string()))?
            .and_utc();
        let end = date_range
            .end
            .and_hms_opt(23, 59, 59)
            .ok_or_else(|| RepositoryError::InvalidData("Invalid end date".to_string()))?
            .and_utc();

        let symbol = ticker.as_str();

        log::info!(
            "DatabentoTradeRepository: Fetching trades for {} from {} to {}",
            symbol,
            date_range.start,
            date_range.end
        );

        // Fetch using manager's cached fetch method
        let exchange_trades = manager
            .fetch_trades_cached(symbol, (start, end))
            .await
            .map_err(|e| RepositoryError::Remote(format!("Databento fetch failed: {:?}", e)))?;

        // Convert to domain trades
        let domain_trades: Vec<Trade> = exchange_trades.iter().map(Self::convert_trade).collect();

        log::info!(
            "DatabentoTradeRepository: Converted {} trades to domain",
            domain_trades.len()
        );

        Ok(domain_trades)
    }

    async fn get_trades_with_progress(
        &self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
        progress_callback: Box<dyn Fn(LoadingStatus) + Send + Sync>,
    ) -> RepositoryResult<Vec<Trade>> {
        let mut manager = self.manager.lock().await;

        // Convert DateRange to chrono DateTime range
        let start = date_range
            .start
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| RepositoryError::InvalidData("Invalid start date".to_string()))?
            .and_utc();
        let end = date_range
            .end
            .and_hms_opt(23, 59, 59)
            .ok_or_else(|| RepositoryError::InvalidData("Invalid end date".to_string()))?
            .and_utc();

        let symbol = ticker.as_str();
        let total_days = date_range.num_days() as usize;

        log::info!(
            "DatabentoTradeRepository: Fetching trades WITH PROGRESS for {} from {} to {}",
            symbol,
            date_range.start,
            date_range.end
        );

        // Initial progress callback
        progress_callback(LoadingStatus::Downloading {
            schema: DataSchema::Trades,
            days_total: total_days,
            days_complete: 0,
            current_day: date_range.start.to_string(),
        });

        // Fetch using manager's cached fetch method WITH progress callback
        let exchange_trades = manager
            .fetch_trades_cached_with_progress(symbol, (start, end), |days_complete, days_total, current_day, from_cache| {
                let status = if from_cache {
                    LoadingStatus::LoadingFromCache {
                        schema: DataSchema::Trades,
                        days_total,
                        days_loaded: days_complete,
                        items_loaded: 0, // We don't track individual items during loading
                    }
                } else {
                    LoadingStatus::Downloading {
                        schema: DataSchema::Trades,
                        days_total,
                        days_complete,
                        current_day: current_day.to_string(),
                    }
                };
                progress_callback(status);
            })
            .await
            .map_err(|e| RepositoryError::Remote(format!("Databento fetch failed: {:?}", e)))?;

        // Convert to domain trades
        let domain_trades: Vec<Trade> = exchange_trades.iter().map(Self::convert_trade).collect();

        log::info!(
            "DatabentoTradeRepository: Converted {} trades to domain (with progress)",
            domain_trades.len()
        );

        Ok(domain_trades)
    }

    async fn has_trades(&self, ticker: &FuturesTicker, date: NaiveDate) -> RepositoryResult<bool> {
        let manager = self.manager.lock().await;
        let symbol = ticker.as_str();

        // Check if cache has this day
        let has_cached = manager
            .cache
            .has_cached(symbol, databento::dbn::Schema::Trades, date)
            .await;

        Ok(has_cached)
    }

    async fn get_trades_for_date(
        &self,
        ticker: &FuturesTicker,
        date: NaiveDate,
    ) -> RepositoryResult<Vec<Trade>> {
        let date_range = DateRange::new(date, date);
        self.get_trades(ticker, &date_range).await
    }

    async fn store_trades(
        &self,
        _ticker: &FuturesTicker,
        _date: NaiveDate,
        _trades: Vec<Trade>,
    ) -> RepositoryResult<()> {
        // Storage is handled automatically by HistoricalDataManager during fetch
        Ok(())
    }

    async fn find_gaps(
        &self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<DateRange>> {
        let manager = self.manager.lock().await;
        let symbol = ticker.as_str();

        let mut gaps = Vec::new();
        let mut current = date_range.start;

        while current <= date_range.end {
            if !manager
                .cache
                .has_cached(symbol, databento::dbn::Schema::Trades, current)
                .await
            {
                // Start of a new gap
                let gap_start = current;
                let mut gap_end = current;

                // Find consecutive uncached days
                while gap_end <= date_range.end
                    && !manager
                        .cache
                        .has_cached(symbol, databento::dbn::Schema::Trades, gap_end)
                        .await
                {
                    gap_end += chrono::Duration::days(1);
                }

                gap_end -= chrono::Duration::days(1);
                gaps.push(DateRange::new(gap_start, gap_end));
                current = gap_end + chrono::Duration::days(1);
            } else {
                current += chrono::Duration::days(1);
            }
        }

        Ok(gaps)
    }

    async fn stats(&self) -> RepositoryResult<RepositoryStats> {
        // Return default stats - cache statistics can be queried separately via HistoricalDataManager
        Ok(RepositoryStats::new())
    }

    async fn check_cache_coverage_databento(
        &self,
        ticker: &FuturesTicker,
        schema_discriminant: u16,
        date_range: &DateRange,
    ) -> RepositoryResult<flowsurface_data::repository::CacheCoverageReport> {
        // Convert discriminant back to Schema
        let schema = unsafe { std::mem::transmute::<u16, databento::dbn::Schema>(schema_discriminant) };
        let manager = self.manager.lock().await;
        let symbol = ticker.as_str();

        let mut cached_days = Vec::new();
        let mut uncached_days = Vec::new();

        // Check each day in the range
        for date in date_range.dates() {
            if manager.cache.has_cached(symbol, schema, date).await {
                cached_days.push(date);
            } else {
                uncached_days.push(date);
            }
        }

        // Find consecutive gaps
        let mut gaps = Vec::new();
        if !uncached_days.is_empty() {
            let mut gap_start = uncached_days[0];
            let mut gap_end = uncached_days[0];

            for (i, &date) in uncached_days.iter().enumerate().skip(1) {
                if date == gap_end + chrono::Duration::days(1) {
                    // Extend current gap
                    gap_end = date;
                } else {
                    // Save previous gap and start new one
                    gaps.push((gap_start, gap_end));
                    gap_start = date;
                    gap_end = date;
                }

                // Handle last gap
                if i == uncached_days.len() - 1 {
                    gaps.push((gap_start, gap_end));
                }
            }

            // Handle single uncached day
            if uncached_days.len() == 1 {
                gaps.push((gap_start, gap_end));
            }
        }

        Ok(flowsurface_data::repository::CacheCoverageReport {
            cached_count: cached_days.len(),
            uncached_count: uncached_days.len(),
            gaps,
            cached_dates: cached_days, // Return list of cached dates
        })
    }

    async fn prefetch_to_cache_databento(
        &self,
        ticker: &FuturesTicker,
        schema_discriminant: u16,
        date_range: &DateRange,
    ) -> RepositoryResult<usize> {
        // Convert discriminant back to Schema
        let schema = unsafe { std::mem::transmute::<u16, databento::dbn::Schema>(schema_discriminant) };
        let mut manager = self.manager.lock().await;
        let symbol = ticker.as_str();

        let mut downloaded = 0;

        // Download each uncached day
        for date in date_range.dates() {
            if !manager.cache.has_cached(symbol, schema, date).await {
                log::info!("Downloading {} for {} (schema: {:?})", date, symbol, schema);

                manager
                    .fetch_to_cache(symbol, schema, date)
                    .await
                    .map_err(|e| RepositoryError::Remote(format!("Download failed for {}: {:?}", date, e)))?;

                downloaded += 1;
                log::info!("Successfully cached {}/{} for {}", date, schema, symbol);
            }
        }

        log::info!(
            "Prefetch complete: Downloaded {} days for {} ({:?})",
            downloaded,
            symbol,
            schema
        );

        Ok(downloaded)
    }

    async fn prefetch_to_cache_databento_with_progress(
        &self,
        ticker: &FuturesTicker,
        schema_discriminant: u16,
        date_range: &DateRange,
        progress_callback: Box<dyn Fn(usize, usize) + Send + Sync>,
    ) -> RepositoryResult<usize> {
        // Convert discriminant back to Schema
        let schema = unsafe { std::mem::transmute::<u16, databento::dbn::Schema>(schema_discriminant) };
        let mut manager = self.manager.lock().await;
        let symbol = ticker.as_str();

        let total_days = date_range.num_days() as usize;
        let mut downloaded = 0;
        let mut processed = 0;

        log::info!("Starting prefetch with progress: {} days for {} ({:?})", total_days, symbol, schema);

        // Download each day (including already cached - for accurate progress)
        for date in date_range.dates() {
            if !manager.cache.has_cached(symbol, schema, date).await {
                log::info!("Downloading {} for {} (schema: {:?})", date, symbol, schema);

                manager
                    .fetch_to_cache(symbol, schema, date)
                    .await
                    .map_err(|e| RepositoryError::Remote(format!("Download failed for {}: {:?}", date, e)))?;

                downloaded += 1;
                log::info!("Successfully cached {}/{} for {}", date, schema, symbol);
            } else {
                log::debug!("Skipping {} - already cached", date);
            }

            // Update progress after each day (downloaded or skipped)
            processed += 1;
            progress_callback(processed, total_days);
        }

        log::info!(
            "Prefetch complete: Downloaded {} days for {} ({:?})",
            downloaded,
            symbol,
            schema
        );

        Ok(downloaded)
    }

    async fn get_actual_cost_databento(
        &self,
        ticker: &FuturesTicker,
        schema_discriminant: u16,
        date_range: &DateRange,
    ) -> RepositoryResult<f64> {
        log::warn!("REPOSITORY: get_actual_cost_databento CALLED for {:?}", ticker);

        // Convert discriminant back to Schema
        let schema = unsafe { std::mem::transmute::<u16, Schema>(schema_discriminant) };
        let mut manager = self.manager.lock().await;
        let symbol = ticker.as_str();

        log::warn!("REPOSITORY: About to call Databento API with symbol={}, schema={:?}", symbol, schema);

        // Convert DateRange to chrono DateTime (UTC start/end of day)
        // NOTE: Databento API uses exclusive end times, so end = (end_date + 1 day) at 00:00:00
        let start = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            date_range.start.and_hms_opt(0, 0, 0)
                .ok_or_else(|| RepositoryError::InvalidData("Invalid start date".to_string()))?,
            chrono::Utc,
        );
        let end = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            (date_range.end + chrono::Duration::days(1))
                .and_hms_opt(0, 0, 0)
                .ok_or_else(|| RepositoryError::InvalidData("Invalid end date".to_string()))?,
            chrono::Utc,
        );

        // Convert to time::OffsetDateTime for Databento API
        let start_time = OffsetDateTime::from_unix_timestamp(start.timestamp())
            .map_err(|e| RepositoryError::InvalidData(format!("Invalid start time: {}", e)))?;
        let end_time = OffsetDateTime::from_unix_timestamp(end.timestamp())
            .map_err(|e| RepositoryError::InvalidData(format!("Invalid end time: {}", e)))?;

        // Determine symbol type (continuous, parent, or raw)
        let stype = crate::adapter::databento::mapper::determine_stype(symbol);

        // Build cost request parameters
        let cost_params = GetCostParams::builder()
            .dataset(manager.config.dataset)
            .symbols(vec![symbol])
            .schema(schema)
            .stype_in(stype)
            .date_time_range((start_time, end_time))
            .build();

        // Call real Databento cost API
        log::info!(
            "Calling Databento cost API: symbol={}, schema={:?}, range={:?} to {:?}",
            symbol, schema, date_range.start, date_range.end
        );
        log::debug!("Cost params: dataset={:?}, symbols=[{}], date_time_range={:?} to {:?}",
            manager.config.dataset, symbol, start_time, end_time);

        let cost_result = manager
            .client
            .metadata()
            .get_cost(&cost_params)
            .await;

        match cost_result {
            Ok(cost_usd) => {
                log::info!("Databento cost API SUCCESS: ${:.4} USD for {} from {} to {}",
                    cost_usd, symbol, date_range.start, date_range.end);
                Ok(cost_usd)
            }
            Err(e) => {
                log::error!("Databento cost API FAILED: {:?}", e);
                log::error!("  Symbol: {}", symbol);
                log::error!("  Schema: {:?}", schema);
                log::error!("  Dataset: {:?}", manager.config.dataset);
                log::error!("  Date range: {} to {}", date_range.start, date_range.end);
                Err(RepositoryError::Remote(format!("Databento cost API failed: {:?}", e)))
            }
        }
    }

    async fn list_cached_symbols_databento(&self) -> RepositoryResult<std::collections::HashSet<String>> {
        let manager = self.manager.lock().await;
        manager.cache.list_cached_symbols()
            .await
            .map_err(|e| RepositoryError::Cache(format!("Failed to list cached symbols: {:?}", e)))
    }
}

// Default trait intentionally not implemented
// DatabentoTradeRepository requires explicit initialization with configuration
// via new() or from_manager(). This prevents accidental creation of invalid instances.
