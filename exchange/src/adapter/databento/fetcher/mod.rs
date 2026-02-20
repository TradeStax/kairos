//! Historical market data fetching with SMART per-day caching
//!
//! Caching Strategy:
//! - Saves data PER DAY for fine-grained reuse
//! - Fetches GAPS IN BATCHES to minimize API calls
//! - Example: Need 5/6-5/18, have 5/10-5/13 + 5/16 cached
//!   -> Makes 3 API calls: [5/6-5/9], [5/14-5/15], [5/17-5/18]
//!   -> Saves each day individually for future reuse

mod aggregation;
mod download;
mod gaps;
mod manager;

use super::{DatabentoConfig, DatabentoError, cache::CacheManager, client};
use databento::HistoricalClient;

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

        log::debug!(
            "HistoricalDataManager initialized with smart per-day caching"
        );

        Ok(Self {
            client,
            cache,
            config,
        })
    }

    /// Get cache statistics
    pub async fn cache_stats(
        &self,
    ) -> Result<super::cache::CacheStats, DatabentoError> {
        self.cache.stats().await
    }

    /// Get cached date ranges for a symbol
    pub async fn get_cached_date_ranges(
        &self,
        symbol: &str,
    ) -> Result<Vec<chrono::NaiveDate>, DatabentoError> {
        use tokio::fs;

        let safe_symbol = symbol.replace('.', "-");
        let symbol_dir = self.cache.cache_root.join(&safe_symbol);

        let mut dates = Vec::new();

        // Check if symbol directory exists
        if !symbol_dir.exists() {
            return Ok(dates);
        }

        // Walk through schema directories
        let mut schema_entries =
            fs::read_dir(&symbol_dir).await.map_err(|e| {
                DatabentoError::Cache(format!(
                    "Failed to read symbol cache dir: {}",
                    e
                ))
            })?;

        while let Some(schema_entry) = schema_entries
            .next_entry()
            .await
            .map_err(|e| {
                DatabentoError::Cache(format!(
                    "Failed to read schema entry: {}",
                    e
                ))
            })?
        {
            if !schema_entry
                .file_type()
                .await
                .map(|t| t.is_dir())
                .unwrap_or(false)
            {
                continue;
            }

            // Read date files in schema directory
            let mut date_entries = fs::read_dir(schema_entry.path())
                .await
                .map_err(|e| {
                    DatabentoError::Cache(format!(
                        "Failed to read schema dir: {}",
                        e
                    ))
                })?;

            while let Some(date_entry) = date_entries
                .next_entry()
                .await
                .map_err(|e| {
                    DatabentoError::Cache(format!(
                        "Failed to read date entry: {}",
                        e
                    ))
                })?
            {
                // Parse date from filename (format: YYYY-MM-DD.dbn.zst)
                if let Some(filename) = date_entry.file_name().to_str()
                    && let Some(date_str) =
                        filename.strip_suffix(".dbn.zst")
                    && let Ok(date) = chrono::NaiveDate::parse_from_str(
                        date_str, "%Y-%m-%d",
                    )
                {
                    dates.push(date);
                }
            }
        }

        // Sort and deduplicate
        dates.sort();
        dates.dedup();

        Ok(dates)
    }

    /// Clean up old cache files
    pub async fn cleanup_cache(&self) -> Result<usize, DatabentoError> {
        self.cache.cleanup_old_files().await
    }
}
