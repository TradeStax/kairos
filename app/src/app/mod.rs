//! # Kairos Application Layer
//!
//! Elm-architecture orchestration for the Kairos charting platform.
//!
//! ## Struct Organization
//! The `Kairos` struct holds all runtime state. Fields are semantically grouped
//! into state structs:
//! - **ui** — chrome, sidebar, theme, timezone, preferences, notifications
//! - **services** — market data, replay engine (optional — present only when API key configured)
//! - **connections** — Rithmic client, trade/depth repos, data feed manager
//! - **persistence** — layout manager, ticker registry, data index, ticker metadata
//! - **modals** — all overlay/panel state, backtest subsystem
//!
//! ## Message Flow
//! `Message` → `Kairos::update()` (`update/mod.rs`) → domain handlers → `Task<Message>`
//!
//! ## Event Sources
//! Background events (Rithmic streaming, Replay, AI streaming, Download progress, Backtest)
//! are staged through `OnceLock<Arc<Mutex<Vec<T>>>>` buffers in `core/globals.rs` and
//! drained by polling subscriptions in `core/subscriptions.rs`. See those modules for details.

pub(crate) mod backtest;
pub(crate) use backtest::history as backtest_history;
pub(crate) mod core;
pub(crate) mod init;
mod layout;
pub(crate) mod messages;
pub(crate) mod state;
mod update;
mod view;

pub(crate) use messages::{ChartMessage, DownloadMessage, Message, WindowMessage};
#[cfg(feature = "options")]
pub(crate) use messages::OptionsMessage;
pub(crate) use init::ticker_registry::{FUTURES_PRODUCTS, build_tickers_info};

use crate::infra::secrets::SecretsManager;
use crate::modals::ThemeEditor;
use crate::screen::dashboard;
use crate::infra::window;

use iced::{Subscription, Task};
use rustc_hash::FxHashMap;

pub(super) const APP_NAME: &str = "Kairos";

pub struct Kairos {
    pub(crate) main_window: window::Window,
    pub(crate) menu_bar: crate::app::update::menu_bar::MenuBar,
    pub(crate) ui: state::UiState,
    pub(crate) services: state::ServiceState,
    pub(crate) connections: state::ConnectionState,
    pub(crate) persistence: state::PersistenceState,
    pub(crate) modals: state::ModalState,
    /// Shared secrets manager for API key operations (zero-cost ZST).
    pub(crate) secrets: SecretsManager,
}

impl Kairos {
    /// Sentinel UUID used to attribute DataIndex entries from the persisted registry
    /// (no real feed ID, represents locally cached data).
    pub(crate) const REGISTRY_SENTINEL_FEED: uuid::Uuid = uuid::Uuid::nil();

