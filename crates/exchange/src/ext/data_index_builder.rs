//! Cache scanner that builds a `DataIndex` from on-disk Databento cache
//! files and Rithmic realtime subscriptions.

use kairos_data::{DataIndex, DataKey, FeedId};
use std::collections::BTreeSet;
use std::path::Path;

/// Scan the Databento local cache directory and build a `DataIndex`.
///
/// Cache layout: `{cache_root}/{symbol-sanitized}/{schema}/{YYYY-MM-DD}.dbn.zst`
///
/// Symbols on disk use `-` as separator (e.g. `ES-c-0`); we convert back to
/// `.` (e.g. `ES.c.0`) to match the ticker format used elsewhere.
pub async fn scan_databento_cache(
    cache_root: &Path,
    feed_id: FeedId,
) -> Result<DataIndex, String> {
    let mut index = DataIndex::new();

    let mut symbol_dirs = tokio::fs::read_dir(cache_root)
        .await
        .map_err(|e| format!("Failed to read cache root: {}", e))?;

    while let Some(symbol_entry) = symbol_dirs
        .next_entry()
        .await
        .map_err(|e| format!("Error reading symbol dir: {}", e))?
    {
        let meta = symbol_entry
            .metadata()
            .await
            .map_err(|e| format!("Metadata error: {}", e))?;
        if !meta.is_dir() {
            continue;
        }

        // Convert sanitized dir name back to ticker (ES-c-0 → ES.c.0)
        let dir_name = symbol_entry.file_name().to_string_lossy().to_string();
        let ticker = dir_name.replace('-', ".");

        // Walk schema subdirectories
        let mut schema_dirs = match tokio::fs::read_dir(symbol_entry.path()).await {
            Ok(d) => d,
            Err(_) => continue,
        };

        while let Some(schema_entry) = schema_dirs.next_entry().await.unwrap_or(None) {
            let schema_meta = match schema_entry.metadata().await {
                Ok(m) => m,
                Err(_) => continue,
            };
            if !schema_meta.is_dir() {
                continue;
            }

            let schema = schema_entry.file_name().to_string_lossy().to_string();

            // Walk date files: YYYY-MM-DD.dbn.zst
            let mut date_files = match tokio::fs::read_dir(schema_entry.path()).await {
                Ok(d) => d,
                Err(_) => continue,
            };

            let mut dates = BTreeSet::new();
            while let Some(file_entry) = date_files.next_entry().await.unwrap_or(None) {
                let file_name = file_entry.file_name().to_string_lossy().to_string();
                if !file_name.ends_with(".dbn.zst") {
                    continue;
                }
                // Parse date from "YYYY-MM-DD.dbn.zst"
                let date_str = file_name.trim_end_matches(".dbn.zst");
                if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                    dates.insert(date);
                }
            }

            if !dates.is_empty() {
                let key = DataKey {
                    ticker: ticker.clone(),
                    schema: schema.clone(),
                };
                index.add_contribution(key, feed_id, dates, false);
            }
        }
    }

    let tickers = index.available_tickers();
    log::info!(
        "Scanned {} ticker(s) from Databento cache at {:?}",
        tickers.len(),
        cache_root
    );
    for t in &tickers {
        log::info!("  - {}", t);
    }

    Ok(index)
}

/// Build a `DataIndex` contribution for Rithmic realtime subscriptions.
///
/// Rithmic feeds provide no historical dates on disk; they just mark tickers
/// as having realtime data, which causes `resolve_chart_range()` to extend
/// the end date to today.
pub fn build_rithmic_contribution(
    feed_id: FeedId,
    subscribed_tickers: &[String],
) -> DataIndex {
    let mut index = DataIndex::new();
    for ticker in subscribed_tickers {
        let key = DataKey {
            ticker: ticker.clone(),
            schema: "trades".to_string(),
        };
        index.add_contribution(key, feed_id, BTreeSet::new(), true);
    }
    index
}
