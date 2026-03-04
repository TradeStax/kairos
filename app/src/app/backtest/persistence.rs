//! Disk persistence for backtest results.
//!
//! Each completed backtest is stored as a JSON file in
//! `{data_path}/backtests/{uuid}.json`. A lightweight index file
//! (`index.json`) enables fast sidebar listing without deserializing
//! every result.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Lightweight index entry for fast sidebar listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestIndexEntry {
    pub id: Uuid,
    pub strategy_name: String,
    pub ticker: String,
    pub started_at_ms: u64,
    pub net_pnl_usd: f64,
    pub total_trades: usize,
    pub win_rate: f64,
    pub file_name: String,
}

#[derive(Serialize, Deserialize)]
struct BacktestIndex {
    version: u32,
    entries: Vec<BacktestIndexEntry>,
}

/// Returns the backtests storage directory, creating it if needed.
fn backtests_dir() -> PathBuf {
    crate::infra::platform::data_path(Some("backtests"))
}

/// Save a completed backtest result to disk.
///
/// Writes the result as `{uuid}.json` with an atomic tmp+rename
/// pattern, then updates the index file.
pub async fn save_backtest_result(
    result: &backtest::BacktestResult,
    strategy_name: &str,
    ticker: &str,
    started_at_ms: u64,
) -> Result<PathBuf, String> {
    let dir = backtests_dir();
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("Failed to create backtests dir: {e}"))?;

    let file_name = format!("{}.json", result.id);
    let path = dir.join(&file_name);
    let tmp_path = dir.join(format!("{}.tmp", result.id));

    // Serialize result
    let json = serde_json::to_string(result)
        .map_err(|e| format!("Failed to serialize backtest: {e}"))?;

    // Atomic write: tmp + rename
    tokio::fs::write(&tmp_path, json.as_bytes())
        .await
        .map_err(|e| format!("Failed to write tmp file: {e}"))?;
    tokio::fs::rename(&tmp_path, &path)
        .await
        .map_err(|e| format!("Failed to rename tmp file: {e}"))?;

    // Update index
    let entry = BacktestIndexEntry {
        id: result.id,
        strategy_name: strategy_name.to_string(),
        ticker: ticker.to_string(),
        started_at_ms,
        net_pnl_usd: result.metrics.net_pnl_usd,
        total_trades: result.metrics.total_trades,
        win_rate: result.metrics.win_rate,
        file_name,
    };
    update_index(&dir, entry).await?;

    log::info!(
        "Backtest {} saved to {}",
        result.id,
        path.display()
    );
    Ok(path)
}

/// Load all persisted backtest results from disk.
///
/// Tries the index first; falls back to scanning the directory
/// if the index is missing or corrupt.
pub async fn load_all_backtest_results()
    -> Result<
        Vec<(BacktestIndexEntry, Arc<backtest::BacktestResult>)>,
        String,
    >
{
    let dir = backtests_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let index = load_or_rebuild_index(&dir).await?;
    let mut results = Vec::with_capacity(index.entries.len());

    for entry in &index.entries {
        let path = dir.join(&entry.file_name);
        match tokio::fs::read_to_string(&path).await {
            Ok(json) => {
                match serde_json::from_str::<backtest::BacktestResult>(
                    &json,
                ) {
                    Ok(result) => {
                        results
                            .push((entry.clone(), Arc::new(result)));
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to deserialize {}: {e}",
                            entry.file_name
                        );
                    }
                }
            }
            Err(e) => {
                log::warn!(
                    "Failed to read {}: {e}",
                    entry.file_name
                );
            }
        }
    }

    log::info!(
        "Loaded {} persisted backtests from disk",
        results.len()
    );
    Ok(results)
}