    pub fn new() -> (Self, Task<Message>) {
        // Load saved state (no I/O beyond disk reads for config, no repo init)
        let saved_state_temp =
            crate::layout::load_saved_state_without_registry(None);

        // Create THE SINGLE shared Arc<Mutex<>> with loaded registry data
        let downloaded_tickers = std::sync::Arc::new(std::sync::Mutex::new(
            saved_state_temp.downloaded_tickers.clone(),
        ));

        // Create the shared DataIndex and seed from persisted registry
        let data_index =
            std::sync::Arc::new(std::sync::Mutex::new(data::DataIndex::new()));
        Self::seed_data_index_from_registry(
            &data::lock_or_recover(&downloaded_tickers),
            &data_index,
        );

        // Re-create layout manager with the shared Arc (no service yet — services load async)
        let layout_manager = if saved_state_temp.layout_manager.layouts.is_empty() {
            crate::modals::LayoutManager::new(None, data_index.clone())
        } else {
            let mut lm = saved_state_temp.layout_manager;
            lm.update_shared_state(None, data_index.clone());
            lm
        };

        // Create shared data feed manager
        let data_feed_manager =
            std::sync::Arc::new(std::sync::Mutex::new(saved_state_temp.data_feeds.clone()));

        // Create final SavedState with shared Arc in layout_manager
        let saved_state = crate::layout::SavedState {
            theme: saved_state_temp.theme,
            custom_theme: saved_state_temp.custom_theme,
            layout_manager,
            main_window: saved_state_temp.main_window,
            timezone: saved_state_temp.timezone,
            sidebar: saved_state_temp.sidebar,
            scale_factor: saved_state_temp.scale_factor,
            downloaded_tickers: saved_state_temp.downloaded_tickers,
            data_feeds: saved_state_temp.data_feeds,
            ai_preferences: saved_state_temp.ai_preferences,
        };

        let (main_window_id, open_main_window) = {
            let (position, size) = saved_state.window();
            let config = window::Settings {
                size,
                position,
                maximized: true,
                exit_on_close_request: false,
                ..window::settings()
            };
            window::open(config)
        };

        let tickers_info = FxHashMap::default();
        let ai_preferences = saved_state.ai_preferences.clone();
        let sidebar = dashboard::Sidebar::new(&saved_state);
        let ticker_ranges = Self::build_ticker_ranges(&data_index);

        let strategy_registry = ::backtest::StrategyRegistry::with_built_ins();
        let backtest_launch_modal = crate::screen::backtest::launch::BacktestLaunchModal::new(
            &::backtest::StrategyRegistry::with_built_ins(),
            &data::lock_or_recover(&data_index),
        );

        let state = Self {
            main_window: window::Window {
                is_maximized: true,
                ..window::Window::new(main_window_id)
            },
            menu_bar: crate::app::update::menu_bar::MenuBar::new(),
            ui: state::UiState {
                sidebar,
                title_bar_hovered: false,
                theme: saved_state.theme,
                ui_scale_factor: saved_state.scale_factor,
                timezone: saved_state.timezone,
                ai_preferences,
                notifications: vec![],
                confirm_dialog: None,
            },
            services: state::ServiceState::new(),
            connections: state::ConnectionState::new(data_feed_manager),
            persistence: state::PersistenceState {
                layout_manager: saved_state.layout_manager,
                downloaded_tickers: downloaded_tickers.clone(),
                data_index: data_index.clone(),
                ticker_ranges,
                tickers_info,
            },
            modals: state::ModalState {
                theme_editor: ThemeEditor::new(saved_state.custom_theme),
                data_management_panel: crate::modals::download::DataManagementPanel::new(),
                connections_menu: crate::modals::connections::ConnectionsMenu::new(),
                data_feeds_modal: crate::modals::data_feeds::DataFeedsModal::new(),
                api_key_setup_modal: None,
                historical_download_modal: None,
                historical_download_id: None,
                replay_manager: crate::modals::replay::ReplayManager::new(),
                backtest: state::modals::BacktestState {
                    strategy_registry,
                    backtest_launch_modal,
                    show_backtest_modal: false,
                    backtest_trade_repo: None,
                    backtest_history: backtest_history::BacktestHistory::new(),
                    backtest_manager: crate::screen::backtest::manager::BacktestManager::new(),
                    show_backtest_manager: false,
                },
            },
            secrets: SecretsManager::new(),
        };

        // Kick off async service init; the UI is responsive in the meantime.
        let init_services = Task::perform(
            init::services::initialize_all_services(),
            Message::ServicesReady,
        );

        let open_window = open_main_window.discard();
        (state, Task::batch([open_window, init_services]))
    }

    pub fn title(&self, _window: window::Id) -> String {
        if let Some(id) = self.persistence.layout_manager.active_layout_id() {
            format!("{} [{}]", APP_NAME, id.name)
        } else {
            APP_NAME.to_string()
        }
    }

    pub fn theme(&self, _window: window::Id) -> iced_core::Theme {
        crate::style::theme::theme_to_iced(&self.ui.theme)
    }

    pub fn scale_factor(&self, _window: window::Id) -> f32 {
        self.ui.ui_scale_factor.into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        core::subscriptions::build_subscription(self.modals.replay_manager.is_dragging)
    }

    /// Build formatted date-range labels from the shared DataIndex.
    pub(crate) fn build_ticker_ranges(
        data_index: &std::sync::Arc<std::sync::Mutex<data::DataIndex>>,
    ) -> std::collections::HashMap<String, String> {
        let idx = data::lock_or_recover(data_index);
        idx.ticker_date_ranges()
            .into_iter()
            .map(|(ticker, range)| {
                let label = if range.start == range.end {
                    range.start.format("%b %d").to_string()
                } else {
                    format!(
                        "{} - {}",
                        range.start.format("%b %d"),
                        range.end.format("%b %d"),
                    )
                };
                (ticker, label)
            })
            .collect()
    }
}
