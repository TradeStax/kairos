#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod chart;
mod layout;
mod logger;
mod modal;
mod screen;
mod style;
mod widget;
mod window;

use data::config::theme::default_theme;
use data::{layout::WindowSpec, sidebar, LoadingStatus};
use layout::{LayoutId, configuration};
use modal::{LayoutManager, ThemeEditor, audio::AudioStream};
use modal::{dashboard_modal, main_dialog_modal};
use screen::dashboard::{self, Dashboard};
use widget::{
    confirm_dialog_container,
    toast::{self, Toast},
    tooltip,
};

use iced::{
    Alignment, Element, Subscription, Task, keyboard, padding,
    widget::{
        button, column, container, pane_grid, pick_list, row, rule, scrollable, text,
        tooltip::Position as TooltipPosition,
    },
};
use std::{borrow::Cow, collections::HashMap, vec};

fn main() {
    logger::setup(cfg!(debug_assertions)).expect("Failed to initialize logger");

    // TODO: Re-implement cache cleanup once cache manager is integrated
    // std::thread::spawn(data::cleanup_old_market_data);

    let _ = iced::daemon(Flowsurface::new, Flowsurface::update, Flowsurface::view)
        .settings(iced::Settings {
            antialiasing: true,
            fonts: vec![
                Cow::Borrowed(style::AZERET_MONO_BYTES),
                Cow::Borrowed(style::ICONS_BYTES),
            ],
            default_text_size: iced::Pixels(12.0),
            ..Default::default()
        })
        .title(Flowsurface::title)
        .theme(Flowsurface::theme)
        .scale_factor(Flowsurface::scale_factor)
        .subscription(Flowsurface::subscription)
        .run();
}

struct Flowsurface {
    main_window: window::Window,
    sidebar: dashboard::Sidebar,
    layout_manager: LayoutManager,
    theme_editor: ThemeEditor,
    audio_stream: AudioStream,
    data_management_panel: crate::modal::pane::data_management::DataManagementPanel,
    confirm_dialog: Option<screen::ConfirmDialog<Message>>,
    // Service layer
    market_data_service: std::sync::Arc<data::MarketDataService>,
    options_service: Option<std::sync::Arc<data::services::OptionsDataService>>,
    gex_service: std::sync::Arc<data::services::GexCalculationService>,
    replay_engine: Option<std::sync::Arc<std::sync::Mutex<data::services::ReplayEngine>>>,
    // User preferences
    ui_scale_factor: data::ScaleFactor,
    timezone: data::UserTimezone,
    theme: data::Theme,
    notifications: Vec<Toast>,
    downloaded_tickers: std::sync::Arc<std::sync::Mutex<data::DownloadedTickersRegistry>>,
}

#[derive(Debug, Clone)]
enum Message {
    Sidebar(dashboard::sidebar::Message),
    Dashboard {
        /// If `None`, the active layout is used for the event.
        layout_id: Option<uuid::Uuid>,
        event: dashboard::Message,
    },
    DataManagement(crate::modal::pane::data_management::DataManagementMessage),
    // Async chart data loading
    LoadChartData {
        layout_id: uuid::Uuid,
        pane_id: uuid::Uuid,
        config: data::ChartConfig,
        ticker_info: exchange::FuturesTickerInfo,
    },
    ChartDataLoaded {
        layout_id: uuid::Uuid,
        pane_id: uuid::Uuid,
        result: Result<data::ChartData, String>,
    },
    // Options data loading
    LoadOptionChain {
        pane_id: uuid::Uuid,
        underlying_ticker: String,
        date: chrono::NaiveDate,
    },
    OptionChainLoaded {
        pane_id: uuid::Uuid,
        result: Result<data::domain::OptionChain, String>,
    },
    LoadGexProfile {
        pane_id: uuid::Uuid,
        underlying_ticker: String,
        date: chrono::NaiveDate,
    },
    GexProfileLoaded {
        pane_id: uuid::Uuid,
        result: Result<data::domain::GexProfile, String>,
    },
    // Replay engine events
    ReplayEvent(data::services::ReplayEvent),
    UpdateLoadingStatus,
    // Data management - cost estimation
    EstimateDataCost {
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        schema: exchange::DatabentoSchema,
        date_range: data::DateRange,
    },
    DataCostEstimated {
        pane_id: uuid::Uuid,
        result: Result<(usize, usize, usize, String, f64, Vec<chrono::NaiveDate>), String>, // (total, cached, uncached, gaps, cost, cached_dates)
    },
    // Data management - download
    DownloadData {
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        schema: exchange::DatabentoSchema,
        date_range: data::DateRange,
    },
    DataDownloadProgress {
        pane_id: uuid::Uuid,
        current: usize,
        total: usize,
    },
    DataDownloadComplete {
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        date_range: data::DateRange,
        result: Result<usize, String>, // days_downloaded
    },
    Tick(std::time::Instant),
    WindowEvent(window::Event),
    ExitRequested(HashMap<window::Id, WindowSpec>),
    RestartRequested(HashMap<window::Id, WindowSpec>),
    GoBack,
    DataFolderRequested,
    ThemeSelected(data::Theme),
    ScaleFactorChanged(data::ScaleFactor),
    SetTimezone(data::UserTimezone),
    RemoveNotification(usize),
    ToggleDialogModal(Option<screen::ConfirmDialog<Message>>),
    ThemeEditor(modal::theme_editor::Message),
    Layouts(modal::layout_manager::Message),
    AudioStream(modal::audio::Message),
}

