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

use crate::components::display::toast::Toast;
use crate::modals::{LayoutManager, ThemeEditor};
use crate::screen::dashboard;
use crate::window;

use data::FeedId;
use exchange::{FuturesTicker, FuturesTickerInfo};
use iced::{Subscription, Task};
use rustc_hash::FxHashMap;
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
    pub(crate) data_index: std::sync::Arc<std::sync::Mutex<data::DataIndex>>,
    /// Precomputed date range labels for the ticker list UI.
    /// Keys are ticker strings (e.g. "NQ.c.0"), values are formatted ranges.
    pub(crate) ticker_ranges: std::collections::HashMap<String, String>,
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
            &downloaded_tickers
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
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
        let sidebar = dashboard::Sidebar::new(&saved_state);
        let ticker_ranges = Self::build_ticker_ranges(&data_index);

        let state = Self {
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
            market_data_service: None,
            #[cfg(feature = "options")]
            options_service: None,
            replay_engine: None,
            replay_manager: crate::modals::replay::ReplayManager::new(),
            menu_bar: crate::components::chrome::menu_bar::MenuBar::new(),
            timezone: saved_state.timezone,
            ui_scale_factor: saved_state.scale_factor,
            theme: saved_state.theme,
            notifications: vec![],
            downloaded_tickers: downloaded_tickers.clone(),
            data_index: data_index.clone(),
            ticker_ranges,
        };

        // Kick off async service init; the UI is responsive in the meantime.
        let init_services = Task::perform(
            services::initialize_all_services(),
            Message::ServicesReady,
        );

        let open_window = open_main_window.discard();
        (state, Task::batch([open_window, init_services]))
    }

    /// Seed the DataIndex from the persisted DownloadedTickersRegistry.
    pub(crate) fn seed_data_index_from_registry(
        registry: &data::DownloadedTickersRegistry,
        data_index: &std::sync::Arc<std::sync::Mutex<data::DataIndex>>,
    ) {
        let mut idx = data_index.lock().unwrap_or_else(|e| e.into_inner());
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
                    Self::REGISTRY_SENTINEL_FEED,
                    dates,
                    false,
                );
            }
        }
    }

    /// Auto-connect feeds with `auto_connect` enabled and an API key present.
    /// Returns tasks for async cache scans.
    pub(crate) fn auto_connect_feeds(
        state: &mut Self,
        secrets: &crate::infra::secrets::SecretsManager,
    ) -> Vec<Task<Message>> {
        let mut scan_tasks: Vec<Task<Message>> = Vec::new();
        let mut feed_manager = state
            .data_feed_manager
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        let auto_connect_ids: Vec<data::FeedId> = feed_manager
            .feeds()
            .iter()
            .filter(|f| f.auto_connect && f.enabled)
            .map(|f| f.id)
            .collect();

        for fid in &auto_connect_ids {
            let feed_snapshot = feed_manager.get(*fid).map(|f| {
                (f.provider, f.dataset_info().cloned())
            });

            let Some((provider, dataset_info)) = feed_snapshot else {
                continue;
            };

            let has_key = match provider {
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

                if provider == data::FeedProvider::Databento {
                    if let Some(info) = &dataset_info {
                        let mut dates = std::collections::BTreeSet::new();
                        for d in info.date_range.dates() {
                            dates.insert(d);
                        }
                        let mut idx = state
                            .data_index
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        idx.add_contribution(
                            data::DataKey {
                                ticker: info.ticker.clone(),
                                schema: "trades".to_string(),
                            },
                            *fid,
                            dates,
                            false,
                        );
                    }

                    let cache_root =
                        crate::infra::platform::data_path(Some("cache/databento"));
                    let feed_id = *fid;
                    scan_tasks.push(Task::perform(
                        async move {
                            exchange::scan_databento_cache(&cache_root, feed_id).await
                        },
                        Message::DataIndexRebuilt,
                    ));
                }
            }
        }

        scan_tasks
    }

    /// Wire up services after async init completes, load the layout, and auto-connect feeds.
    pub(crate) fn handle_services_ready(
        &mut self,
        result: services::AllServicesResult,
    ) -> Task<Message> {
        let market_data_service = result.market_data.as_ref().map(|r| r.service.clone());
        let replay_engine = services::create_replay_engine(result.market_data.as_ref());

        self.market_data_service = market_data_service.clone();
        self.replay_engine = replay_engine;

        #[cfg(feature = "options")]
        {
            self.options_service = result.options;
        }

        // Update layout manager with the live service
        self.layout_manager.update_shared_state(
            market_data_service,
            self.data_index.clone(),
        );

        // Load the active layout now that services are ready
        let main_window_id = self.main_window.id;
        let load_layout = if let Some(active_layout_id) = self
            .layout_manager
            .active_layout_id()
            .or_else(|| self.layout_manager.layouts.first().map(|l| &l.id))
        {
            self.load_layout(active_layout_id.unique, main_window_id)
        } else {
            log::error!("No layouts available at startup");
            Task::none()
        };

        // Auto-connect feeds
        let secrets = crate::infra::secrets::SecretsManager::new();
        let mut scan_tasks = Self::auto_connect_feeds(self, &secrets);

        // Populate tickers from DataIndex
        let tickers: std::collections::HashSet<String> = self
            .data_index
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .available_tickers()
            .into_iter()
            .collect();
        if !tickers.is_empty() {
            self.tickers_info = build_tickers_info(tickers);
            self.ticker_ranges = Self::build_ticker_ranges(&self.data_index);
            log::info!(
                "Populated {} tickers from DataIndex at startup",
                self.tickers_info.len()
            );
        }

        {
            let feed_manager = self
                .data_feed_manager
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            self.data_feeds_modal.sync_snapshot(&feed_manager);
            self.connections_menu.sync_snapshot(&feed_manager);
        }

        let mut all_tasks = vec![load_layout];
        all_tasks.append(&mut scan_tasks);
        Task::batch(all_tasks)
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

    /// Build formatted date-range labels from the shared DataIndex.
    pub(crate) fn build_ticker_ranges(
        data_index: &std::sync::Arc<std::sync::Mutex<data::DataIndex>>,
    ) -> std::collections::HashMap<String, String> {
        let idx = data_index.lock().unwrap_or_else(|e| e.into_inner());
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

