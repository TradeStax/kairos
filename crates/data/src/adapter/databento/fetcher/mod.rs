//! Databento fetch orchestration with unified CacheStore
//!
//! `DatabentoAdapter` is the main entry point. It wraps an API client,
//! a `CacheStore`, and the fetch orchestration logic.

mod depth;
mod download;
mod gaps;
mod trades;

use super::{DatabentoConfig, DatabentoError, client};
use crate::cache::store::{CacheProvider, CacheStore};
use crate::domain::index::DataIndex;
use crate::domain::types::FeedId;
use databento::HistoricalClient;
use std::sync::Arc;

pub(super) use gaps::find_uncached_gaps;

/// Main Databento adapter — wraps API client + unified cache
pub struct DatabentoAdapter {
    pub(crate) client: HistoricalClient,
    pub(crate) cache: Arc<CacheStore>,
    pub(crate) config: DatabentoConfig,
}

impl DatabentoAdapter {
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

    /// Scan the cache and build a DataIndex
    pub async fn scan_cache(&self, feed_id: FeedId) -> DataIndex {
        self.cache
            .scan_to_index(CacheProvider::Databento, feed_id)
            .await
    }

    /// Cache statistics
    pub async fn cache_stats(&self) -> crate::cache::CacheStats {
        self.cache.stats().await
    }

    /// Evict old cache entries
    pub async fn evict_old(&self) -> usize {
        self.cache
            .evict_old(CacheProvider::Databento, self.config.cache_max_days)
            .await
    }

    /// Get cached dates for a symbol (trades schema)
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
