//! Cache management modal — view and delete cached Databento data files.

pub mod view;

use chrono::NaiveDate;
use std::collections::HashSet;
use std::path::PathBuf;

// ── Types ────────────────────────────────────────────────────────────

/// A single cached data file on disk.
#[derive(Debug, Clone)]
pub struct CachedEntry {
    pub symbol: String,
    pub schema: String,
    pub date: NaiveDate,
    pub size_bytes: u64,
    pub path: PathBuf,
}

/// What should be deleted when the user confirms.
#[derive(Debug, Clone)]
pub enum DeleteTarget {
    All,
    Selected,
    #[allow(dead_code)]
    Symbol(String),
    #[allow(dead_code)]
    SchemaGroup(String, String),
}

impl DeleteTarget {
    pub fn confirm_message(&self, count: usize) -> String {
        match self {
            DeleteTarget::All => "Delete ALL cached data? This cannot be undone.".into(),
            DeleteTarget::Selected => {
                format!(
                    "Delete {} selected file{}?",
                    count,
                    if count == 1 { "" } else { "s" }
                )
            }
            DeleteTarget::Symbol(s) => format!("Delete all cached data for {}?", s),
            DeleteTarget::SchemaGroup(sym, schema) => {
                format!("Delete all {} data for {}?", schema, sym)
            }
        }
    }
}

// ── Messages ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum CacheManagementMessage {
    CacheScanned(Result<Vec<CachedEntry>, String>),
    ToggleSymbolExpanded(String),
    ToggleSchemaExpanded(String, String),
    ToggleEntrySelected(usize),
    #[allow(dead_code)]
    SelectAllInSymbol(String),
    #[allow(dead_code)]
    SelectAllInSchema(String, String),
    DeselectAll,
    SearchChanged(String),
    RequestDelete(DeleteTarget),
    ConfirmDelete,
    CancelDelete,
    DeleteComplete(Result<usize, String>),
    Close,
}

// ── Modal state ──────────────────────────────────────────────────────

pub struct CacheManagementModal {
    pub entries: Vec<CachedEntry>,
    pub loading: bool,
    pub total_size: u64,
    pub total_files: usize,
    pub expanded_symbols: HashSet<String>,
    pub expanded_schemas: HashSet<(String, String)>,
    pub selected_entries: HashSet<usize>,
    pub deleting: bool,
    pub search_query: String,
    pub confirm_delete: Option<DeleteTarget>,
}

