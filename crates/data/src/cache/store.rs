//! CacheStore — unified read/write/scan operations
//!
//! Layout:
//! ```text
//! {cache_root}/
//!   {provider}/          -- "databento" | "rithmic"
//!     {symbol-sanitized}/ -- "ES-c-0"
//!       trades/
//!         2025-01-15.bin.zst
//!       depth/
//!         2025-01-15.bin.zst
//!       ohlcv/
//!         2025-01-15.bin.zst
//! ```
//!
//! Writes are atomic: write to `.tmp`, then `rename`.
//! Reads are lock-free (immutable files, async fs I/O).

use super::{
    format::{DayFileHeader, decode, encode},
    stats::CacheStats,
};
use crate::domain::index::{DataIndex, DataKey};
use crate::domain::types::FeedId;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Which data provider populated a cache directory
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheProvider {
    Databento,
    Rithmic,
}

impl CacheProvider {
    pub fn dir_name(self) -> &'static str {
        match self {
            CacheProvider::Databento => "databento",
            CacheProvider::Rithmic => "rithmic",
        }
    }
}

/// Which type of data is stored in a cache file
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheSchema {
    Trades,
    Depth,
    Ohlcv,
}

impl CacheSchema {
    pub fn dir_name(self) -> &'static str {
        match self {
            CacheSchema::Trades => "trades",
            CacheSchema::Depth => "depth",
            CacheSchema::Ohlcv => "ohlcv",
        }
    }
}

/// Unified cache store for all providers and schemas
#[derive(Debug, Clone)]
pub struct CacheStore {
    cache_root: PathBuf,
}

impl CacheStore {
    pub fn new(cache_root: PathBuf) -> Self {
        Self { cache_root }
    }

    /// Initialize cache directory
    pub async fn init(&self) -> Result<(), crate::Error> {
        fs::create_dir_all(&self.cache_root).await?;
        log::info!("Cache store initialized at: {:?}", self.cache_root);
        Ok(())
    }

    // ── Path helpers ──────────────────────────────────────────────────

    fn sanitize_symbol(symbol: &str) -> String {
        symbol.replace('.', "-")
    }

    fn symbol_dir(&self, provider: CacheProvider, symbol: &str) -> PathBuf {
        self.cache_root
            .join(provider.dir_name())
            .join(Self::sanitize_symbol(symbol))
    }

    pub fn day_file_path(
        &self,
        provider: CacheProvider,
        symbol: &str,
        schema: CacheSchema,
        date: NaiveDate,
    ) -> PathBuf {
        self.symbol_dir(provider, symbol)
            .join(schema.dir_name())
            .join(format!("{}.bin.zst", date.format("%Y-%m-%d")))
    }

    // ── Existence checks ──────────────────────────────────────────────

    pub async fn has_day(
        &self,
        provider: CacheProvider,
        symbol: &str,
        schema: CacheSchema,
        date: NaiveDate,
    ) -> bool {
        let path = self.day_file_path(provider, symbol, schema, date);
        fs::try_exists(&path).await.unwrap_or(false)
    }

    // ── Read ──────────────────────────────────────────────────────────

    pub async fn read_day<T>(
        &self,
        provider: CacheProvider,
        symbol: &str,
        schema: CacheSchema,
        date: NaiveDate,
    ) -> Result<Vec<T>, crate::Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        let path = self.day_file_path(provider, symbol, schema, date);
        let bytes = fs::read(&path).await.map_err(|e| {
            crate::Error::Cache(format!("Read failed for {}: {}", path.display(), e))
        })?;

