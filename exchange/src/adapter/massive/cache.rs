use super::{MassiveError, MassiveResult};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Cache manager for Massive API data
///
/// Stores data per-day in JSON format with Zstandard compression.
///
/// Directory structure:
/// ```text
/// cache/massive/
/// ├── snapshots/
/// │   ├── AAPL/
/// │   │   ├── 2024-01-01.json.zst
/// │   │   └── 2024-01-02.json.zst
/// │   └── TSLA/
/// │       └── 2024-01-01.json.zst
/// ├── chains/
/// │   └── SPY/
/// │       └── 2024-01-01.json.zst
/// └── contracts/
///     └── AAPL/
///         └── metadata.json.zst
/// ```
pub struct CacheManager {
    cache_root: PathBuf,
    max_age_days: u32,
}

impl CacheManager {
    /// Create a new cache manager
    pub fn new(cache_root: PathBuf, max_age_days: u32) -> Self {
        Self {
            cache_root,
            max_age_days,
        }
    }

    /// Initialize cache directories
    pub async fn init(&self) -> MassiveResult<()> {
        let dirs = ["snapshots", "chains", "contracts"];

        for dir in &dirs {
            let path = self.cache_root.join(dir);
            tokio::fs::create_dir_all(&path).await?;
        }

        log::info!("Initialized Massive cache at {:?}", self.cache_root);

        Ok(())
    }

    /// Check if data is cached for a symbol and date
    pub async fn has_cached(
        &self,
        data_type: &str,
        symbol: &str,
        date: NaiveDate,
    ) -> bool {
        let path = self.cache_path(data_type, symbol, Some(date));
        tokio::fs::metadata(&path).await.is_ok()
    }

    /// Get cache file path (returns path even if file doesn't exist)
    pub fn get_cache_path(
        &self,
        data_type: &str,
        symbol: &str,
        date: Option<NaiveDate>,
    ) -> PathBuf {
        self.cache_path(data_type, symbol, date)
    }

    /// Store data in cache
    pub async fn store<T: Serialize>(
        &self,
        data_type: &str,
        symbol: &str,
        date: Option<NaiveDate>,
        data: &T,
    ) -> MassiveResult<()> {
        let path = self.cache_path(data_type, symbol, date);

        // Create parent directory
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Serialize to JSON
        let json = serde_json::to_string(data)?;

        // Compress with zstd
        let compressed = zstd::encode_all(json.as_bytes(), 3)
            .map_err(|e| MassiveError::Cache(format!("Compression failed: {}", e)))?;

        // Write to file
        tokio::fs::write(&path, compressed).await?;

        log::debug!("Cached {} for {} at {:?}", data_type, symbol, path);

        Ok(())
    }

    /// Load data from cache
    pub async fn load<T: for<'de> Deserialize<'de>>(
        &self,
        data_type: &str,
        symbol: &str,
        date: Option<NaiveDate>,
    ) -> MassiveResult<T> {
        let path = self.cache_path(data_type, symbol, date);

        // Read compressed file
        let compressed = tokio::fs::read(&path).await?;

        // Decompress
        let decompressed = zstd::decode_all(&compressed[..])
            .map_err(|e| MassiveError::Cache(format!("Decompression failed: {}", e)))?;

        // Deserialize JSON
        let data: T = serde_json::from_slice(&decompressed)?;

        log::debug!("Loaded {} for {} from cache", data_type, symbol);

        Ok(data)
    }

    /// Get cache statistics
    pub async fn stats(&self) -> MassiveResult<CacheStats> {
        let mut stats = CacheStats {
            total_files: 0,
            total_size_bytes: 0,
            oldest_file: None,
        };

        self.walk_cache_dir(&self.cache_root, &mut stats).await?;

        Ok(stats)
    }

    /// Clean up old cache files
    pub async fn cleanup_old_files(&self) -> MassiveResult<usize> {
        let max_age = chrono::Duration::days(self.max_age_days as i64);
        let cutoff = chrono::Utc::now().naive_utc() - max_age;
        let mut removed = 0;

        removed += self.cleanup_dir(&self.cache_root, cutoff).await?;

        log::info!("Cleaned up {} old cache files", removed);

        Ok(removed)
    }

    // Private helper methods

    fn cache_path(&self, data_type: &str, symbol: &str, date: Option<NaiveDate>) -> PathBuf {
        let base = self.cache_root.join(data_type).join(symbol);

        if let Some(date) = date {
            base.join(format!("{}.json.zst", date.format("%Y-%m-%d")))
        } else {
            base.join("metadata.json.zst")
        }
    }

