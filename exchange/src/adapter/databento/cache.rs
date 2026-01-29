//! Local caching system for Databento historical data
//!
//! This module manages local storage of DBN (Databento Binary) files to minimize
//! API usage and improve performance for frequently accessed data.

use super::{DatabentoConfig, DatabentoError};
use databento::dbn::Schema;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Manages local cache of historical market data
pub struct CacheManager {
    /// Root directory for cache storage
    pub(crate) cache_root: PathBuf,
    /// Maximum age of cached files in days
    max_age_days: u32,
}

impl CacheManager {
    /// Create a new cache manager with default settings
    pub fn new(config: &DatabentoConfig) -> Self {
        Self {
            cache_root: config.cache_dir.clone(),
            max_age_days: config.cache_max_days,
        }
    }

    /// Initialize cache directory structure
    pub async fn init(&self) -> Result<(), DatabentoError> {
        fs::create_dir_all(&self.cache_root)
            .await
            .map_err(|e| DatabentoError::Cache(format!("Failed to create cache dir: {}", e)))?;
        log::info!("Cache initialized at: {:?}", self.cache_root);
        Ok(())
    }

    /// Get path for a cached data file
    pub fn get_cache_path(&self, symbol: &str, schema: Schema, date: chrono::NaiveDate) -> PathBuf {
        let date_str = date.format("%Y-%m-%d").to_string();
        let schema_str = format!("{:?}", schema).to_lowercase();

        // Sanitize symbol for filesystem (replace . with -)
        let safe_symbol = symbol.replace('.', "-");

        self.cache_root
            .join(safe_symbol)
            .join(schema_str)
            .join(format!("{}.dbn.zst", date_str))
    }

    /// Check if data is cached for a specific date
    pub async fn has_cached(&self, symbol: &str, schema: Schema, date: chrono::NaiveDate) -> bool {
        let path = self.get_cache_path(symbol, schema, date);
        // Use async file system check instead of synchronous path.exists()
        tokio::fs::try_exists(&path).await.unwrap_or(false)
    }

    /// Get cached file path if it exists
    pub async fn get_cached(
        &self,
        symbol: &str,
        schema: Schema,
        date: chrono::NaiveDate,
    ) -> Option<PathBuf> {
        let path = self.get_cache_path(symbol, schema, date);
        if path.exists() { Some(path) } else { None }
    }