        let (_, records) = decode::<T>(&bytes).map_err(|e| {
            crate::Error::Cache(format!("Decode failed for {}: {}", path.display(), e))
        })?;
        Ok(records)
    }

    // ── Write (atomic) ────────────────────────────────────────────────

    pub async fn write_day<T>(
        &self,
        provider: CacheProvider,
        symbol: &str,
        schema: CacheSchema,
        date: NaiveDate,
        records: &[T],
    ) -> Result<(), crate::Error>
    where
        T: Serialize,
    {
        let path = self.day_file_path(provider, symbol, schema, date);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let header = DayFileHeader::new(
            schema.dir_name(),
            symbol,
            date.format("%Y-%m-%d").to_string(),
            records.len() as u64,
        );

        let encoded = encode(&header, records).map_err(crate::Error::Cache)?;

        // Atomic write: write to .tmp, then rename
        let tmp_path = path.with_extension("bin.zst.tmp");
        fs::write(&tmp_path, &encoded).await.map_err(|e| {
            crate::Error::Cache(format!("Write failed for {}: {}", tmp_path.display(), e))
        })?;

        fs::rename(&tmp_path, &path).await.map_err(|e| {
            crate::Error::Cache(format!("Rename failed for {}: {}", tmp_path.display(), e))
        })?;

        log::debug!(
            "Cached {} {} {} {} ({} records)",
            provider.dir_name(),
            symbol,
            schema.dir_name(),
            date,
            records.len()
        );
        Ok(())
    }

    /// Write raw bytes to a day file (e.g. for .dbn.zst passthrough)
    pub async fn write_raw(
        &self,
        provider: CacheProvider,
        symbol: &str,
        schema: CacheSchema,
        date: NaiveDate,
        bytes: &[u8],
    ) -> Result<(), crate::Error> {
        let path = self.day_file_path(provider, symbol, schema, date);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let tmp_path = path.with_extension("bin.zst.tmp");
        fs::write(&tmp_path, bytes).await?;
        fs::rename(&tmp_path, &path).await?;
        Ok(())
    }

    // ── Scan ──────────────────────────────────────────────────────────

    /// Scan all cached dates for a symbol/schema/provider
    pub async fn list_dates(
        &self,
        provider: CacheProvider,
        symbol: &str,
        schema: CacheSchema,
    ) -> BTreeSet<NaiveDate> {
        let dir = self.symbol_dir(provider, symbol).join(schema.dir_name());
        let mut dates = BTreeSet::new();

        let Ok(mut entries) = fs::read_dir(&dir).await else {
            return dates;
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            let stem = name.trim_end_matches(".bin.zst");
            if let Ok(date) = NaiveDate::parse_from_str(stem, "%Y-%m-%d") {
                dates.insert(date);
            }
        }
        dates
    }

    /// Scan all symbols under a provider
    pub async fn list_symbols(&self, provider: CacheProvider) -> Vec<String> {
        let dir = self.cache_root.join(provider.dir_name());
        let mut symbols = Vec::new();

        let Ok(mut entries) = fs::read_dir(&dir).await else {
            return symbols;
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let Ok(meta) = entry.metadata().await else {
                continue;
            };
            if meta.is_dir() {
                // Convert sanitized dir name back (ES-c-0 → ES.c.0)
                let dir_name = entry.file_name().to_string_lossy().to_string();
                symbols.push(dir_name.replace('-', "."));
            }
        }
        symbols
    }

    /// Build a DataIndex by scanning a provider's cache directory
    pub async fn scan_to_index(&self, provider: CacheProvider, feed_id: FeedId) -> DataIndex {
        let mut index = DataIndex::new();
        let schemas = [CacheSchema::Trades, CacheSchema::Depth, CacheSchema::Ohlcv];

        for symbol in self.list_symbols(provider).await {
            for schema in schemas {
                let dates = self.list_dates(provider, &symbol, schema).await;
                if !dates.is_empty() {
                    let key = DataKey {
                        ticker: symbol.clone(),
                        schema: schema.dir_name().to_string(),
                    };
                    index.add_contribution(key, feed_id, dates, false);
                }
            }
        }

        index
    }

    // ── Evict ─────────────────────────────────────────────────────────

    /// Delete cached files older than `max_age_days`
    pub async fn evict_old(&self, provider: CacheProvider, max_age_days: u32) -> usize {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(max_age_days as i64);
        let dir = self.cache_root.join(provider.dir_name());
        walk_and_delete(&dir, cutoff).await
    }

    // ── Stats ─────────────────────────────────────────────────────────

    pub async fn stats(&self) -> CacheStats {
        let (total_files, total_size_bytes, oldest_file) = bfs_count(&self.cache_root).await;
        CacheStats {
            total_files,
            total_size_bytes,
            oldest_file,
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// BFS directory walk for stats
async fn bfs_count(root: &Path) -> (usize, u64, Option<chrono::DateTime<chrono::Utc>>) {
    let mut queue = vec![root.to_path_buf()];
    let mut count = 0usize;
    let mut total_size = 0u64;
    let mut oldest: Option<chrono::DateTime<chrono::Utc>> = None;

    while let Some(dir) = queue.pop() {
        let Ok(mut entries) = fs::read_dir(&dir).await else {
            continue;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let Ok(meta) = entry.metadata().await else {
                continue;
            };
            if meta.is_file() {
                count += 1;
                total_size += meta.len();
                if let Ok(modified) = meta.modified() {
                    let dt: chrono::DateTime<chrono::Utc> = modified.into();
                    oldest = Some(match oldest {
                        None => dt,
                        Some(current) => current.min(dt),
                    });
                }
            } else if meta.is_dir() {
                queue.push(entry.path());
            }
        }
    }

    (count, total_size, oldest)
}

/// Recursively delete files older than `cutoff`. Returns deleted count.
fn walk_and_delete<'a>(
    dir: &'a Path,
    cutoff: chrono::DateTime<chrono::Utc>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = usize> + Send + 'a>> {
    Box::pin(async move {
        let mut deleted = 0usize;
        let Ok(mut entries) = fs::read_dir(dir).await else {
            return deleted;
        };

        while let Some(entry) = entries.next_entry().await.ok().flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata().await else {
                continue;
            };

            if meta.is_dir() {
                deleted += walk_and_delete(&path, cutoff).await;
            } else if meta.is_file()
                && let Ok(modified) = meta.modified()
            {
                let dt: chrono::DateTime<chrono::Utc> = modified.into();
                if dt < cutoff && fs::remove_file(&path).await.is_ok() {
                    deleted += 1;
                }
            }
        }
        deleted
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Price, Quantity, Side, Timestamp, Trade};

    #[tokio::test]
    async fn test_write_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().to_path_buf());
        store.init().await.unwrap();

        let trades = vec![
            Trade::new(
                Timestamp::from_millis(1000),
                Price::from_f32(5000.0),
                Quantity(10.0),
                Side::Buy,
            ),
            Trade::new(
                Timestamp::from_millis(2000),
                Price::from_f32(5001.25),
                Quantity(5.0),
                Side::Sell,
            ),
        ];

        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
        store
            .write_day(
                CacheProvider::Databento,
                "ES.c.0",
                CacheSchema::Trades,
                date,
                &trades,
            )
            .await
            .unwrap();

        assert!(
            store
                .has_day(
                    CacheProvider::Databento,
                    "ES.c.0",
                    CacheSchema::Trades,
                    date
                )
                .await
        );

        let loaded: Vec<Trade> = store
            .read_day(
                CacheProvider::Databento,
                "ES.c.0",
                CacheSchema::Trades,
                date,
            )
            .await
            .unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].price.to_f32(), 5000.0);
        assert_eq!(loaded[1].price.to_f32(), 5001.25);
    }

    #[tokio::test]
    async fn test_list_dates() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().to_path_buf());
        store.init().await.unwrap();

        let records: Vec<Trade> = vec![];
        for day in 1..=3 {
            let date = NaiveDate::from_ymd_opt(2025, 1, day).unwrap();
            store
                .write_day(
                    CacheProvider::Rithmic,
                    "NQ.c.0",
                    CacheSchema::Trades,
                    date,
                    &records,
                )
                .await
                .unwrap();
        }

        let dates = store
            .list_dates(CacheProvider::Rithmic, "NQ.c.0", CacheSchema::Trades)
            .await;
        assert_eq!(dates.len(), 3);
    }

    #[tokio::test]
    async fn test_scan_to_index() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().to_path_buf());
        store.init().await.unwrap();

        let feed_id = uuid::Uuid::new_v4();
        let records: Vec<Trade> = vec![];
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

        store
            .write_day(
                CacheProvider::Databento,
                "ES.c.0",
                CacheSchema::Trades,
                date,
                &records,
            )
            .await
            .unwrap();

        let index = store.scan_to_index(CacheProvider::Databento, feed_id).await;
        assert!(index.has_data("ES.c.0"));
    }
}
