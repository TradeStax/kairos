//! Databento fetch orchestration with unified [`CacheStore`].
//!
//! [`DatabentoAdapter`] is the main entry point. It wraps an API client,
//! a [`CacheStore`], and the fetch/cache orchestration logic for trades
//! and depth data.

mod depth;
mod download;
mod gaps;
mod trades;

use std::sync::Arc;

use databento::HistoricalClient;

use super::{DatabentoConfig, DatabentoError, client};
use crate::cache::store::{CacheProvider, CacheStore};
use crate::domain::index::DataIndex;
use crate::domain::types::FeedId;

pub(super) use gaps::find_uncached_gaps;

/// Main Databento adapter — wraps an API client and a unified cache store.
///
/// All fetch methods follow a cache-first strategy: cached days are served
/// directly, and only uncached date gaps are fetched from the Databento API.
pub struct DatabentoAdapter {
    /// Authenticated Databento historical API client
    pub(crate) client: HistoricalClient,
    /// Shared cache store for persisting decoded market data
    pub(crate) cache: Arc<CacheStore>,
    /// Configuration (dataset, cache dir, cost warnings, etc.)
    pub(crate) config: DatabentoConfig,
}

impl DatabentoAdapter {
    /// Creates a new adapter, initializing the API client and cache store
    pub async fn new(config: DatabentoConfig) -> Result<Self, DatabentoError> {
        let client = client::create_historical_client(&config)?;

        let cache = Arc::new(CacheStore::new(config.cache_dir.clone()));
        cache
            .init()
            .await
            .map_err(|e| DatabentoError::Cache(e.to_string()))?;

        log::info!(
            "DatabentoAdapter initialized, cache at {:?}",
            config.cache_dir
        );

        Ok(Self {
            client,
            cache,
            config,
        })
    }

    /// Scans the cache and builds a [`DataIndex`] for the given feed
    pub async fn scan_cache(&self, feed_id: FeedId) -> DataIndex {
        self.cache
            .scan_to_index(CacheProvider::Databento, feed_id)
            .await
    }

    /// Returns aggregate cache statistics (entry count, total size, etc.)
    pub async fn cache_stats(&self) -> crate::cache::CacheStats {
        self.cache.stats().await
    }

    /// Evicts cache entries older than [`DatabentoConfig::cache_max_days`].
    ///
    /// Returns the number of entries removed.
    pub async fn evict_old(&self) -> usize {
        self.cache
            .evict_old(CacheProvider::Databento, self.config.cache_max_days)
            .await
    }

    /// Returns the list of dates with cached trade data for a symbol
    pub async fn cached_trade_dates(&self, symbol: &str) -> Vec<chrono::NaiveDate> {
        self.cache
            .list_dates(
                CacheProvider::Databento,
                symbol,
                crate::cache::store::CacheSchema::Trades,
            )
            .await
            .into_iter()
            .collect()
    }
}
