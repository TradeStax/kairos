//! Layout Loading — startup deserialization from `app-state.json`.
//!
//! Handles reading persisted state and reconstructing the runtime `LayoutManager`
//! from it. Called once at application startup.
//!
//! ## Counterpart
//! The save path is [`crate::app::layout::dashboard::Kairos::save_state_to_disk`].
//! Loading happens once at startup; saving happens once at controlled exit.

use crate::modals::layout::LayoutManager;
use crate::screen::dashboard::Dashboard;

use super::runtime::{Layout, LayoutId, SavedState, configuration};

pub fn load_saved_state_without_registry() -> SavedState {
    let downloaded_tickers =
        std::sync::Arc::new(std::sync::Mutex::new(data::DownloadedTickersRegistry::new()));
    let state_dir = crate::infra::platform::data_path(None);
    match crate::persistence::load_state(state_dir.as_path(), "app-state.json") {
        Ok(state) => {
            let layout_manager = rebuild_layout_manager(&state, downloaded_tickers.clone());

            SavedState {
                theme: state.selected_theme,
                custom_theme: state.custom_theme,
                layout_manager,
                main_window: state.main_window,
                timezone: state.timezone,
                sidebar: state.sidebar,
                scale_factor: state.scale_factor,
                downloaded_tickers: state.downloaded_tickers,
                data_feeds: state.data_feeds,
                ai_preferences: state.ai_preferences,
                auto_update: state.auto_update,
            }
        }
        Err(e) => {
            log::error!(
                "Failed to load/find layout state: {}. Starting with a new layout.",
                e
            );

            SavedState::default_with_service(downloaded_tickers)
        }
    }
}

/// Rebuild the runtime `LayoutManager` from persisted `AppState`.
fn rebuild_layout_manager(
    state: &crate::persistence::AppState,
    downloaded_tickers: std::sync::Arc<std::sync::Mutex<data::DownloadedTickersRegistry>>,
) -> LayoutManager {
    let persisted = &state.layout_manager;
    let data_index = std::sync::Arc::new(std::sync::Mutex::new(data::DataIndex::new()));

    // Seed the data_index from the persisted downloaded_tickers registry
    // so that charts can load before a feed reconnect triggers a full scan.
    {
        let registry = data::lock_or_recover(&downloaded_tickers);
        let mut idx = data::lock_or_recover(&data_index);
        let sentinel_feed = uuid::Uuid::nil();
        for ticker_str in registry.list_tickers() {
            if let Some(range) = registry.get_range_by_ticker_str(&ticker_str) {
                let mut dates = std::collections::BTreeSet::new();
                for d in range.dates() {
                    dates.insert(d);
                }
                idx.add_contribution(
                    data::DataKey {
                        ticker: ticker_str,
                        schema: "trades".to_string(),
                    },
                    sentinel_feed,
                    dates,
                    false,
                );
            }
        }
    }

    if persisted.layouts.is_empty() {
        log::info!("No persisted layouts found, creating default");
        return LayoutManager::new(data_index);
    }

    let mut runtime_layouts = Vec::with_capacity(persisted.layouts.len());
    let mut active_uid = None;

    for saved in &persisted.layouts {
        // The runtime LayoutId::unique is an ephemeral session-local UUID.
        // It is NOT persisted. The stable identity across sessions is `saved.name`.
        // This UUID is used only for O(1) lookup in LayoutManager::get(uid) within
        // one session. Do NOT add a uuid field to the persisted format and expect
        // this value to be stable across restarts.
        let uid = uuid::Uuid::new_v4();
        let layout_id = LayoutId {
            unique: uid,
            name: saved.name.clone(),
        };

        if persisted.active_layout.as_deref() == Some(&saved.name) {
            active_uid = Some(uid);
        }

        let mut popout_windows = Vec::new();
        for (pane, window_spec) in &saved.dashboard.popout {
            popout_windows.push((configuration(pane.clone()), window_spec.clone()));
        }

        let dashboard = Dashboard::from_config(
            configuration(saved.dashboard.pane.clone()),
            popout_windows,
            None,
            data_index.clone(),
        );

        runtime_layouts.push(Layout {
            id: layout_id,
            dashboard,
        });
    }

    log::info!(
        "Restored {} layout(s) from persisted state",
        runtime_layouts.len()
    );

    LayoutManager::from_saved(runtime_layouts, active_uid, data_index)
}