impl Flowsurface {
    fn new() -> (Self, Task<Message>) {
        // Initialize Databento configuration and services FIRST
        let databento_config = match exchange::adapter::databento::DatabentoConfig::from_env() {
            Ok(config) => config,
            Err(e) => {
                log::warn!("Failed to load Databento config from environment: {}, using defaults", e);
                exchange::adapter::databento::DatabentoConfig::default()
            }
        };

        // Create runtime for async repository initialization
        let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");

        // Create repository instances (async)
        let (trade_repo, depth_repo) = rt.block_on(async {
            let trade = exchange::DatabentoTradeRepository::new(databento_config.clone())
                .await
                .expect("Failed to create trade repository");
            let depth = exchange::DatabentoDepthRepository::new(databento_config)
                .await
                .expect("Failed to create depth repository");
            (std::sync::Arc::new(trade), std::sync::Arc::new(depth))
        });

        // Create market data service
        let market_data_service = std::sync::Arc::new(
            data::MarketDataService::new(trade_repo.clone(), depth_repo.clone())
        );

        // Create replay engine for historical data playback
        let replay_engine = Some(std::sync::Arc::new(std::sync::Mutex::new(
            data::services::ReplayEngine::with_default_config(trade_repo, Some(depth_repo))
        )));

        // Initialize options services (Massive API)
        let (options_service, gex_service) = Self::initialize_options_services();

        // Load saved state first to get persisted registry
        let saved_state_temp = layout::load_saved_state_without_registry(market_data_service.clone());

        // Create THE SINGLE shared Arc<Mutex<>> with loaded registry data
        let downloaded_tickers = std::sync::Arc::new(std::sync::Mutex::new(saved_state_temp.downloaded_tickers.clone()));

        // Re-create layout manager with the shared Arc
        let layout_manager = modal::LayoutManager::new(market_data_service.clone(), downloaded_tickers.clone());

        // Create final SavedState with shared Arc in layout_manager
        let saved_state = layout::SavedState {
            theme: saved_state_temp.theme,
            custom_theme: saved_state_temp.custom_theme,
            layout_manager,
            main_window: saved_state_temp.main_window,
            timezone: saved_state_temp.timezone,
            sidebar: saved_state_temp.sidebar,
            scale_factor: saved_state_temp.scale_factor,
            audio_cfg: saved_state_temp.audio_cfg,
            downloaded_tickers: saved_state_temp.downloaded_tickers,
        };

        let (main_window_id, open_main_window) = {
            let (position, size) = saved_state.window();
            let config = window::Settings {
                size,
                position,
                exit_on_close_request: false,
                ..window::settings()
            };
            window::open(config)
        };

        let (sidebar, launch_sidebar) = dashboard::Sidebar::new(&saved_state, downloaded_tickers.clone());

        let mut state = Self {
            main_window: window::Window::new(main_window_id),
            layout_manager: saved_state.layout_manager,
            theme_editor: ThemeEditor::new(saved_state.custom_theme),
            audio_stream: AudioStream::new(saved_state.audio_cfg),
            data_management_panel: crate::modal::pane::data_management::DataManagementPanel::new(),
            sidebar,
            confirm_dialog: None,
            market_data_service,
            options_service,
            gex_service,
            replay_engine,
            timezone: saved_state.timezone,
            ui_scale_factor: saved_state.scale_factor,
            theme: saved_state.theme,
            notifications: vec![],
            downloaded_tickers: downloaded_tickers.clone(),
        };

        let active_layout_id = state.layout_manager.active_layout_id().unwrap_or(
            &state
                .layout_manager
                .layouts
                .first()
                .expect("No layouts available")
                .id,
        );
        let load_layout = state.load_layout(active_layout_id.unique, main_window_id);

        (
            state,
            open_main_window
                .discard()
                .chain(load_layout)
                .chain(launch_sidebar.map(Message::Sidebar)),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // Async chart data loading
            Message::LoadChartData { layout_id, pane_id, config, ticker_info } => {
                let service = self.market_data_service.clone();
                return Task::perform(
                    async move {
                        service.get_chart_data(&config, &ticker_info).await
                            .map_err(|e| e.to_string())
                    },
                    move |result| Message::ChartDataLoaded { layout_id, pane_id, result }
                );
            }
            Message::ChartDataLoaded { layout_id, pane_id, result } => {
                // Forward to dashboard to update the pane
                match result {
                    Ok(chart_data) => {
                        log::info!("Chart data loaded for pane {}: {} trades, {} candles",
                            pane_id, chart_data.trades.len(), chart_data.candles.len());

                        // Forward successful data load to dashboard
                        return Task::done(Message::Dashboard {
                            layout_id: Some(layout_id),
                            event: dashboard::Message::ChartDataLoaded { pane_id, chart_data },
                        });
                    }
                    Err(e) => {
                        log::error!("Failed to load chart data for pane {}: {}", pane_id, e);
                        self.notifications.push(Toast::error(format!("Failed to load chart data: {}", e)));

                        // Set pane status to error
                        return Task::done(Message::Dashboard {
                            layout_id: Some(layout_id),
                            event: dashboard::Message::ChangePaneStatus(
                                pane_id,
                                LoadingStatus::Error { message: e }
                            ),
                        });
                    }
                }
            }
            // Options data loading
            Message::LoadOptionChain { pane_id, underlying_ticker, date } => {
                if let Some(service) = self.options_service.clone() {
                    return Task::perform(
                        async move {
                            service.get_chain_with_greeks(&underlying_ticker, date).await
                                .map_err(|e| e.to_string())
                        },
                        move |result| Message::OptionChainLoaded { pane_id, result }
                    );
                } else {
                    log::warn!("Options service not available - set MASSIVE_API_KEY to enable");
                    self.notifications.push(Toast::error("Options data unavailable - configure MASSIVE_API_KEY".to_string()));
                }
            }
            Message::OptionChainLoaded { pane_id, result } => {
                match result {
                    Ok(chain) => {
                        log::info!("Option chain loaded for pane {}: {} contracts for {}",
                            pane_id, chain.contract_count(), chain.underlying_ticker);
                        self.notifications.push(Toast::new(toast::Notification::Info(format!(
                            "Loaded {} option contracts",
                            chain.contract_count()
                        ))));
                        // TODO: Dashboard needs handler for option chain display
                    }
                    Err(e) => {
                        log::error!("Failed to load option chain for pane {}: {}", pane_id, e);
                        self.notifications.push(Toast::error(format!("Failed to load option chain: {}", e)));
                    }
                }
            }
            Message::LoadGexProfile { pane_id, underlying_ticker, date } => {
                if let Some(service) = self.options_service.clone() {
                    return Task::perform(
                        async move {
                            service.get_gex_profile(&underlying_ticker, date).await
                                .map_err(|e| e.to_string())
                        },
                        move |result| Message::GexProfileLoaded { pane_id, result }
                    );
                } else {
                    log::warn!("Options service not available - set MASSIVE_API_KEY to enable");
                    self.notifications.push(Toast::error("GEX data unavailable - configure MASSIVE_API_KEY".to_string()));
                }
            }
            Message::GexProfileLoaded { pane_id, result } => {
                match result {
                    Ok(profile) => {
                        log::info!("GEX profile loaded for pane {}: {} exposure levels for {}",
                            pane_id, profile.exposure_count(), profile.underlying_ticker);

                        if let Some(zero_gamma) = profile.zero_gamma_level {
                            log::info!("Zero gamma level: ${:.2}", zero_gamma.to_f64());
                        }

                        self.notifications.push(Toast::new(toast::Notification::Info(format!(
                            "Loaded GEX: {} key levels",
                            profile.key_levels.len()
                        ))));
                        // TODO: Dashboard needs handler for GEX visualization
                    }
                    Err(e) => {
                        log::error!("Failed to load GEX profile for pane {}: {}", pane_id, e);
                        self.notifications.push(Toast::error(format!("Failed to load GEX: {}", e)));
                    }
                }
            }
            Message::ReplayEvent(event) => {
                log::debug!("Replay event: {:?}", event);
                // Forward replay events to panels for visualization
                use data::services::ReplayEvent;

                match event {
                    ReplayEvent::DataLoaded { ticker, trade_count, depth_count, time_range } => {
                        log::info!("Replay data loaded for {:?}: {} trades, {} depth snapshots, range: {:?}",
                            ticker, trade_count, depth_count, time_range);
                        self.notifications.push(Toast::new(toast::Notification::Info(
                            format!("Replay data loaded: {} trades", trade_count)
                        )));
                    }
                    ReplayEvent::LoadingProgress { progress, message } => {
                        log::debug!("Replay loading: {}% - {}", (progress * 100.0) as u32, message);
                    }
                    ReplayEvent::MarketData { timestamp, trades, depth } => {
                        // Forward market data updates to panels
                        // Panels (Ladder, TimeAndSales) can call update_from_replay()
                        log::debug!("Replay market data at {}: {} trades, depth: {}",
                            timestamp, trades.len(), depth.is_some()
                        );
                        // TODO: Route to specific panels that are in replay mode
                    }
                    ReplayEvent::PositionUpdate { timestamp, progress } => {
                        log::debug!("Replay position: {} ({:.1}%)", timestamp, progress * 100.0);
                    }
                    ReplayEvent::StatusChanged(status) => {
                        log::info!("Replay status changed: {:?}", status);
                    }
                    ReplayEvent::PlaybackComplete => {
                        log::info!("Replay playback completed");
                        self.notifications.push(Toast::new(toast::Notification::Info(
                            "Replay completed".to_string()
                        )));
                    }
                    ReplayEvent::PlaybackStarted => {
                        log::info!("Replay playback started");
                    }
                    ReplayEvent::PlaybackPaused => {
                        log::info!("Replay playback paused");
                    }
                    ReplayEvent::PlaybackStopped => {
                        log::info!("Replay playback stopped");
                    }
                    ReplayEvent::SeekCompleted { timestamp, progress } => {
                        log::info!("Replay seek completed to {} ({:.1}%)", timestamp, progress * 100.0);
                    }
                    ReplayEvent::SpeedChanged(speed) => {
                        log::info!("Replay speed changed to {:?}", speed);
                    }
                    ReplayEvent::CacheHit { symbol, date_range } => {
                        log::debug!("Replay cache hit for {} in range {:?}", symbol, date_range);
                    }
                    ReplayEvent::MemoryUsage { bytes, trades, depth_snapshots } => {
                        log::debug!("Replay memory usage: {:.2} MB ({} trades, {} depth snapshots)",
                            bytes as f32 / 1024.0 / 1024.0, trades, depth_snapshots);
                    }
                    ReplayEvent::Error(err) => {
                        log::error!("Replay error: {}", err);
                        self.notifications.push(Toast::error(format!("Replay error: {}", err)));
                    }
                }
            }
            Message::UpdateLoadingStatus => {
                // Poll loading statuses from MarketDataService and update panes
                let all_statuses = self.market_data_service.get_all_loading_statuses();

                for (chart_key, status) in all_statuses {
                    // Parse the chart key to get pane_id
                    // Chart keys are in format "ticker-basis-daterange"
                    // We need to match this to panes by their current config
                    for layout in &self.layout_manager.layouts {
                        if let Some((pane_id, _)) = layout.dashboard.charts.iter().find(|(_, chart_state)| {
                            let config = &chart_state.config;
                            let key = format!("{:?}-{:?}-{:?}", config.ticker, config.basis, config.date_range);
                            key == chart_key
                        }) {
                            // Update pane loading status
                            return Task::done(Message::Dashboard {
                                layout_id: Some(layout.id.unique),
                                event: dashboard::Message::ChangePaneStatus(*pane_id, status.clone()),
                            });
                        }
                    }
                }
            }
            // Data management - cost estimation
            Message::EstimateDataCost { pane_id, ticker, schema, date_range } => {
                let service = self.market_data_service.clone();
                let schema_discriminant = schema as u16;
                return Task::perform(
                    async move {
                        service.estimate_data_request(&ticker, schema_discriminant, &date_range).await
                            .map_err(|e| e.to_string())
                    },
                    move |result| Message::DataCostEstimated { pane_id, result }
                );
            }
            Message::DataCostEstimated { pane_id, result } => {
                // Update sidebar data management modal
                match result {
                    Ok((total_days, cached_days, uncached_days, gaps_desc, actual_cost_usd, cached_dates)) => {
                        log::info!("Cost estimated: {}/{} days cached, ${:.4} USD", cached_days, total_days, actual_cost_usd);

                        // Update sidebar modal (if it's nil UUID, it's from sidebar)
                        if pane_id == uuid::Uuid::nil() {
                            self.data_management_panel.set_cache_status(
                                crate::modal::pane::data_management::CacheStatus {
                                    total_days,
                                    cached_days,
                                    uncached_days,
                                    gaps_description: gaps_desc.clone(),
                                },
                                cached_dates
                            );

                            // Set REAL cost from Databento API
                            self.data_management_panel.set_actual_cost(actual_cost_usd);
                        } else {
                            // Forward to dashboard pane modal
                            let layout_id = self.layout_manager.active_layout_id()
                                .map(|id| id.unique)
                                .unwrap_or_else(|| self.layout_manager.layouts.first().unwrap().id.unique);

                            return Task::done(Message::Dashboard {
                                layout_id: Some(layout_id),
                                event: dashboard::Message::DataCostEstimated {
                                    pane_id,
                                    total_days,
                                    cached_days,
                                    uncached_days,
                                    gaps_desc,
                                    actual_cost_usd,
                                    cached_dates,
                                },
                            });
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to estimate cost: {}", e);
                        self.notifications.push(Toast::error(format!("Estimation failed: {}", e)));
                    }
                }
            }
            // Data management - download
            Message::DownloadData { pane_id, ticker, schema, date_range } => {
                let service = self.market_data_service.clone();
                let schema_discriminant = schema as u16;
                let ticker_clone = ticker.clone();
                let date_range_clone = date_range;
                return Task::perform(
                    async move {
                        service.download_to_cache(&ticker, schema_discriminant, &date_range).await
                            .map_err(|e| e.to_string())
                    },
                    move |result| Message::DataDownloadComplete {
                        pane_id,
                        ticker: ticker_clone,
                        date_range: date_range_clone,
                        result
                    }
                );
            }
            Message::DataDownloadProgress { pane_id, current, total } => {
                log::debug!("Download progress for pane {}: {}/{}", pane_id, current, total);
                // TODO: Update pane's data management modal with progress
            }
            Message::DataDownloadComplete { pane_id, ticker, date_range, result } => {
                match result {
                    Ok(days_downloaded) => {
                        log::info!("Downloaded {} days for {} ({} to {})",
                            days_downloaded, ticker, date_range.start, date_range.end);
                        self.notifications.push(Toast::new(toast::Notification::Info(
                            format!("Successfully downloaded {} days of data", days_downloaded)
                        )));

                        // Register ticker in the registry
                        self.downloaded_tickers.lock().unwrap().register(ticker.clone(), date_range);
                        log::info!("Registered {} in downloaded tickers registry", ticker);

                        // Update ticker list to show newly downloaded ticker
                        let ticker_symbols: std::collections::HashSet<String> =
                            self.downloaded_tickers.lock().unwrap().list_tickers().into_iter().collect();
                        self.sidebar.tickers_table.set_cached_filter(ticker_symbols);
                        log::info!("Updated ticker list with {} tickers", self.downloaded_tickers.lock().unwrap().count());

                        // Update sidebar modal (if nil UUID) or dashboard pane
                        if pane_id == uuid::Uuid::nil() {
                            // Reset to Idle state
                            self.data_management_panel.set_download_progress(
                                crate::modal::pane::data_management::DownloadProgress::Idle
                            );

                            // Re-trigger estimation to refresh cache colors in calendar
                            let estimate_ticker = data::FuturesTicker::new(
                                crate::modal::pane::data_management::FUTURES_PRODUCTS[self.data_management_panel.selected_ticker_idx()].0,
                                data::FuturesVenue::CMEGlobex
                            );
                            let schema = crate::modal::pane::data_management::SCHEMAS[self.data_management_panel.selected_schema_idx()].0;
                            let estimate_date_range = self.data_management_panel.current_date_range();

                            let estimate_task = Task::done(Message::EstimateDataCost {
                                pane_id: uuid::Uuid::nil(),
                                ticker: estimate_ticker,
                                schema,
                                date_range: estimate_date_range,
                            });

                            return estimate_task;
                        } else {
                            let layout_id = self.layout_manager.active_layout_id()
                                .map(|id| id.unique)
                                .unwrap_or_else(|| self.layout_manager.layouts.first().unwrap().id.unique);

                            return Task::done(Message::Dashboard {
                                layout_id: Some(layout_id),
                                event: dashboard::Message::DataDownloadComplete {
                                    pane_id,
                                    days_downloaded,
                                },
                            });
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to download data: {}", e);
                        self.notifications.push(Toast::error(format!("Download failed: {}", e)));
                    }
                }
            }
            Message::Tick(now) => {
                let main_window_id = self.main_window.id;

                return self
                    .active_dashboard_mut()
                    .tick(now, main_window_id)
                    .map(move |msg| Message::Dashboard {
                        layout_id: None,
                        event: msg,
                    });
            }
            Message::WindowEvent(event) => match event {
                window::Event::CloseRequested(window) => {
                    let main_window = self.main_window.id;
                    let dashboard = self.active_dashboard_mut();

                    if window != main_window {
                        dashboard.popout.remove(&window);
                        return window::close(window);
                    }

                    let mut active_windows = dashboard
                        .popout
                        .keys()
                        .copied()
                        .collect::<Vec<window::Id>>();
                    active_windows.push(main_window);

                    return window::collect_window_specs(active_windows, Message::ExitRequested);
                }
            },
            Message::ExitRequested(windows) => {
                self.save_state_to_disk(&windows);
                return iced::exit();
            }
            Message::RestartRequested(windows) => {
                self.save_state_to_disk(&windows);
                return self.restart();
            }
            Message::GoBack => {
                let main_window = self.main_window.id;

                if self.confirm_dialog.is_some() {
                    self.confirm_dialog = None;
                } else if self.sidebar.active_menu().is_some() {
                    self.sidebar.set_menu(None);
                } else {
                    let dashboard = self.active_dashboard_mut();

                    if dashboard.go_back(main_window) {
                        return Task::none();
                    } else if dashboard.focus.is_some() {
                        dashboard.focus = None;
                    } else {
                        self.sidebar.hide_tickers_table();
                    }
                }
            }
            Message::ThemeSelected(theme) => {
                self.theme = theme.clone();
            }
            Message::Dashboard {
                layout_id: id,
                event: msg,
            } => {
                let Some(active_layout) = self.layout_manager.active_layout_id() else {
                    log::error!("No active layout to handle dashboard message");
                    return Task::none();
                };

                let main_window = self.main_window;
                let layout_id = id.unwrap_or(active_layout.unique);

                if let Some(dashboard) = self.layout_manager.mut_dashboard(layout_id) {
                    let (main_task, event) = dashboard.update(msg, &main_window, &layout_id);

                    let additional_task = match event {
                        Some(dashboard::Event::LoadChart { pane_id, config, ticker_info }) => {
                            // Trigger async chart data loading
                            Task::done(Message::LoadChartData { layout_id, pane_id, config, ticker_info })
                        }
                        Some(dashboard::Event::Notification(toast)) => {
                            self.notifications.push(toast);
                            Task::none()
                        }
                        Some(dashboard::Event::EstimateDataCost { pane_id, ticker, schema, date_range }) => {
                            // Trigger async cost estimation
                            Task::done(Message::EstimateDataCost { pane_id, ticker, schema, date_range })
                        }
                        Some(dashboard::Event::DownloadData { pane_id, ticker, schema, date_range }) => {
                            // Trigger async data download
                            Task::done(Message::DownloadData { pane_id, ticker, schema, date_range })
                        }
                        None => Task::none(),
                    };

                    return main_task
                        .map(move |msg| Message::Dashboard {
                            layout_id: Some(layout_id),
                            event: msg,
                        })
                        .chain(additional_task);
                }
            }
            Message::RemoveNotification(index) => {
                if index < self.notifications.len() {
                    self.notifications.remove(index);
                }
            }
            Message::SetTimezone(tz) => {
                self.timezone = tz;
            }
            Message::ScaleFactorChanged(value) => {
                self.ui_scale_factor = value;
            }
            Message::ToggleDialogModal(dialog) => {
                self.confirm_dialog = dialog;
            }
            Message::Layouts(message) => {
                let action = self.layout_manager.update(message);

                match action {
                    Some(modal::layout_manager::Action::Select(layout)) => {
                        let active_popout_keys = self
                            .active_dashboard()
                            .popout
                            .keys()
                            .copied()
                            .collect::<Vec<_>>();

                        let window_tasks = Task::batch(
                            active_popout_keys
                                .iter()
                                .map(|&popout_id| window::close::<window::Id>(popout_id))
                                .collect::<Vec<_>>(),
                        )
                        .discard();

                        let old_layout_id = self
                            .layout_manager
                            .active_layout_id()
                            .as_ref()
                            .map(|layout| layout.unique);

                        return window::collect_window_specs(
                            active_popout_keys,
                            dashboard::Message::SavePopoutSpecs,
                        )
                        .map(move |msg| Message::Dashboard {
                            layout_id: old_layout_id,
                            event: msg,
                        })
                        .chain(window_tasks)
                        .chain(self.load_layout(layout, self.main_window.id));
                    }
                    Some(modal::layout_manager::Action::Clone(id)) => {
                        let manager = &mut self.layout_manager;

                        let source_data = manager.get(id).map(|layout| {
                            (
                                layout.id.name.clone(),
                                layout.id.unique,
                                data::Dashboard::from(&layout.dashboard),
                            )
                        });

                        if let Some((name, old_id, ser_dashboard)) = source_data {
                            let new_uid = uuid::Uuid::new_v4();
                            let new_layout = LayoutId {
                                unique: new_uid,
                                name: manager.ensure_unique_name(&name, new_uid),
                            };

                            let mut popout_windows = Vec::new();

                            for (pane, window_spec) in &ser_dashboard.popout {
                                let configuration = configuration(pane.clone());
                                popout_windows.push((configuration, window_spec.clone()));
                            }

                            let dashboard = Dashboard::from_config(
                                configuration(ser_dashboard.pane.clone()),
                                popout_windows,
                                old_id,
                                self.market_data_service.clone(),
                                self.downloaded_tickers.clone(),
                            );

                            manager.insert_layout(new_layout.clone(), dashboard);
                        }
                    }
                    None => {}
                }
            }
            Message::DataManagement(msg) => {
                if let Some(action) = self.data_management_panel.update(msg) {
                    match action {
                        crate::modal::pane::data_management::Action::EstimateRequested { ticker, schema, date_range } => {
                            log::info!("Estimate requested from sidebar: {:?} {:?} {:?}", ticker, schema, date_range);
                            return Task::done(Message::EstimateDataCost {
                                pane_id: uuid::Uuid::nil(),
                                ticker,
                                schema,
                                date_range,
                            });
                        }
                        crate::modal::pane::data_management::Action::DownloadRequested { ticker, schema, date_range } => {
                            log::info!("Download requested from sidebar: {:?} {:?} {:?}", ticker, schema, date_range);
                            return Task::done(Message::DownloadData {
                                pane_id: uuid::Uuid::nil(),
                                ticker,
                                schema,
                                date_range,
                            });
                        }
                    }
                }
            }
            Message::AudioStream(message) => self.audio_stream.update(message),
            Message::DataFolderRequested => {
                if let Err(err) = data::open_data_folder() {
                    self.notifications
                        .push(Toast::error(format!("Failed to open data folder: {err}")));
                }
            }
            Message::ThemeEditor(msg) => {
                let action = self.theme_editor.update(msg, &self.theme.clone().into());

                match action {
                    Some(modal::theme_editor::Action::Exit) => {
                        self.sidebar.set_menu(Some(sidebar::Menu::Settings));
                    }
                    Some(modal::theme_editor::Action::UpdateTheme(theme)) => {
                        self.theme = data::Theme(theme);

                        let main_window = self.main_window.id;

                        self.active_dashboard_mut()
                            .invalidate_all_panes(main_window);
                    }
                    None => {}
                }
            }
            Message::Sidebar(message) => {
                let (task, action) = self.sidebar.update(message);

                match action {
                    Some(dashboard::sidebar::Action::TickerSelected(ticker_info, content)) => {
                        let main_window_id = self.main_window.id;

                        // Convert exchange::TickerInfo to domain::FuturesTickerInfo
                        let futures_info = ticker_info.to_domain();

                        // Default to CandlestickChart if no content kind specified
                        let kind = content.unwrap_or(data::ContentKind::CandlestickChart);

                        log::info!("MAIN: TickerSelected {:?} with ContentKind::{:?}", ticker_info.ticker, kind);

                        let task = self.active_dashboard_mut().init_focused_pane(
                            main_window_id,
                            futures_info,
                            kind,
                        );

                        return task.map(move |msg| Message::Dashboard {
                            layout_id: None,
                            event: msg,
                        });
                    }
                    Some(dashboard::sidebar::Action::ErrorOccurred(err)) => {
                        self.notifications.push(Toast::error(err.to_string()));
                    }
                    None => {}
                }

                return task.map(Message::Sidebar);
            }
        }
        Task::none()
    }

    fn view(&self, id: window::Id) -> Element<'_, Message> {
        let dashboard = self.active_dashboard();
        let sidebar_pos = self.sidebar.position();

        let tickers_table = &self.sidebar.tickers_table;

        let content = if id == self.main_window.id {
            let sidebar_view = self
                .sidebar
                .view(self.audio_stream.volume())
                .map(Message::Sidebar);

            let dashboard_view = dashboard
                .view(&self.main_window, tickers_table, self.timezone)
                .map(move |msg| Message::Dashboard {
                    layout_id: None,
                    event: msg,
                });

            let header_title = {
                #[cfg(target_os = "macos")]
                {
                    iced::widget::center(
                        text("FLOWSURFACE")
                            .font(iced::Font {
                                weight: iced::font::Weight::Bold,
                                ..Default::default()
                            })
                            .size(16)
                            .style(style::title_text),
                    )
                    .height(20)
                    .align_y(Alignment::Center)
                    .padding(padding::top(4))
                }
                #[cfg(not(target_os = "macos"))]
                {
                    column![]
                }
            };

            let base = column![
                header_title,
                match sidebar_pos {
                    sidebar::Position::Left => row![sidebar_view, dashboard_view,],
                    sidebar::Position::Right => row![dashboard_view, sidebar_view],
                }
                .spacing(4)
                .padding(8),
            ];

            if let Some(menu) = self.sidebar.active_menu() {
                self.view_with_modal(base.into(), dashboard, menu)
            } else {
                base.into()
            }
        } else {
            container(
                dashboard
                    .view_window(id, &self.main_window, tickers_table, self.timezone)
                    .map(move |msg| Message::Dashboard {
                        layout_id: None,
                        event: msg,
                    }),
            )
            .padding(padding::top(style::TITLE_PADDING_TOP))
            .into()
        };

        toast::Manager::new(
            content,
            &self.notifications,
            match sidebar_pos {
                sidebar::Position::Left => Alignment::Start,
                sidebar::Position::Right => Alignment::End,
            },
            Message::RemoveNotification,
        )
        .into()
    }

    fn theme(&self, _window: window::Id) -> iced_core::Theme {
        self.theme.clone().into()
    }

    fn title(&self, _window: window::Id) -> String {
        if let Some(id) = self.layout_manager.active_layout_id() {
            format!("Flowsurface [{}]", id.name)
        } else {
            "Flowsurface".to_string()
        }
    }

    fn scale_factor(&self, _window: window::Id) -> f32 {
        self.ui_scale_factor.into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let window_events = window::events().map(Message::WindowEvent);
        let sidebar = self.sidebar.subscription().map(Message::Sidebar);

        let tick = iced::time::every(std::time::Duration::from_millis(100)).map(Message::Tick);

        // Poll for loading status updates every 500ms
        let status_poll = iced::time::every(std::time::Duration::from_millis(500))
            .map(|_| Message::UpdateLoadingStatus);

        let hotkeys = keyboard::listen().filter_map(|event| {
            let keyboard::Event::KeyPressed { key, .. } = event else {
                return None;
            };
            match key {
                keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::GoBack),
                _ => None,
            }
        });

        Subscription::batch(vec![
            sidebar,
            window_events,
            tick,
            status_poll,
            hotkeys,
        ])
    }

