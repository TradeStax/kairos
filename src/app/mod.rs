pub(crate) mod globals;
pub(crate) mod messages;
pub(crate) mod services;
mod sidebar_view;
mod state;
mod subscriptions;
pub(crate) mod ticker_registry;
mod update;
mod view;

pub(crate) use messages::*;
pub(crate) use ticker_registry::*;

use crate::components;
use crate::components::display::toast::{self, Toast};
use crate::components::display::tooltip::tooltip;
use crate::modals::{LayoutManager, ThemeEditor};
use crate::modals::{main_dialog_modal, positioned_overlay};
use crate::screen::dashboard::{self, Dashboard};
use crate::style::tokens;
use crate::{split_column, style, window};
use data::{sidebar, state::WindowSpec};

use data::FeedId;
use exchange::{FuturesTicker, FuturesTickerInfo, FuturesVenue};
use iced::{
    Alignment, Element, Length, Subscription, Task, padding,
    widget::{
        button, column, container, pane_grid, pick_list, row, rule, scrollable, text,
        tooltip::Position as TooltipPosition,
    },
};
use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::vec;

pub(super) const APP_NAME: &str = "Kairos";

pub struct Kairos {
    pub(crate) main_window: window::Window,
    pub(crate) sidebar: dashboard::Sidebar,
    pub(crate) tickers_info: FxHashMap<FuturesTicker, FuturesTickerInfo>,
    pub(crate) layout_manager: LayoutManager,
    pub(crate) theme_editor: ThemeEditor,
    pub(crate) data_management_panel: crate::modals::download::DataManagementPanel,
    pub(crate) connections_menu: crate::modals::connections::ConnectionsMenu,
    pub(crate) data_feeds_modal: crate::modals::data_feeds::DataFeedsModal,
    pub(crate) api_key_setup_modal: Option<crate::modals::download::ApiKeySetupModal>,
    pub(crate) historical_download_modal: Option<crate::modals::download::HistoricalDownloadModal>,
    pub(crate) historical_download_id: Option<uuid::Uuid>,
    pub(crate) data_feed_manager: std::sync::Arc<std::sync::Mutex<data::DataFeedManager>>,
    pub(crate) confirm_dialog:
        Option<crate::components::overlay::confirm_dialog::ConfirmDialog<Message>>,
    // Service layer (optional - None when API key not configured)
    pub(crate) market_data_service: Option<std::sync::Arc<data::MarketDataService>>,
    #[cfg(feature = "options")]
    pub(crate) options_service: Option<std::sync::Arc<data::services::OptionsDataService>>,
    pub(crate) replay_engine:
        Option<std::sync::Arc<tokio::sync::Mutex<data::services::ReplayEngine>>>,
    pub(crate) replay_manager: crate::modals::replay::ReplayManager,
    pub(crate) menu_bar: crate::components::chrome::menu_bar::MenuBar,
    // Rithmic connection state
    pub(crate) rithmic_client: Option<std::sync::Arc<tokio::sync::Mutex<exchange::RithmicClient>>>,
    pub(crate) rithmic_trade_repo: Option<std::sync::Arc<exchange::RithmicTradeRepository>>,
    pub(crate) rithmic_depth_repo: Option<std::sync::Arc<exchange::RithmicDepthRepository>>,
    pub(crate) rithmic_feed_id: Option<FeedId>,
    // User preferences
    pub(crate) ui_scale_factor: data::ScaleFactor,
    pub(crate) timezone: data::UserTimezone,
    pub(crate) theme: data::Theme,
    pub(crate) notifications: Vec<Toast>,
    pub(crate) downloaded_tickers:
        std::sync::Arc<std::sync::Mutex<data::DownloadedTickersRegistry>>,
}

