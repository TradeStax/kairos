//! Databento Depth Repository Implementation
//!
//! Implements DepthRepository using Databento's HistoricalDataManager.
//! Provides automatic per-day caching of MBP-10 depth data for heatmap visualization.

use super::find_cache_gaps;
use super::mapper;
use crate::adapter::databento::{DatabentoConfig, HistoricalDataManager, cache::CacheManager};
use chrono::NaiveDate;
use kairos_data::domain::{DateRange, DepthSnapshot, FuturesTicker};
use kairos_data::repository::{
    DepthRepository, RepositoryError, RepositoryResult, RepositoryStats,
};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Databento depth repository
///
/// Wraps HistoricalDataManager to implement the repository pattern.
/// Handles per-day caching of MBP-10 depth snapshots.
///
/// The `cache` field is an `Arc<CacheManager>` shared with the manager.
/// Read-only operations (cache checks, gap detection) use it directly without
/// acquiring the manager lock, so they never block concurrent downloads.
pub struct DatabentoDepthRepository {
    manager: Arc<Mutex<HistoricalDataManager>>,
    /// Shared cache reference — accessed without the manager lock for read-only checks.
    cache: Arc<CacheManager>,
}

impl DatabentoDepthRepository {
    /// Create a new Databento depth repository
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
impl DepthRepository for DatabentoDepthRepository {
    async fn get_depth(
        &self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<DepthSnapshot>> {
        let mut manager = self.manager.lock().await;

        let start = mapper::date_range_start_utc(date_range.start)?;
        let end = mapper::date_range_end_utc(date_range.end)?;

        let symbol = ticker.as_str();

        log::debug!(
            "Fetching MBP-10 depth for {} from {} to {}",
            symbol,
            date_range.start,
            date_range.end
        );

        // Fetch using manager's cached fetch method
        let depth_tuples = manager
            .fetch_mbp10_cached(symbol, (start, end))
            .await
            .map_err(|e| RepositoryError::Remote(format!("Databento fetch failed: {:?}", e)))?;

        let original_count = depth_tuples.len();

        // PERFORMANCE FIX: Decimate depth snapshots to prevent 30+ minute freeze
        // MBP-10 can have 100K+ snapshots/day which causes O(millions) BTreeMap operations
        // For heatmap visualization, we only need ~1 snapshot per second
        //
        // Decimation strategy: Keep every Nth snapshot based on data density
        // More aggressive for multi-day loads to ensure <5 minute load times
        const DECIMATION_ULTRA_THRESHOLD: usize = 200_000;
        const DECIMATION_HIGH_THRESHOLD: usize = 100_000;
        const DECIMATION_MEDIUM_THRESHOLD: usize = 50_000;
        const DECIMATION_LOW_THRESHOLD: usize = 10_000;

        let decimation_factor = if original_count > DECIMATION_ULTRA_THRESHOLD {
            50 // 5+ days NQ: keep every 50th snapshot
        } else if original_count > DECIMATION_HIGH_THRESHOLD {
            30 // 3-4 days NQ: keep every 30th snapshot
        } else if original_count > DECIMATION_MEDIUM_THRESHOLD {
            15 // 1-2 days NQ: keep every 15th snapshot
        } else if original_count > DECIMATION_LOW_THRESHOLD {
            5 // Moderate density: keep every 5th snapshot
        } else {
            1 // Low density: keep all snapshots
        };

        let decimated: Vec<_> = if decimation_factor > 1 {
            depth_tuples
                .into_iter()
                .enumerate()
                .filter(|(idx, _)| idx % decimation_factor == 0)
                .map(|(_, snapshot)| snapshot)
                .collect()
        } else {
            depth_tuples
        };

        log::debug!(
            "Decimated MBP-10 snapshots {} -> {} ({}x reduction)",
            original_count,
            decimated.len(),
            decimation_factor
        );

        // Convert to domain depth snapshots
        let domain_snapshots: Vec<DepthSnapshot> = decimated
            .iter()
            .map(|(time, depth)| mapper::convert_depth_snapshot(*time, depth))
            .collect();

        log::debug!(
            "Converted {} depth snapshots to domain",
            domain_snapshots.len()
        );

        Ok(domain_snapshots)
    }

    /// Check cache without acquiring the manager lock.
    async fn has_depth(&self, ticker: &FuturesTicker, date: NaiveDate) -> RepositoryResult<bool> {
        let symbol = ticker.as_str();
        let has_cached = self.cache.has_cached(symbol, databento::dbn::Schema::Mbp10, date).await;
        Ok(has_cached)
    }

    async fn get_depth_for_date(
        &self,
        ticker: &FuturesTicker,
        date: NaiveDate,
    ) -> RepositoryResult<Vec<DepthSnapshot>> {
        let date_range = DateRange::new(date, date)
            .expect("invariant: same date for start and end");
        self.get_depth(ticker, &date_range).await
    }

    async fn store_depth(
        &self,
        _ticker: &FuturesTicker,
        _date: NaiveDate,
        _depth: Vec<DepthSnapshot>,
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
        find_cache_gaps(&self.cache, ticker.as_str(), databento::dbn::Schema::Mbp10, date_range)
            .await
    }

    async fn stats(&self) -> RepositoryResult<RepositoryStats> {
        // Return default stats - cache statistics can be queried separately via HistoricalDataManager
        Ok(RepositoryStats::new())
    }
}

// Default trait intentionally not implemented
// DatabentoDepthRepository requires explicit initialization with configuration
// via new() or from_manager(). This prevents accidental creation of invalid instances.