    /// Initialize options services from environment
    fn initialize_options_services() -> (
        Option<std::sync::Arc<data::services::OptionsDataService>>,
        std::sync::Arc<data::services::GexCalculationService>,
    ) {
        // GEX service is always available (no I/O, pure computation)
        let gex_service = std::sync::Arc::new(data::services::GexCalculationService::new());

        // Try to initialize Massive options service from environment
        let options_service = match std::env::var("MASSIVE_API_KEY") {
            Ok(api_key) if !api_key.is_empty() => {
                log::info!("MASSIVE_API_KEY found, initializing options data service");

                let config = exchange::MassiveConfig::new(api_key);

                // Initialize repositories asynchronously
                match tokio::runtime::Runtime::new() {
                    Ok(rt) => {
                        rt.block_on(async {
                            let snapshot_repo_result = exchange::MassiveSnapshotRepository::new(config.clone()).await;
                            let chain_repo_result = exchange::MassiveChainRepository::new(config.clone()).await;
                            let contract_repo_result = exchange::MassiveContractRepository::new(config).await;

                            match (snapshot_repo_result, chain_repo_result, contract_repo_result) {
                                (Ok(snapshot_repo), Ok(chain_repo), Ok(contract_repo)) => {
                                    let service = data::services::OptionsDataService::new(
                                        std::sync::Arc::new(snapshot_repo),
                                        std::sync::Arc::new(chain_repo),
                                        std::sync::Arc::new(contract_repo),
                                    );

                                    log::info!("✓ Options data service initialized successfully");
                                    Some(std::sync::Arc::new(service))
                                }
                                (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                                    log::error!("Failed to initialize options repositories: {}", e);
                                    None
                                }
                            }
                        })
                    }
                    Err(e) => {
                        log::error!("Failed to create runtime for options service: {}", e);
                        None
                    }
                }
            }
            _ => {
                log::info!("MASSIVE_API_KEY not set - options data features disabled");
                log::info!("To enable: export MASSIVE_API_KEY=your_polygon_api_key");
                None
            }
        };

        (options_service, gex_service)
    }

