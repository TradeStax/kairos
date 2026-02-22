//! Databento Trade Repository Implementation
//!
//! Implements TradeRepository using Databento's HistoricalDataManager.
//! Provides automatic per-day caching, gap detection, and batch fetching.
//!
//! The DownloadRepository impl lives in `download.rs`.

use super::find_cache_gaps;
use super::mapper;
use crate::adapter::databento::{DatabentoConfig, HistoricalDataManager, cache::CacheManager};
use chrono::NaiveDate;
use databento::dbn::Schema;
use kairos_data::domain::chart::{DataSchema, LoadingStatus};
use kairos_data::domain::{DateRange, FuturesTicker, Trade};
use kairos_data::repository::{
    RepositoryError, RepositoryResult, RepositoryStats, TradeRepository,
};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Safely convert a u16 schema discriminant to a databento Schema.
pub(super) fn schema_from_discriminant(discriminant: u16) -> Result<Schema, RepositoryError> {
    Schema::try_from(discriminant).map_err(|_| {
        RepositoryError::InvalidData(format!("Invalid schema discriminant: {}", discriminant))
    })
}

/// Databento trade repository
///
/// Wraps HistoricalDataManager to implement the repository pattern.
/// Handles per-day caching and automatic gap detection.
///
/// The `cache` field is an `Arc<CacheManager>` shared with the manager.
/// Read-only operations (cache checks, gap detection) use it directly without
/// acquiring the manager lock, so they never block concurrent downloads.
pub struct DatabentoTradeRepository {
    pub(super) manager: Arc<Mutex<HistoricalDataManager>>,
    /// Shared cache reference — accessed without the manager lock for read-only checks.
    pub(super) cache: Arc<CacheManager>,
}

impl DatabentoTradeRepository {
    /// Create a new Databento trade repository
    pub async fn new(config: DatabentoConfig) -> Result<Self, RepositoryError> {
        let manager = HistoricalDataManager::new(config)
            .await
            .map_err(|e| RepositoryError::Remote(format!("Failed to create manager: {:?}", e)))?;

        let cache = manager.cache();
        Ok(Self {
            manager: Arc::new(Mutex::new(manager)),
            cache,
        })
    }

    /// Create from existing manager (for testing)
    pub fn from_manager(manager: HistoricalDataManager) -> Self {
        let cache = manager.cache();
        Self {
            manager: Arc::new(Mutex::new(manager)),
            cache,
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

        let start = mapper::date_range_start_utc(date_range.start)?;
        let end = mapper::date_range_end_utc(date_range.end)?;

        let symbol = ticker.as_str();

        log::debug!(
            "Fetching trades for {} from {} to {}",
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
        let domain_trades: Vec<Trade> = exchange_trades.iter().map(mapper::convert_trade).collect();

        log::debug!("Converted {} trades to domain", domain_trades.len());

        Ok(domain_trades)
    }

    async fn get_trades_with_progress(
        &self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
        progress_callback: Box<dyn Fn(LoadingStatus) + Send + Sync>,
    ) -> RepositoryResult<Vec<Trade>> {
        let mut manager = self.manager.lock().await;

        let start = mapper::date_range_start_utc(date_range.start)?;
        let end = mapper::date_range_end_utc(date_range.end)?;

        let symbol = ticker.as_str();
        let total_days = date_range.num_days() as usize;

        log::debug!(
            "Fetching trades with progress for {} from {} to {}",
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
            .fetch_trades_cached_with_progress(
                symbol,
                (start, end),
                |days_complete, days_total, current_day, from_cache| {
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
                },
            )
            .await
            .map_err(|e| RepositoryError::Remote(format!("Databento fetch failed: {:?}", e)))?;

        // Convert to domain trades
        let domain_trades: Vec<Trade> = exchange_trades.iter().map(mapper::convert_trade).collect();

        log::debug!(
            "Converted {} trades to domain (with progress)",
            domain_trades.len()
        );

        Ok(domain_trades)
    }

    /// Check cache without acquiring the manager lock.
    async fn has_trades(&self, ticker: &FuturesTicker, date: NaiveDate) -> RepositoryResult<bool> {
        let symbol = ticker.as_str();
        let has_cached = self.cache.has_cached(symbol, Schema::Trades, date).await;
        Ok(has_cached)
    }

    async fn get_trades_for_date(
        &self,
        ticker: &FuturesTicker,
        date: NaiveDate,
    ) -> RepositoryResult<Vec<Trade>> {
        let date_range = DateRange::new(date, date)
            .expect("invariant: same date for start and end");
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

    /// Detect cache gaps without acquiring the manager lock.
    async fn find_gaps(
        &self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<DateRange>> {
        find_cache_gaps(&self.cache, ticker.as_str(), Schema::Trades, date_range).await
    }

    async fn stats(&self) -> RepositoryResult<RepositoryStats> {
        // Return default stats - cache statistics can be queried separately via HistoricalDataManager
        Ok(RepositoryStats::new())
    }
}

// Default trait intentionally not implemented
// DatabentoTradeRepository requires explicit initialization with configuration
// via new() or from_manager(). This prevents accidental creation of invalid instances.
