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

/// Data provider that populated a cache directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheProvider {
    /// Databento historical data
    Databento,
    /// Rithmic live/historical data
    Rithmic,
}

impl CacheProvider {
    /// Returns the filesystem directory name for this provider
    #[must_use]
    pub fn dir_name(self) -> &'static str {
        match self {
            CacheProvider::Databento => "databento",
            CacheProvider::Rithmic => "rithmic",
        }
    }
}

/// Type of data stored in a cache file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheSchema {
    /// Tick-by-tick trade data
    Trades,
    /// Order book depth snapshots
    Depth,
    /// Pre-aggregated OHLCV candles
    Ohlcv,
}

impl CacheSchema {
    /// Returns the filesystem directory name for this schema
    #[must_use]
    pub fn dir_name(self) -> &'static str {
        match self {
            CacheSchema::Trades => "trades",
            CacheSchema::Depth => "depth",
            CacheSchema::Ohlcv => "ohlcv",
        }
    }
}

/// Unified cache store for all providers and schemas.
///
/// Provides async read/write/scan/evict operations backed by the filesystem.
/// Writes are atomic (`.tmp` then rename); reads are lock-free.
#[derive(Debug, Clone)]
pub struct CacheStore {
    cache_root: PathBuf,
}

impl CacheStore {
    /// Creates a new cache store rooted at the given directory
    #[must_use]
    pub fn new(cache_root: PathBuf) -> Self {
        Self { cache_root }
    }

    /// Ensures the cache root directory exists
    pub async fn init(&self) -> Result<(), crate::Error> {
        fs::create_dir_all(&self.cache_root).await?;
        log::info!("Cache store initialized at: {:?}", self.cache_root);
        Ok(())
    }

    // ── Path helpers ──────────────────────────────────────────────────

    /// Replaces dots with dashes for filesystem-safe directory names
    fn sanitize_symbol(symbol: &str) -> String {
        symbol.replace('.', "-")
    }

    /// Returns the directory path for a symbol under a provider
    fn symbol_dir(&self, provider: CacheProvider, symbol: &str) -> PathBuf {
        self.cache_root
            .join(provider.dir_name())
            .join(Self::sanitize_symbol(symbol))
    }

    /// Returns the full path to a day's cache file
    #[must_use]
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

    /// Returns `true` if a cache file exists for the given parameters
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

    /// Reads and deserializes a day's cached records
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

    /// Serializes and atomically writes a day's records to the cache.
    ///
    /// Writes to a `.tmp` file first, then renames to the final path to
    /// prevent partial-write corruption.
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

    /// Atomically writes raw bytes to a day file (e.g. for .dbn.zst passthrough)
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

    /// Returns all cached dates for a given symbol, schema, and provider
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

    /// Returns all symbol names found under a provider's cache directory.
    ///
    /// Converts sanitized directory names back to dotted form (e.g. "ES-c-0" to "ES.c.0").
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
                let dir_name = entry.file_name().to_string_lossy().to_string();
                symbols.push(dir_name.replace('-', "."));
            }
        }
        symbols
    }

    /// Builds a [`DataIndex`] by scanning a provider's entire cache directory
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

    /// Deletes cached files older than `max_age_days`. Returns the count of deleted files.
    pub async fn evict_old(&self, provider: CacheProvider, max_age_days: u32) -> usize {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(max_age_days as i64);
        let dir = self.cache_root.join(provider.dir_name());
        walk_and_delete(&dir, cutoff).await
    }

    // ── Stats ─────────────────────────────────────────────────────────

    /// Computes aggregate cache statistics by walking the entire cache directory
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

/// Walks the directory tree breadth-first, collecting file count, total size, and oldest mtime.
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