impl Kairos {
    pub fn new() -> (Self, Task<Message>) {
        // Initialize script engine and load indicator scripts
        services::initialize_script_registry();

        // Initialize services
        let market_data_result = services::initialize_market_data_service();
        let market_data_service = market_data_result.as_ref().map(|r| r.service.clone());
        let replay_engine = services::create_replay_engine(market_data_result.as_ref());
        #[cfg(feature = "options")]
        let (options_service, _gex_service) = services::initialize_options_services();

        // Load saved state first to get persisted registry
        let saved_state_temp =
            crate::layout::load_saved_state_without_registry(market_data_service.clone());

        // Create THE SINGLE shared Arc<Mutex<>> with loaded registry data
        let downloaded_tickers = std::sync::Arc::new(std::sync::Mutex::new(
            saved_state_temp.downloaded_tickers.clone(),
        ));

        // Re-create layout manager with the shared Arc
        let layout_manager = crate::modals::LayoutManager::new(
            market_data_service.clone(),
            downloaded_tickers.clone(),
            saved_state_temp.sidebar.date_range_preset,
        );

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

        // Ticker info starts empty - tickers only appear after the user
        // connects to a data feed via the connections menu.
        let tickers_info = FxHashMap::default();
        log::info!("Ticker list empty until a data feed is connected");

        let sidebar = dashboard::Sidebar::new(&saved_state);

        let mut state = Self {
            main_window: window::Window {
                is_maximized: true,
                ..window::Window::new(main_window_id)
            },
            layout_manager: saved_state.layout_manager,
            theme_editor: ThemeEditor::new(saved_state.custom_theme),
            data_management_panel: crate::modals::download::DataManagementPanel::new(),
            connections_menu: crate::modals::connections::ConnectionsMenu::new(),
            data_feeds_modal: crate::modals::data_feeds::DataFeedsModal::new(),
            api_key_setup_modal: None,
            historical_download_modal: None,
            historical_download_id: None,
            data_feed_manager,
            sidebar,
            tickers_info,
            confirm_dialog: None,
            rithmic_client: None,
            rithmic_trade_repo: None,
            rithmic_depth_repo: None,
            rithmic_feed_id: None,
            market_data_service,
            #[cfg(feature = "options")]
            options_service,
            replay_engine,
            replay_manager: crate::modals::replay::ReplayManager::new(),
            menu_bar: crate::components::chrome::menu_bar::MenuBar::new(),
            timezone: saved_state.timezone,
            ui_scale_factor: saved_state.scale_factor,
            theme: saved_state.theme,
            notifications: vec![],
            downloaded_tickers: downloaded_tickers.clone(),
        };

        let load_layout = if let Some(active_layout_id) = state
            .layout_manager
            .active_layout_id()
            .or_else(|| state.layout_manager.layouts.first().map(|l| &l.id))
        {
            state.load_layout(active_layout_id.unique, main_window_id)
        } else {
            log::error!("No layouts available at startup");
            Task::none()
        };

        // Auto-connect feeds that have auto_connect enabled
        {
            let mut feed_manager = state
                .data_feed_manager
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let secrets = crate::infra::secrets::SecretsManager::new();

            let auto_connect_ids: Vec<data::FeedId> = feed_manager
                .feeds()
                .iter()
                .filter(|f| f.auto_connect && f.enabled)
                .map(|f| f.id)
                .collect();

            for fid in &auto_connect_ids {
                if let Some(feed) = feed_manager.get(*fid) {
                    let has_key = match feed.provider {
                        data::FeedProvider::Databento => {
                            secrets.has_api_key(data::config::secrets::ApiProvider::Databento)
                        }
                        data::FeedProvider::Rithmic => {
                            secrets.has_api_key(data::config::secrets::ApiProvider::Rithmic)
                        }
                    };
                    if has_key {
                        feed_manager.set_status(*fid, data::FeedStatus::Connected);
                        log::info!("Auto-connected feed {} on startup", fid);
                    }
                }
            }

            // Populate ticker list for auto-connected feeds
            if !auto_connect_ids.is_empty() {
                let ticker_symbols: std::collections::HashSet<String> = state
                    .downloaded_tickers
                    .lock()
                    .unwrap()
                    .list_tickers()
                    .into_iter()
                    .collect();
                if !ticker_symbols.is_empty() {
                    state.tickers_info = build_tickers_info(ticker_symbols);
                }
            }

            state.data_feeds_modal.sync_snapshot(&feed_manager);
            state.connections_menu.sync_snapshot(&feed_manager);
        }

        (state, open_main_window.discard().chain(load_layout))
    }

    pub fn title(&self, _window: window::Id) -> String {
        if let Some(id) = self.layout_manager.active_layout_id() {
            format!("{} [{}]", APP_NAME, id.name)
        } else {
            APP_NAME.to_string()
        }
    }

    pub fn theme(&self, _window: window::Id) -> iced_core::Theme {
        crate::style::theme_bridge::theme_to_iced(&self.theme)
    }

    pub fn scale_factor(&self, _window: window::Id) -> f32 {
        self.ui_scale_factor.into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        subscriptions::build_subscription(self.replay_manager.is_dragging)
    }

}

