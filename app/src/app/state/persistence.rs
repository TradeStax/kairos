//! Persistent data state: layout manager, downloaded ticker registry, data index, ticker metadata.

use crate::modals::LayoutManager;
use data::{FuturesTicker, FuturesTickerInfo};
use rustc_hash::FxHashMap;

pub(crate) struct PersistenceState {
    pub(crate) layout_manager: LayoutManager,
    pub(crate) downloaded_tickers:
        std::sync::Arc<std::sync::Mutex<data::DownloadedTickersRegistry>>,
    pub(crate) data_index: std::sync::Arc<std::sync::Mutex<data::DataIndex>>,
    /// Precomputed date range labels for the ticker list UI.
    /// Keys are ticker strings (e.g. "NQ.c.0"), values are formatted ranges.
    pub(crate) ticker_ranges: std::collections::HashMap<String, String>,
    pub(crate) tickers_info: FxHashMap<FuturesTicker, FuturesTickerInfo>,
    /// Auto-update preferences (check interval, skipped versions, etc.)
    pub(crate) auto_update_prefs: crate::persistence::AutoUpdatePreferences,
}