    fn active_dashboard(&self) -> &Dashboard {
        let active_layout = self
            .layout_manager
            .active_layout_id()
            .expect("No active layout");
        self.layout_manager
            .get(active_layout.unique)
            .map(|layout| &layout.dashboard)
            .expect("No active dashboard")
    }

    fn active_dashboard_mut(&mut self) -> &mut Dashboard {
        let active_layout = self
            .layout_manager
            .active_layout_id()
            .expect("No active layout");
        self.layout_manager
            .get_mut(active_layout.unique)
            .map(|layout| &mut layout.dashboard)
            .expect("No active dashboard")
    }

    /// Get options service (if available)
    pub fn options_service(&self) -> Option<&std::sync::Arc<data::services::OptionsDataService>> {
        self.options_service.as_ref()
    }

    /// Get GEX calculation service
    pub fn gex_service(&self) -> &std::sync::Arc<data::services::GexCalculationService> {
        &self.gex_service
    }

    fn load_layout(&mut self, layout_uid: uuid::Uuid, main_window: window::Id) -> Task<Message> {
        match self.layout_manager.set_active_layout(layout_uid) {
            Ok(layout) => {
                layout
                    .dashboard
                    .load_layout(main_window)
                    .map(move |msg| Message::Dashboard {
                        layout_id: Some(layout_uid),
                        event: msg,
                    })
            }
            Err(err) => {
                log::error!("Failed to set active layout: {}", err);
                Task::none()
            }
        }
    }