    /// Store data in cache
    pub async fn store(
        &self,
        symbol: &str,
        schema: Schema,
        date: chrono::NaiveDate,
        data_path: &Path,
    ) -> Result<(), DatabentoError> {
        let cache_path = self.get_cache_path(symbol, schema, date);

        // Create parent directories
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                DatabentoError::Cache(format!("Failed to create cache subdir: {}", e))
            })?;
        }

        // Copy file to cache location
        fs::copy(data_path, &cache_path)
            .await
            .map_err(|e| DatabentoError::Cache(format!("Failed to copy to cache: {}", e)))?;

        log::info!("Cached data for {} ({:?}) on {}", symbol, schema, date);
        Ok(())
    }

    /// Get cache statistics
    pub async fn stats(&self) -> Result<CacheStats, DatabentoError> {
        let mut total_files = 0;
        let mut total_size = 0u64;
        let mut oldest_file: Option<chrono::DateTime<chrono::Utc>> = None;

        // Walk through all cache files recursively
        total_files +=
            Self::count_files_recursive(&self.cache_root, &mut total_size, &mut oldest_file).await;

        Ok(CacheStats {
            total_files,
            total_size,
            oldest_file,
        })
    }

    fn count_files_recursive<'a>(
        dir: &'a Path,
        total_size: &'a mut u64,
        oldest_file: &'a mut Option<chrono::DateTime<chrono::Utc>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = usize> + 'a>> {
        Box::pin(async move {
            let mut count = 0;

            let Ok(mut entries) = fs::read_dir(dir).await else {
                return 0;
            };

            while let Ok(Some(entry)) = entries.next_entry().await {
                let Ok(metadata) = entry.metadata().await else {
                    continue;
                };

                if metadata.is_file() {
                    count += 1;
                    *total_size += metadata.len();

                    if let Ok(modified) = metadata.modified() {
                        let modified_dt: chrono::DateTime<chrono::Utc> = modified.into();
                        *oldest_file = Some(match *oldest_file {
                            None => modified_dt,
                            Some(current) => current.min(modified_dt),
                        });
                    }
                } else if metadata.is_dir() {
                    count +=
                        Self::count_files_recursive(&entry.path(), total_size, oldest_file).await;
                }
            }

            count
        })
    }

    /// List all symbols that have cached data
    ///
    /// Scans cache directory and returns set of symbols with at least one cached file
    pub async fn list_cached_symbols(&self) -> Result<std::collections::HashSet<String>, DatabentoError> {
        use std::collections::HashSet;

        let mut cached_symbols = HashSet::new();

        // Read cache root directory
        let mut entries = fs::read_dir(&self.cache_root)
            .await
            .map_err(|e| DatabentoError::Cache(format!("Failed to read cache dir: {}", e)))?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Ok(metadata) = entry.metadata().await {
                if metadata.is_dir() {
                    // Directory name is sanitized symbol (ES-c-0 → ES.c.0)
                    // Preserve case to match ticker format
                    let dir_name = entry.file_name().to_string_lossy().to_string();
                    let symbol = dir_name.replace('-', ".");
                    log::debug!("  Found cached dir: {} → symbol: {}", dir_name, symbol);
                    cached_symbols.insert(symbol);
                }
            }
        }

        log::info!("CACHE: Found {} tickers with cached data", cached_symbols.len());
        for symbol in &cached_symbols {
            log::info!("CACHE:   - {}", symbol);
        }
        Ok(cached_symbols)
    }

    /// Clean up old cached files
    pub async fn cleanup_old_files(&self) -> Result<usize, DatabentoError> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(self.max_age_days as i64);
        let deleted_count = self.cleanup_dir(&self.cache_root, cutoff).await?;

        if deleted_count > 0 {
            log::info!("Cleaned up {} old cache files", deleted_count);
        }

        Ok(deleted_count)
    }

    /// Recursively cleanup a directory
    fn cleanup_dir<'a>(
        &'a self,
        dir: &'a Path,
        cutoff: chrono::DateTime<chrono::Utc>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<usize, DatabentoError>> + 'a>>
    {
        Box::pin(async move {
            let mut deleted_count = 0;

            let mut entries = fs::read_dir(dir)
                .await
                .map_err(|e| DatabentoError::Cache(format!("Failed to read dir: {}", e)))?;

            while let Some(entry) = entries.next_entry().await.ok().flatten() {
                let path = entry.path();
                let metadata = entry.metadata().await.map_err(|e| {
                    DatabentoError::Cache(format!("Failed to read metadata: {}", e))
                })?;

                if metadata.is_dir() {
                    deleted_count += self.cleanup_dir(&path, cutoff).await?;
                } else if metadata.is_file()
                    && let Ok(modified) = metadata.modified()
                {
                    let modified_dt: chrono::DateTime<chrono::Utc> = modified.into();
                    if modified_dt < cutoff && fs::remove_file(&path).await.is_ok() {
                        deleted_count += 1;
                        log::debug!("Deleted old cache file: {:?}", path);
                    }
                }
            }

            Ok(deleted_count)
        })
    }

    /// Clear all cached data
    pub async fn clear_all(&self) -> Result<(), DatabentoError> {
        if self.cache_root.exists() {
            fs::remove_dir_all(&self.cache_root)
                .await
                .map_err(|e| DatabentoError::Cache(format!("Failed to clear cache: {}", e)))?;

            self.init().await?;
            log::info!("Cleared all cached data");
        }
        Ok(())
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Total number of cached files
    pub total_files: usize,
    /// Total size of cached data in bytes
    pub total_size: u64,
    /// Timestamp of oldest cached file
    pub oldest_file: Option<chrono::DateTime<chrono::Utc>>,
}

impl CacheStats {
    /// Get human-readable size string
    pub fn size_human_readable(&self) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = self.total_size as f64;
        let mut unit_idx = 0;

        while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
            size /= 1024.0;
            unit_idx += 1;
        }

        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_path_generation() {
        let config = DatabentoConfig {
            cache_dir: PathBuf::from("/tmp/cache"),
            ..Default::default()
        };
        let cache = CacheManager::new(&config);

        let date = chrono::NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
        let path = cache.get_cache_path("ES.c.0", Schema::Trades, date);

        assert!(path.to_string_lossy().contains("ES-c-0"));
        assert!(path.to_string_lossy().contains("trades"));
        assert!(path.to_string_lossy().contains("2025-01-15"));
        assert!(path.to_string_lossy().ends_with(".dbn.zst"));
    }

    #[test]
    fn test_cache_stats_size_formatting() {
        let stats = CacheStats {
            total_files: 10,
            total_size: 1_500_000,
            oldest_file: None,
        };

        let readable = stats.size_human_readable();
        assert!(readable.contains("MB") || readable.contains("KB"));
    }
}