    fn walk_cache_dir<'a>(&'a self, dir: &'a Path, stats: &'a mut CacheStats) -> std::pin::Pin<Box<dyn std::future::Future<Output = MassiveResult<()>> + Send + 'a>> {
        Box::pin(async move {
            let mut entries = tokio::fs::read_dir(dir).await?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                let metadata = entry.metadata().await?;

                if metadata.is_file() {
                    stats.total_files += 1;
                    stats.total_size_bytes += metadata.len();

                    if let Ok(modified) = metadata.modified() {
                        let modified_dt = chrono::DateTime::<chrono::Utc>::from(modified);

                        if stats.oldest_file.is_none()
                            || Some(modified_dt) < stats.oldest_file
                        {
                            stats.oldest_file = Some(modified_dt);
                        }
                    }
                } else if metadata.is_dir() {
                    self.walk_cache_dir(&path, stats).await?;
                }
            }

            Ok(())
        })
    }

    fn cleanup_dir<'a>(&'a self, dir: &'a Path, cutoff: chrono::NaiveDateTime) -> std::pin::Pin<Box<dyn std::future::Future<Output = MassiveResult<usize>> + Send + 'a>> {
        Box::pin(async move {
            let mut removed = 0;

            if !dir.exists() {
                return Ok(0);
            }

            let mut entries = tokio::fs::read_dir(dir).await?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                let metadata = entry.metadata().await?;

                if metadata.is_file() {
                    if let Ok(modified) = metadata.modified() {
                        let modified_dt = chrono::DateTime::<chrono::Utc>::from(modified);

                        if modified_dt.naive_utc() < cutoff {
                            tokio::fs::remove_file(&path).await?;
                            removed += 1;
                            log::debug!("Removed old cache file: {:?}", path);
                        }
                    }
                } else if metadata.is_dir() {
                    removed += self.cleanup_dir(&path, cutoff).await?;

                    // Remove empty directories
                    if fs::read_dir(&path)?.count() == 0 {
                        tokio::fs::remove_dir(&path).await?;
                    }
                }
            }

            Ok(removed)
        })
    }
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    /// Total number of cached files
    pub total_files: usize,

    /// Total size in bytes
    pub total_size_bytes: u64,

    /// Oldest file timestamp
    pub oldest_file: Option<chrono::DateTime<chrono::Utc>>,
}

impl CacheStats {
    /// Get human-readable size
    pub fn size_human_readable(&self) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = self.total_size_bytes as f64;
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
    use tempfile::TempDir;

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct TestData {
        value: String,
        number: i32,
    }

    #[tokio::test]
    async fn test_cache_manager_init() {
        let temp_dir = TempDir::new().unwrap();
        let cache = CacheManager::new(temp_dir.path().to_path_buf(), 30);

        assert!(cache.init().await.is_ok());

        // Check directories were created
        assert!(temp_dir.path().join("snapshots").exists());
        assert!(temp_dir.path().join("chains").exists());
        assert!(temp_dir.path().join("contracts").exists());
    }

    #[tokio::test]
    async fn test_store_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let cache = CacheManager::new(temp_dir.path().to_path_buf(), 30);
        cache.init().await.unwrap();

        let test_data = TestData {
            value: "test".to_string(),
            number: 42,
        };

        let date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();

        // Store
        cache
            .store("snapshots", "TEST", Some(date), &test_data)
            .await
            .unwrap();

        // Load
        let loaded: TestData = cache
            .load("snapshots", "TEST", Some(date))
            .await
            .unwrap();

        assert_eq!(loaded, test_data);
    }

    #[tokio::test]
    async fn test_has_cached() {
        let temp_dir = TempDir::new().unwrap();
        let cache = CacheManager::new(temp_dir.path().to_path_buf(), 30);
        cache.init().await.unwrap();

        let date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();

        assert!(!cache.has_cached("snapshots", "TEST", date).await);

        let test_data = TestData {
            value: "test".to_string(),
            number: 42,
        };

        cache
            .store("snapshots", "TEST", Some(date), &test_data)
            .await
            .unwrap();

        assert!(cache.has_cached("snapshots", "TEST", date).await);
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let temp_dir = TempDir::new().unwrap();
        let cache = CacheManager::new(temp_dir.path().to_path_buf(), 30);
        cache.init().await.unwrap();

        let test_data = TestData {
            value: "test".to_string(),
            number: 42,
        };

        cache
            .store("snapshots", "TEST", None, &test_data)
            .await
            .unwrap();

        let stats = cache.stats().await.unwrap();
        assert_eq!(stats.total_files, 1);
        assert!(stats.total_size_bytes > 0);
    }
}