    fn view_with_modal<'a>(
        &'a self,
        base: Element<'a, Message>,
        dashboard: &'a Dashboard,
        menu: sidebar::Menu,
    ) -> Element<'a, Message> {
        let sidebar_pos = self.sidebar.position();

        match menu {
            sidebar::Menu::Settings => {
                let settings_modal = {
                    let theme_picklist = {
                        let mut themes: Vec<iced::Theme> = iced_core::Theme::ALL.to_vec();

                        let default_theme = iced_core::Theme::Custom(default_theme().into());
                        themes.push(default_theme);

                        if let Some(custom_theme) = &self.theme_editor.custom_theme {
                            themes.push(custom_theme.clone());
                        }

                        pick_list(themes, Some(self.theme.0.clone()), |theme| {
                            Message::ThemeSelected(data::Theme(theme))
                        })
                    };

                    let toggle_theme_editor = button(text("Theme editor")).on_press(
                        Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(Some(
                            sidebar::Menu::ThemeEditor,
                        ))),
                    );

                    let timezone_picklist = pick_list(
                        [data::UserTimezone::Utc, data::UserTimezone::Local],
                        Some(self.timezone),
                        Message::SetTimezone,
                    );


                    let sidebar_pos = pick_list(
                        [sidebar::Position::Left, sidebar::Position::Right],
                        Some(sidebar_pos),
                        |pos| {
                            Message::Sidebar(dashboard::sidebar::Message::SetSidebarPosition(pos))
                        },
                    );

                    let scale_factor = {
                        let current_value: f32 = self.ui_scale_factor.into();

                        let decrease_btn = if current_value > data::config::MIN_SCALE {
                            button(text("-"))
                                .on_press(Message::ScaleFactorChanged((current_value - 0.1).into()))
                        } else {
                            button(text("-"))
                        };

                        let increase_btn = if current_value < data::config::MAX_SCALE {
                            button(text("+"))
                                .on_press(Message::ScaleFactorChanged((current_value + 0.1).into()))
                        } else {
                            button(text("+"))
                        };

                        container(
                            row![
                                decrease_btn,
                                text(format!("{:.0}%", current_value * 100.0)).size(14),
                                increase_btn,
                            ]
                            .align_y(Alignment::Center)
                            .spacing(8)
                            .padding(4),
                        )
                        .style(style::modal_container)
                    };


                    let open_data_folder = {
                        let button =
                            button(text("Open data folder")).on_press(Message::DataFolderRequested);

                        tooltip(
                            button,
                            Some("Open the folder where the data & config is stored"),
                            TooltipPosition::Top,
                        )
                    };

                    let column_content = split_column![
                        column![open_data_folder,].spacing(8),
                        column![text("Sidebar position").size(14), sidebar_pos,].spacing(12),
                        column![text("Time zone").size(14), timezone_picklist,].spacing(12),
                        column![text("Theme").size(14), theme_picklist,].spacing(12),
                        column![text("Interface scale").size(14), scale_factor,].spacing(12),
                        column![
                            text("Experimental").size(14),
                            toggle_theme_editor,
                        ]
                        .spacing(12),
                        ; spacing = 16, align_x = Alignment::Start
                    ];

                    let content = scrollable::Scrollable::with_direction(
                        column_content,
                        scrollable::Direction::Vertical(
                            scrollable::Scrollbar::new().width(8).scroller_width(6),
                        ),
                    );

                    container(content)
                        .align_x(Alignment::Start)
                        .max_width(240)
                        .padding(24)
                        .style(style::dashboard_modal)
                };

                let (align_x, padding) = match sidebar_pos {
                    sidebar::Position::Left => (Alignment::Start, padding::left(44).bottom(4)),
                    sidebar::Position::Right => (Alignment::End, padding::right(44).bottom(4)),
                };

                let base_content = dashboard_modal(
                    base,
                    settings_modal,
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                    padding,
                    Alignment::End,
                    align_x,
                );

                if let Some(dialog) = &self.confirm_dialog {
                    let dialog_content =
                        confirm_dialog_container(dialog.clone(), Message::ToggleDialogModal(None));

                    main_dialog_modal(
                        base_content,
                        dialog_content,
                        Message::ToggleDialogModal(None),
                    )
                } else {
                    base_content
                }
            }
            sidebar::Menu::Layout => {
                let main_window = self.main_window.id;

                let manage_pane = if let Some((window_id, pane_id)) = dashboard.focus {
                    let selected_pane_str =
                        if let Some(state) = dashboard.get_pane(main_window, window_id, pane_id) {
                            let link_group_name: String =
                                state.link_group.as_ref().map_or_else(String::new, |g| {
                                    " - Group ".to_string() + &g.to_string()
                                });

                            state.content.to_string() + &link_group_name
                        } else {
                            "".to_string()
                        };

                    let is_main_window = window_id == main_window;

                    let reset_pane_button = {
                        let btn = button(text("Reset").align_x(Alignment::Center))
                            .width(iced::Length::Fill);
                        if is_main_window {
                            let dashboard_msg = Message::Dashboard {
                                layout_id: None,
                                event: dashboard::Message::Pane(
                                    main_window,
                                    dashboard::pane::Message::ReplacePane(pane_id),
                                ),
                            };

                            btn.on_press(dashboard_msg)
                        } else {
                            btn
                        }
                    };
                    let split_pane_button = {
                        let btn = button(text("Split").align_x(Alignment::Center))
                            .width(iced::Length::Fill);
                        if is_main_window {
                            let dashboard_msg = Message::Dashboard {
                                layout_id: None,
                                event: dashboard::Message::Pane(
                                    main_window,
                                    dashboard::pane::Message::SplitPane(
                                        pane_grid::Axis::Horizontal,
                                        pane_id,
                                    ),
                                ),
                            };
                            btn.on_press(dashboard_msg)
                        } else {
                            btn
                        }
                    };

                    column![
                        text(selected_pane_str),
                        row![
                            tooltip(
                                reset_pane_button,
                                if is_main_window {
                                    Some("Reset selected pane")
                                } else {
                                    None
                                },
                                TooltipPosition::Top,
                            ),
                            tooltip(
                                split_pane_button,
                                if is_main_window {
                                    Some("Split selected pane horizontally")
                                } else {
                                    None
                                },
                                TooltipPosition::Top,
                            ),
                        ]
                        .spacing(8)
                    ]
                    .spacing(8)
                } else {
                    column![text("No pane selected"),].spacing(8)
                };

                let manage_layout_modal = {
                    let col = column![
                        manage_pane,
                        rule::horizontal(1.0).style(style::split_ruler),
                        self.layout_manager.view().map(Message::Layouts)
                    ];

                    container(col.align_x(Alignment::Center).spacing(20))
                        .width(260)
                        .padding(24)
                        .style(style::dashboard_modal)
                };

                let (align_x, padding) = match sidebar_pos {
                    sidebar::Position::Left => (Alignment::Start, padding::left(44).top(40)),
                    sidebar::Position::Right => (Alignment::End, padding::right(44).top(40)),
                };

                dashboard_modal(
                    base,
                    manage_layout_modal,
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                    padding,
                    Alignment::Start,
                    align_x,
                )
            }
            sidebar::Menu::DataManagement => {
                let (align_x, padding) = match sidebar_pos {
                    sidebar::Position::Left => (Alignment::Start, padding::left(44).bottom(40)),
                    sidebar::Position::Right => (Alignment::End, padding::right(44).bottom(40)),
                };

                dashboard_modal(
                    base,
                    self.data_management_panel.view()
                        .map(Message::DataManagement), // Handle modal messages properly
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                    padding,
                    Alignment::End, // Align to left/right edge (same as settings)
                    align_x,
                )
            }
            sidebar::Menu::Audio => {
                let (align_x, padding) = match sidebar_pos {
                    sidebar::Position::Left => (Alignment::Start, padding::left(44).top(76)),
                    sidebar::Position::Right => (Alignment::End, padding::right(44).top(76)),
                };

                // TODO: Fetch depth streams from panes
                let depth_streams_list = vec![];

                dashboard_modal(
                    base,
                    self.audio_stream
                        .view(depth_streams_list)
                        .map(Message::AudioStream),
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                    padding,
                    Alignment::Start,
                    align_x,
                )
            }
            sidebar::Menu::ThemeEditor => {
                let (align_x, padding) = match sidebar_pos {
                    sidebar::Position::Left => (Alignment::Start, padding::left(44).bottom(4)),
                    sidebar::Position::Right => (Alignment::End, padding::right(44).bottom(4)),
                };

                dashboard_modal(
                    base,
                    self.theme_editor
                        .view(&self.theme.0)
                        .map(Message::ThemeEditor),
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                    padding,
                    Alignment::End,
                    align_x,
                )
            }
        }
    }

    fn save_state_to_disk(&mut self, windows: &HashMap<window::Id, WindowSpec>) {
        self.active_dashboard_mut()
            .popout
            .iter_mut()
            .for_each(|(id, (_, window_spec))| {
                if let Some(new_window_spec) = windows.get(id) {
                    *window_spec = new_window_spec.clone();
                }
            });

        self.sidebar.sync_tickers_table_settings();

        let main_window_spec = windows
            .iter()
            .find(|(id, _)| **id == self.main_window.id)
            .map(|(_, spec)| spec.clone());

        // Convert audio stream config to app_state format (simplified)
        let audio_cfg_full = data::AudioStream::from(&self.audio_stream);
        let audio_cfg_simplified = data::state::app_state::AudioStream {
            volume: audio_cfg_full.volume,
            enabled: !audio_cfg_full.streams.is_empty(),
        };

        // Clone the layout manager data for serialization
        let active_layout_name = self.layout_manager.active_layout_id()
            .map(|id| id.name.clone());

        let layouts_for_save: Vec<data::state::app_state::Layout> = self.layout_manager.layouts
            .iter()
            .filter_map(|layout| {
                self.layout_manager.get(layout.id.unique).map(|_l| {
                    data::state::app_state::Layout {
                        name: Some(layout.id.name.clone()),
                        panes: vec![], // Simplified - actual pane structure stored in dashboard
                    }
                })
            })
            .collect();

        let layout_manager_clone = data::state::app_state::LayoutManager {
            layouts: layouts_for_save,
            active_layout: active_layout_name,
        };

        let state = data::AppState::from_parts(
            layout_manager_clone,
            self.theme.clone(),
            self.theme_editor.custom_theme.clone().map(data::Theme),
            main_window_spec,
            self.timezone,
            self.sidebar.state.clone(),
            self.ui_scale_factor,
            audio_cfg_simplified,
            self.downloaded_tickers.lock().unwrap().clone(),
        );

        // Save state using the persistence module
        if let Err(e) = data::save_state(&state, "app-state.json") {
            log::error!("Failed to save application state: {}", e);
        } else {
            log::info!("Application state persisted successfully");
        }
    }

    fn restart(&mut self) -> Task<Message> {
        let mut windows_to_close: Vec<window::Id> =
            self.active_dashboard().popout.keys().copied().collect();
        windows_to_close.push(self.main_window.id);

        let close_windows = Task::batch(
            windows_to_close
                .into_iter()
                .map(window::close)
                .collect::<Vec<_>>(),
        );

        let (new_state, init_task) = Flowsurface::new();
        *self = new_state;

        close_windows.chain(init_task)
    }
}