impl CacheManagementModal {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            loading: false,
            total_size: 0,
            total_files: 0,
            expanded_symbols: HashSet::new(),
            expanded_schemas: HashSet::new(),
            selected_entries: HashSet::new(),
            deleting: false,
            search_query: String::new(),
            confirm_delete: None,
        }
    }

    /// Reset state when modal closes.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Update modal state from a scan result.
    pub fn apply_scan(&mut self, entries: Vec<CachedEntry>) {
        self.total_size = entries.iter().map(|e| e.size_bytes).sum();
        self.total_files = entries.len();
        self.entries = entries;
        self.loading = false;
        self.deleting = false;
        self.selected_entries.clear();
        self.confirm_delete = None;
    }

    /// Process a message, returning an optional async action.
    pub fn update(&mut self, msg: CacheManagementMessage) -> Option<Action> {
        match msg {
            CacheManagementMessage::CacheScanned(result) => match result {
                Ok(entries) => {
                    self.apply_scan(entries);
                    None
                }
                Err(e) => {
                    log::error!("Cache scan failed: {}", e);
                    self.loading = false;
                    None
                }
            },
            CacheManagementMessage::ToggleSymbolExpanded(sym) => {
                if !self.expanded_symbols.remove(&sym) {
                    self.expanded_symbols.insert(sym);
                }
                None
            }
            CacheManagementMessage::ToggleSchemaExpanded(sym, schema) => {
                let key = (sym, schema);
                if !self.expanded_schemas.remove(&key) {
                    self.expanded_schemas.insert(key);
                }
                None
            }
            CacheManagementMessage::ToggleEntrySelected(idx) => {
                if !self.selected_entries.remove(&idx) {
                    self.selected_entries.insert(idx);
                }
                None
            }
            CacheManagementMessage::SelectAllInSymbol(sym) => {
                let indices: Vec<usize> = self
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.symbol == sym)
                    .map(|(i, _)| i)
                    .collect();
                self.selected_entries.extend(indices);
                None
            }
            CacheManagementMessage::SelectAllInSchema(sym, schema) => {
                let indices: Vec<usize> = self
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.symbol == sym && e.schema == schema)
                    .map(|(i, _)| i)
                    .collect();
                self.selected_entries.extend(indices);
                None
            }
            CacheManagementMessage::DeselectAll => {
                self.selected_entries.clear();
                None
            }
            CacheManagementMessage::SearchChanged(query) => {
                self.search_query = query;
                None
            }
            CacheManagementMessage::RequestDelete(target) => {
                self.confirm_delete = Some(target);
                None
            }
            CacheManagementMessage::ConfirmDelete => {
                let target = self.confirm_delete.take()?;
                self.deleting = true;
                let paths = self.paths_for_target(&target);
                if paths.is_empty() {
                    self.deleting = false;
                    return None;
                }
                match target {
                    DeleteTarget::All => Some(Action::ClearAll),
                    _ => Some(Action::DeleteEntries(paths)),
                }
            }
            CacheManagementMessage::CancelDelete => {
                self.confirm_delete = None;
                None
            }
            CacheManagementMessage::DeleteComplete(result) => {
                match &result {
                    Ok(count) => log::info!("Deleted {} cache files", count),
                    Err(e) => log::error!("Cache deletion error: {}", e),
                }
                // Trigger rescan
                self.loading = true;
                self.deleting = false;
                Some(Action::ScanCache)
            }
            CacheManagementMessage::Close => Some(Action::Close),
        }
    }

    /// Entries filtered by the current search query.
    pub fn filtered_entries(&self) -> Vec<(usize, &CachedEntry)> {
        let query = self.search_query.to_lowercase();
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                query.is_empty()
                    || e.symbol.to_lowercase().contains(&query)
                    || e.schema.to_lowercase().contains(&query)
            })
            .collect()
    }

    /// Collect file paths to delete for a given target.
    fn paths_for_target(&self, target: &DeleteTarget) -> Vec<PathBuf> {
        match target {
            DeleteTarget::All => self.entries.iter().map(|e| e.path.clone()).collect(),
            DeleteTarget::Selected => self
                .selected_entries
                .iter()
                .filter_map(|&i| self.entries.get(i))
                .map(|e| e.path.clone())
                .collect(),
            DeleteTarget::Symbol(sym) => self
                .entries
                .iter()
                .filter(|e| &e.symbol == sym)
                .map(|e| e.path.clone())
                .collect(),
            DeleteTarget::SchemaGroup(sym, schema) => self
                .entries
                .iter()
                .filter(|e| &e.symbol == sym && &e.schema == schema)
                .map(|e| e.path.clone())
                .collect(),
        }
    }

    /// Number of currently selected entries.
    pub fn selected_count(&self) -> usize {
        self.selected_entries.len()
    }
}

// ── Actions ──────────────────────────────────────────────────────────

/// Async actions the host update loop should perform.
#[derive(Debug)]
pub enum Action {
    ScanCache,
    DeleteEntries(Vec<PathBuf>),
    ClearAll,
    Close,
}

// ── Async helpers ────────────────────────────────────────────────────

/// Cache root directories to scan (Databento adapter + DataEngine).
fn cache_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    // Databento adapter cache
    {
        let config = data::DatabentoConfig::default();
        roots.push(config.cache_dir);
    }

    // DataEngine cache (Rithmic data + engine-level Databento)
    let engine_root = crate::infra::platform::data_path(Some("cache"));
    roots.push(engine_root);

    roots
}