/// Recursively deletes files older than `cutoff`. Returns the deleted count.
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

    fn make_trade(time: u64, price: f32, qty: f64, side: Side) -> Trade {
        Trade::new(
            Timestamp::from_millis(time),
            Price::from_f32(price),
            Quantity(qty),
            side,
        )
    }

    #[tokio::test]
    async fn test_write_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().to_path_buf());
        store.init().await.unwrap();

        let trades = vec![
            make_trade(1000, 5000.0, 10.0, Side::Buy),
            make_trade(2000, 5001.25, 5.0, Side::Sell),
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
        assert_eq!(loaded[0].side, Side::Buy);
        assert_eq!(loaded[1].side, Side::Sell);
        assert!((loaded[0].quantity.0 - 10.0).abs() < 0.01);
        assert!((loaded[1].quantity.0 - 5.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn roundtrip_preserves_all_trade_fields() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().to_path_buf());
        store.init().await.unwrap();

        let trades = vec![make_trade(1736899200000, 5432.75, 42.0, Side::Buy)];
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

        store
            .write_day(
                CacheProvider::Rithmic,
                "ES.c.0",
                CacheSchema::Trades,
                date,
                &trades,
            )
            .await
            .unwrap();

        let loaded: Vec<Trade> = store
            .read_day(CacheProvider::Rithmic, "ES.c.0", CacheSchema::Trades, date)
            .await
            .unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].time.to_millis(), 1736899200000);
        assert!((loaded[0].price.to_f64() - 5432.75).abs() < 0.01);
        assert!((loaded[0].quantity.0 - 42.0).abs() < 0.01);
        assert_eq!(loaded[0].side, Side::Buy);
    }

    #[tokio::test]
    async fn per_day_keying_different_dates_stored_separately() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().to_path_buf());
        store.init().await.unwrap();

        let day1 = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let day2 = NaiveDate::from_ymd_opt(2025, 1, 11).unwrap();

        let trades1 = vec![make_trade(1000, 100.0, 5.0, Side::Buy)];
        let trades2 = vec![
            make_trade(2000, 200.0, 10.0, Side::Sell),
            make_trade(3000, 201.0, 15.0, Side::Buy),
        ];

        store
            .write_day(
                CacheProvider::Databento,
                "ES.c.0",
                CacheSchema::Trades,
                day1,
                &trades1,
            )
            .await
            .unwrap();
        store
            .write_day(
                CacheProvider::Databento,
                "ES.c.0",
                CacheSchema::Trades,
                day2,
                &trades2,
            )
            .await
            .unwrap();

        let loaded1: Vec<Trade> = store
            .read_day(
                CacheProvider::Databento,
                "ES.c.0",
                CacheSchema::Trades,
                day1,
            )
            .await
            .unwrap();
        let loaded2: Vec<Trade> = store
            .read_day(
                CacheProvider::Databento,
                "ES.c.0",
                CacheSchema::Trades,
                day2,
            )
            .await
            .unwrap();

        assert_eq!(loaded1.len(), 1);
        assert_eq!(loaded2.len(), 2);
        assert_eq!(loaded1[0].price.to_f32(), 100.0);
        assert_eq!(loaded2[0].price.to_f32(), 200.0);
    }

    #[tokio::test]
    async fn has_day_false_for_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().to_path_buf());
        store.init().await.unwrap();

        let date = NaiveDate::from_ymd_opt(2025, 6, 1).unwrap();
        assert!(
            !store
                .has_day(
                    CacheProvider::Databento,
                    "ES.c.0",
                    CacheSchema::Trades,
                    date
                )
                .await
        );
    }

    #[tokio::test]
    async fn read_missing_file_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().to_path_buf());
        store.init().await.unwrap();

        let date = NaiveDate::from_ymd_opt(2025, 6, 1).unwrap();
        let result: Result<Vec<Trade>, _> = store
            .read_day(
                CacheProvider::Databento,
                "ES.c.0",
                CacheSchema::Trades,
                date,
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn corrupted_data_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().to_path_buf());
        store.init().await.unwrap();

        let date = NaiveDate::from_ymd_opt(2025, 1, 20).unwrap();
        let path = store.day_file_path(
            CacheProvider::Databento,
            "ES.c.0",
            CacheSchema::Trades,
            date,
        );

        // Create parent dirs and write garbage
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.unwrap();
        }
        fs::write(&path, b"this is not valid cache data")
            .await
            .unwrap();

        let result: Result<Vec<Trade>, _> = store
            .read_day(
                CacheProvider::Databento,
                "ES.c.0",
                CacheSchema::Trades,
                date,
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn atomic_write_no_leftover_tmp() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().to_path_buf());
        store.init().await.unwrap();

        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
        let trades = vec![make_trade(1000, 100.0, 1.0, Side::Buy)];

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

        // Check the final file exists
        let path = store.day_file_path(
            CacheProvider::Databento,
            "ES.c.0",
            CacheSchema::Trades,
            date,
        );
        assert!(fs::try_exists(&path).await.unwrap());

        // Check no .tmp leftover
        let tmp = path.with_extension("bin.zst.tmp");
        assert!(!fs::try_exists(&tmp).await.unwrap_or(false));
    }

    #[tokio::test]
    async fn write_empty_records_then_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().to_path_buf());
        store.init().await.unwrap();

        let date = NaiveDate::from_ymd_opt(2025, 3, 1).unwrap();
        let empty: Vec<Trade> = vec![];

        store
            .write_day(
                CacheProvider::Rithmic,
                "NQ.c.0",
                CacheSchema::Trades,
                date,
                &empty,
            )
            .await
            .unwrap();

        let loaded: Vec<Trade> = store
            .read_day(CacheProvider::Rithmic, "NQ.c.0", CacheSchema::Trades, date)
            .await
            .unwrap();
        assert!(loaded.is_empty());
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
        // Dates should be ordered (BTreeSet)
        let dates_vec: Vec<_> = dates.into_iter().collect();
        assert!(dates_vec[0] < dates_vec[1]);
        assert!(dates_vec[1] < dates_vec[2]);
    }

    #[tokio::test]
    async fn list_dates_empty_for_nonexistent_symbol() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().to_path_buf());
        store.init().await.unwrap();

        let dates = store
            .list_dates(CacheProvider::Databento, "FAKE.c.0", CacheSchema::Trades)
            .await;
        assert!(dates.is_empty());
    }

    #[tokio::test]
    async fn list_symbols_converts_dash_to_dot() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().to_path_buf());
        store.init().await.unwrap();

        let trades: Vec<Trade> = vec![];
        let date = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
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

        let symbols = store.list_symbols(CacheProvider::Databento).await;
        assert_eq!(symbols.len(), 1);
        // The directory is "ES-c-0", list_symbols converts back to "ES.c.0"
        assert_eq!(symbols[0], "ES.c.0");
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

    #[tokio::test]
    async fn day_file_path_format() {
        let store = CacheStore::new(PathBuf::from("/tmp/cache"));
        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let path = store.day_file_path(
            CacheProvider::Databento,
            "ES.c.0",
            CacheSchema::Trades,
            date,
        );
        let path_str = path.to_string_lossy().replace('\\', "/");
        assert!(path_str.ends_with("databento/ES-c-0/trades/2025-03-15.bin.zst"));
    }

    #[tokio::test]
    async fn provider_dir_names() {
        assert_eq!(CacheProvider::Databento.dir_name(), "databento");
        assert_eq!(CacheProvider::Rithmic.dir_name(), "rithmic");
    }

    #[tokio::test]
    async fn schema_dir_names() {
        assert_eq!(CacheSchema::Trades.dir_name(), "trades");
        assert_eq!(CacheSchema::Depth.dir_name(), "depth");
        assert_eq!(CacheSchema::Ohlcv.dir_name(), "ohlcv");
    }
}