/// Delete a backtest result from disk and update the index.
pub async fn delete_backtest_result(id: Uuid) -> Result<(), String> {
    let dir = backtests_dir();
    let path = dir.join(format!("{id}.json"));

    if path.exists() {
        tokio::fs::remove_file(&path)
            .await
            .map_err(|e| {
                format!("Failed to delete {}: {e}", path.display())
            })?;
    }

    // Update index: remove the entry
    let index_path = dir.join("index.json");
    if index_path.exists() {
        if let Ok(data) =
            tokio::fs::read_to_string(&index_path).await
        {
            if let Ok(mut index) =
                serde_json::from_str::<BacktestIndex>(&data)
            {
                index.entries.retain(|e| e.id != id);
                if let Ok(json) =
                    serde_json::to_string_pretty(&index)
                {
                    let _ =
                        tokio::fs::write(&index_path, json).await;
                }
            }
        }
    }

    log::info!("Deleted backtest {id} from disk");
    Ok(())
}

/// Append an entry to the index file.
async fn update_index(
    dir: &std::path::Path,
    entry: BacktestIndexEntry,
) -> Result<(), String> {
    let index_path = dir.join("index.json");
    let mut index = if index_path.exists() {
        match tokio::fs::read_to_string(&index_path).await {
            Ok(data) => {
                serde_json::from_str::<BacktestIndex>(&data)
                    .unwrap_or(BacktestIndex {
                        version: 1,
                        entries: Vec::new(),
                    })
            }
            Err(_) => BacktestIndex {
                version: 1,
                entries: Vec::new(),
            },
        }
    } else {
        BacktestIndex {
            version: 1,
            entries: Vec::new(),
        }
    };

    // Replace existing entry or append
    if let Some(pos) =
        index.entries.iter().position(|e| e.id == entry.id)
    {
        index.entries[pos] = entry;
    } else {
        index.entries.push(entry);
    }

    let json = serde_json::to_string_pretty(&index)
        .map_err(|e| format!("Failed to serialize index: {e}"))?;
    tokio::fs::write(&index_path, json)
        .await
        .map_err(|e| format!("Failed to write index: {e}"))?;
    Ok(())
}

/// Load the index, or rebuild it by scanning for .json files.
async fn load_or_rebuild_index(
    dir: &std::path::Path,
) -> Result<BacktestIndex, String> {
    let index_path = dir.join("index.json");

    // Try loading existing index
    if index_path.exists() {
        if let Ok(data) =
            tokio::fs::read_to_string(&index_path).await
        {
            if let Ok(index) =
                serde_json::from_str::<BacktestIndex>(&data)
            {
                return Ok(index);
            }
        }
        log::warn!(
            "Index file corrupt, rebuilding from directory scan"
        );
    }

    // Rebuild by scanning
    let mut entries = Vec::new();
    let mut read_dir = tokio::fs::read_dir(dir)
        .await
        .map_err(|e| format!("Failed to read backtests dir: {e}"))?;

    while let Ok(Some(entry)) = read_dir.next_entry().await {
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();

        if !file_name.ends_with(".json")
            || file_name == "index.json"
        {
            continue;
        }

        if let Ok(data) = tokio::fs::read_to_string(&path).await {
            if let Ok(result) =
                serde_json::from_str::<backtest::BacktestResult>(
                    &data,
                )
            {
                entries.push(BacktestIndexEntry {
                    id: result.id,
                    strategy_name: result.strategy_name.clone(),
                    ticker: result
                        .config
                        .ticker
                        .as_str()
                        .to_string(),
                    started_at_ms: result.run_started_at_ms,
                    net_pnl_usd: result.metrics.net_pnl_usd,
                    total_trades: result.metrics.total_trades,
                    win_rate: result.metrics.win_rate,
                    file_name,
                });
            }
        }
    }

    let index = BacktestIndex {
        version: 1,
        entries,
    };

    // Write rebuilt index
    if let Ok(json) = serde_json::to_string_pretty(&index) {
        let _ = tokio::fs::write(&index_path, json).await;
    }

    Ok(index)
}