/// Scan all cache directories and return cached entries.
///
/// Scans both the Databento adapter cache and the DataEngine cache
/// (which holds Rithmic data).
pub async fn scan_databento_cache() -> Result<Vec<CachedEntry>, String> {
    use data::cache::{CacheProvider, CacheSchema, CacheStore};

    let mut entries = Vec::new();
    let mut seen_paths = HashSet::new();

    for root in cache_roots() {
        if !tokio::fs::try_exists(&root).await.unwrap_or(false) {
            continue;
        }

        let store = CacheStore::new(root);

        for provider in [CacheProvider::Databento, CacheProvider::Rithmic] {
            let symbols = store.list_symbols(provider).await;
            for symbol in symbols {
                for schema in [CacheSchema::Trades, CacheSchema::Depth, CacheSchema::Ohlcv] {
                    let dates = store.list_dates(provider, &symbol, schema).await;
                    for date in dates {
                        let path = store.day_file_path(provider, &symbol, schema, date);
                        // Deduplicate across cache roots
                        if !seen_paths.insert(path.clone()) {
                            continue;
                        }
                        let size = tokio::fs::metadata(&path)
                            .await
                            .map(|m| m.len())
                            .unwrap_or(0);
                        entries.push(CachedEntry {
                            symbol: symbol.clone(),
                            schema: schema.dir_name().to_string(),
                            date,
                            size_bytes: size,
                            path,
                        });
                    }
                }
            }
        }
    }

    // Sort by symbol, then schema, then date
    entries.sort_by(|a, b| {
        a.symbol
            .cmp(&b.symbol)
            .then(a.schema.cmp(&b.schema))
            .then(a.date.cmp(&b.date))
    });

    Ok(entries)
}

/// Delete specific cache files and clean up empty parent directories.
pub async fn delete_cache_entries(paths: Vec<PathBuf>) -> Result<usize, String> {
    let mut deleted = 0;
    let mut parent_dirs = HashSet::new();

    for path in &paths {
        if let Some(parent) = path.parent() {
            parent_dirs.insert(parent.to_path_buf());
        }
        match tokio::fs::remove_file(path).await {
            Ok(()) => deleted += 1,
            Err(e) => log::warn!("Failed to delete {:?}: {}", path, e),
        }
    }

    // Clean up empty parent directories (schema dirs, then symbol dirs)
    for dir in &parent_dirs {
        let _ = try_remove_empty_dir(dir).await;
        if let Some(parent) = dir.parent() {
            let _ = try_remove_empty_dir(parent).await;
        }
    }

    Ok(deleted)
}

/// Delete all cache directories and recreate them.
pub async fn clear_all_cache() -> Result<usize, String> {
    let entries = scan_databento_cache().await?;
    let count = entries.len();

    for root in cache_roots() {
        if tokio::fs::try_exists(&root).await.unwrap_or(false) {
            tokio::fs::remove_dir_all(&root)
                .await
                .map_err(|e| format!("Failed to clear cache at {:?}: {}", root, e))?;
            tokio::fs::create_dir_all(&root)
                .await
                .map_err(|e| format!("Failed to recreate cache dir {:?}: {}", root, e))?;
        }
    }

    Ok(count)
}

/// Remove a directory only if it is empty.
async fn try_remove_empty_dir(dir: &std::path::Path) -> Result<(), ()> {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return Err(());
    };
    // If there's any entry, the dir is not empty
    if entries.next_entry().await.ok().flatten().is_some() {
        return Err(());
    }
    tokio::fs::remove_dir(dir).await.map_err(|_| ())
}

/// Format bytes to a human-readable string.
pub fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.1} {}", size, UNITS[unit_idx])
    }
}
