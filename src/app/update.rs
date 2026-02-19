use iced::Task;
use data::LoadingStatus;
use crate::screen::dashboard;
use crate::screen::dashboard::tickers_table;
use crate::widget::toast::{Toast, Notification};
use crate::window;

use super::{Flowsurface, Message, get_download_progress, services};

impl Flowsurface {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // Async chart data loading
            Message::LoadChartData { layout_id, pane_id, config, ticker_info } => {
                log::info!("LoadChartData message received for pane {}: {:?} chart", pane_id, config.chart_type);

                // Validate that a Databento feed is connected and track which feed
                let databento_feed_id = {
                    let feed_manager = self.data_feed_manager.lock()
                        .unwrap_or_else(|e| e.into_inner());
                    match feed_manager.connected_feed_id_for_provider(data::FeedProvider::Databento) {
                        Some(fid) => fid,
                        None => {
                            log::warn!("No Databento feed connected - cannot load chart data");
                            self.notifications.push(Toast::error(
                                "No data feed connected. Connect a feed in connection settings."
                                    .to_string(),
                            ));
                            return Task::done(Message::Dashboard {
                                layout_id: Some(layout_id),
                                event: dashboard::Message::ChangePaneStatus(
                                    pane_id,
                                    LoadingStatus::Error {
                                        message: "No data feed connected".to_string(),
                                    },
                                ),
                            });
                        }
                    }
                };

                // Set feed_id on the pane so we know which feed owns its data
                if let Some(dashboard) = self.layout_manager.mut_dashboard(layout_id) {
                    let main_window = self.main_window.id;
                    if let Some(pane_state) = dashboard.get_mut_pane_state_by_uuid(main_window, pane_id) {
                        pane_state.feed_id = Some(databento_feed_id);
                    }
                }

                let Some(service) = self.market_data_service.clone() else {
                    log::warn!("Market data service not available (API key not configured)");
                    self.notifications.push(Toast::error(
                        "Databento API key not configured. Set it in connection settings."
                            .to_string(),
                    ));
                    return Task::none();
                };

                return Task::perform(
                    async move {
                        log::info!("Starting async get_chart_data for {:?}...", config.chart_type);
                        let result = service.get_chart_data(&config, &ticker_info).await;
                        log::info!("get_chart_data completed: {}", if result.is_ok() { "SUCCESS" } else { "ERROR" });
                        result.map_err(|e| e.to_string())
                    },
                    move |result| Message::ChartDataLoaded { layout_id, pane_id, result }
                );
            }
            Message::ChartDataLoaded { layout_id, pane_id, result } => {
                match result {
                    Ok(chart_data) => {
                        log::info!("Chart data loaded for pane {}: {} trades, {} candles",
                            pane_id, chart_data.trades.len(), chart_data.candles.len());

                        return Task::done(Message::Dashboard {
                            layout_id: Some(layout_id),
                            event: dashboard::Message::ChartDataLoaded { pane_id, chart_data },
                        });
                    }
                    Err(e) => {
                        log::error!("Failed to load chart data for pane {}: {}", pane_id, e);
                        self.notifications.push(Toast::error(format!("Failed to load chart data: {}", e)));

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
                let secrets = data::SecretsManager::new();
                if !secrets.has_api_key(data::ApiProvider::Massive) {
                    log::warn!("Massive API key not configured");
                    self.notifications.push(Toast::error(
                        "Massive API key not configured.".to_string()
                    ));
                    return Task::none();
                }

                if let Some(service) = self.options_service.clone() {
                    return Task::perform(
                        async move {
                            service.get_chain_with_greeks(&underlying_ticker, date).await
                                .map_err(|e| e.to_string())
                        },
                        move |result| Message::OptionChainLoaded { pane_id, result }
                    );
                } else {
                    log::warn!("Options service not available - reinitializing may be required");
                    self.notifications.push(Toast::error("Options service not initialized - try reconfiguring API key".to_string()));
                }
            }
            Message::OptionChainLoaded { pane_id, result } => {
                match result {
                    Ok(chain) => {
                        log::info!("Option chain loaded for pane {}: {} contracts for {}",
                            pane_id, chain.contract_count(), chain.underlying_ticker);
                        self.notifications.push(Toast::new(Notification::Info(format!(
                            "Loaded {} option contracts",
                            chain.contract_count()
                        ))));
                    }
                    Err(e) => {
                        log::error!("Failed to load option chain for pane {}: {}", pane_id, e);
                        self.notifications.push(Toast::error(format!("Failed to load option chain: {}", e)));
                    }
                }
            }
            Message::LoadGexProfile { pane_id, underlying_ticker, date } => {
                let secrets = data::SecretsManager::new();
                if !secrets.has_api_key(data::ApiProvider::Massive) {
                    log::warn!("Massive API key not configured");
                    self.notifications.push(Toast::error(
                        "Massive API key not configured.".to_string()
                    ));
                    return Task::none();
                }

                if let Some(service) = self.options_service.clone() {
                    return Task::perform(
                        async move {
                            service.get_gex_profile(&underlying_ticker, date).await
                                .map_err(|e| e.to_string())
                        },
                        move |result| Message::GexProfileLoaded { pane_id, result }
                    );
                } else {
                    log::warn!("Options service not available - reinitializing may be required");
                    self.notifications.push(Toast::error("Options service not initialized - try reconfiguring API key".to_string()));
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

                        self.notifications.push(Toast::new(Notification::Info(format!(
                            "Loaded GEX: {} key levels",
                            profile.key_levels.len()
                        ))));
                    }
                    Err(e) => {
                        log::error!("Failed to load GEX profile for pane {}: {}", pane_id, e);
                        self.notifications.push(Toast::error(format!("Failed to load GEX: {}", e)));
                    }
                }
            }
            Message::UpdateLoadingStatus => {
                // Poll loading statuses from MarketDataService and update panes
                let Some(service) = &self.market_data_service else {
                    // No service available, nothing to update
                    return Task::none();
                };

                let all_statuses = service.get_all_loading_statuses();

                let mut tasks = Vec::new();
                for (chart_key, status) in all_statuses {
                    for layout in &self.layout_manager.layouts {
                        if let Some((pane_id, _)) = layout.dashboard.charts.iter().find(|(_, chart_state)| {
                            let config = &chart_state.config;
                            let key = format!("{:?}-{:?}-{:?}", config.ticker, config.basis, config.date_range);
                            key == chart_key
                        }) {
                            tasks.push(Task::done(Message::Dashboard {
                                layout_id: Some(layout.id.unique),
                                event: dashboard::Message::ChangePaneStatus(*pane_id, status.clone()),
                            }));
                            break;
                        }
                    }
                }
                if !tasks.is_empty() {
                    return Task::batch(tasks);
                }
            }
            // Data management - cost estimation
            Message::EstimateDataCost { pane_id, ticker, schema, date_range } => {
                log::info!("EstimateDataCost message received");
                log::info!("Ticker={:?}, Schema={:?}, Range={:?}", ticker, schema, date_range);

                let Some(service) = self.market_data_service.clone() else {
                    log::warn!("Market data service not available (API key not configured)");
                    self.notifications.push(Toast::error(
                        "Databento API key not configured. Set it in connection settings."
                            .to_string(),
                    ));
                    return Task::none();
                };

                let schema_discriminant = schema as u16;
                return Task::perform(
                    async move {
                        log::info!("Async block entered, about to call service");
                        let result = service.estimate_data_request(&ticker, schema_discriminant, &date_range).await;
                        log::info!("Service call completed, result success: {}", result.is_ok());
                        if let Err(ref e) = result {
                            log::error!("Service error: {}", e);
                        }
                        result.map_err(|e| e.to_string())
                    },
                    move |result| {
                        log::info!("Task finished, sending DataCostEstimated");
                        Message::DataCostEstimated { pane_id, result }
                    }
                );
            }
            Message::DataCostEstimated { pane_id, result } => {
                match result {
                    Ok((total_days, cached_days, uncached_days, gaps_desc, actual_cost_usd, cached_dates)) => {
                        log::info!("Cost estimated: {}/{} days cached, ${:.4} USD", cached_days, total_days, actual_cost_usd);

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
                            self.data_management_panel.set_actual_cost(actual_cost_usd);
                        } else {
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
                // Check if market data service is available
                let Some(service) = self.market_data_service.clone() else {
                    log::warn!("Market data service not available (API key not configured)");
                    self.notifications.push(Toast::error(
                        "Databento API key not configured. Set it in connection settings."
                            .to_string(),
                    ));
                    return Task::none();
                };

                let schema_discriminant = schema as u16;
                let ticker_clone = ticker;
                let date_range_clone = date_range;

                {
                    let mut progress =
                        data::lock_or_recover(get_download_progress());
                    progress.insert(pane_id, (0, date_range.num_days() as usize));
                }

                return Task::perform(
                    async move {
                        service.download_to_cache_with_progress(
                            &ticker,
                            schema_discriminant,
                            &date_range,
                            Box::new(move |current, total| {
                                if let Ok(mut progress) = get_download_progress().lock() {
                                    progress.insert(pane_id, (current, total));
                                }
                                log::info!("Download progress: {}/{} days", current, total);
                            })
                        ).await
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
                log::info!("Download progress for pane {}: {}/{}", pane_id, current, total);

                if self.historical_download_id == Some(pane_id) {
                    if let Some(modal) = &mut self.historical_download_modal {
                        modal.set_download_progress(
                            crate::modal::pane::historical_download::DownloadProgress::Downloading {
                                current_day: current,
                                total_days: total,
                            }
                        );
                    }
                } else if pane_id == uuid::Uuid::nil() {
                    self.data_management_panel.set_download_progress(
                        crate::modal::pane::data_management::DownloadProgress::Downloading {
                            current_day: current,
                            total_days: total,
                        }
                    );
                } else {
                    return Task::done(Message::Dashboard {
                        layout_id: self.layout_manager.active_layout_id().map(|l| l.unique),
                        event: dashboard::Message::DataDownloadProgress { pane_id, current, total },
                    });
                }
            }
            Message::DataDownloadComplete { pane_id, ticker, date_range, result } => {
                {
                    let mut progress =
                        data::lock_or_recover(get_download_progress());
                    progress.remove(&pane_id);
                }

                match result {
                    Ok(days_downloaded) => {
                        log::info!("Downloaded {} days for {} ({} to {})",
                            days_downloaded, ticker, date_range.start, date_range.end);
                        self.notifications.push(Toast::new(Notification::Info(
                            format!("Successfully downloaded {} days of data", days_downloaded)
                        )));

                        data::lock_or_recover(&self.downloaded_tickers)
                            .register(ticker, date_range);
                        log::info!("Registered {} in downloaded tickers registry", ticker);

                        let ticker_symbols: std::collections::HashSet<String> =
                            data::lock_or_recover(&self.downloaded_tickers)
                                .list_tickers()
                                .into_iter()
                                .collect();
                        self.tickers_table.set_cached_filter(ticker_symbols);
                        log::info!(
                            "Updated ticker list with {} tickers",
                            data::lock_or_recover(&self.downloaded_tickers).count()
                        );

                        if pane_id == uuid::Uuid::nil() {
                            self.data_management_panel.set_download_progress(
                                crate::modal::pane::data_management::DownloadProgress::Idle
                            );

                            let estimate_ticker = data::FuturesTicker::new(
                                crate::modal::pane::FUTURES_PRODUCTS[self.data_management_panel.selected_ticker_idx()].0,
                                data::FuturesVenue::CMEGlobex
                            );
                            let schema = crate::modal::pane::SCHEMAS[self.data_management_panel.selected_schema_idx()].0;
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
                            Task::done(Message::LoadChartData { layout_id, pane_id, config, ticker_info })
                        }
                        Some(dashboard::Event::Notification(toast)) => {
                            self.notifications.push(toast);
                            Task::none()
                        }
                        Some(dashboard::Event::EstimateDataCost { pane_id, ticker, schema, date_range }) => {
                            Task::done(Message::EstimateDataCost { pane_id, ticker, schema, date_range })
                        }
                        Some(dashboard::Event::DownloadData { pane_id, ticker, schema, date_range }) => {
                            Task::done(Message::DownloadData { pane_id, ticker, schema, date_range })
                        }
                        Some(dashboard::Event::PaneClosed { pane_id }) => {
                            // Clean up any in-progress download tracking for the closed pane
                            if let Ok(mut progress) = super::get_download_progress().lock() {
                                progress.remove(&pane_id);
                            }
                            log::debug!("Cleaned up resources for closed pane {}", pane_id);
                            Task::none()
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
                    Some(crate::modal::layout_manager::Action::Select(layout)) => {
                        return self.handle_layout_select(layout);
                    }
                    Some(crate::modal::layout_manager::Action::Clone(id)) => {
                        self.handle_layout_clone(id);
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
            Message::ConnectionsMenu(msg) => {
                use crate::modal::pane::connections_menu::{Action, ConnectionsMenuMessage};
                if let Some(action) = self.connections_menu.update(msg) {
                    match action {
                        Action::ConnectFeed(feed_id) => {
                            self.sidebar.set_menu(None);
                            return Task::done(Message::DataFeeds(
                                crate::modal::pane::data_feeds::DataFeedsMessage::ConnectFeed(
                                    feed_id,
                                ),
                            ));
                        }
                        Action::DisconnectFeed(feed_id) => {
                            self.sidebar.set_menu(None);
                            return Task::done(Message::DataFeeds(
                                crate::modal::pane::data_feeds::DataFeedsMessage::DisconnectFeed(
                                    feed_id,
                                ),
                            ));
                        }
                        Action::OpenManageDialog => {
                            self.sidebar.set_menu(Some(data::sidebar::Menu::DataFeeds));
                            let feed_manager = self
                                .data_feed_manager
                                .lock()
                                .unwrap_or_else(|e| e.into_inner());
                            self.data_feeds_modal.sync_snapshot(&feed_manager);
                        }
                    }
                }
            }
            Message::HistoricalDownload(msg) => {
                if let Some(modal) = &mut self.historical_download_modal {
                    if let Some(action) = modal.update(msg) {
                        match action {
                            crate::modal::pane::historical_download::Action::EstimateRequested { ticker, schema, date_range } => {
                                let Some(service) = self.market_data_service.clone() else {
                                    self.notifications.push(Toast::error(
                                        "Databento API key not configured. Set it in connection settings."
                                            .to_string(),
                                    ));
                                    return Task::none();
                                };
                                let schema_discriminant = schema as u16;
                                return Task::perform(
                                    async move {
                                        service.estimate_data_request(&ticker, schema_discriminant, &date_range).await
                                            .map_err(|e| e.to_string())
                                    },
                                    move |result| Message::HistoricalDownloadCostEstimated { result }
                                );
                            }
                            crate::modal::pane::historical_download::Action::DownloadRequested { ticker, schema, date_range } => {
                                let Some(service) = self.market_data_service.clone() else {
                                    self.notifications.push(Toast::error("Databento API key required".to_string()));
                                    return Task::none();
                                };
                                let schema_discriminant = schema as u16;
                                let download_id = uuid::Uuid::new_v4();
                                self.historical_download_id = Some(download_id);
                                {
                                    let mut progress = get_download_progress().lock().unwrap();
                                    progress.insert(download_id, (0, date_range.num_days() as usize));
                                }
                                let ticker_clone = ticker.clone();
                                let date_range_clone = date_range.clone();
                                return Task::perform(
                                    async move {
                                        service.download_to_cache_with_progress(
                                            &ticker,
                                            schema_discriminant,
                                            &date_range,
                                            Box::new(move |current, total| {
                                                if let Ok(mut progress) = get_download_progress().lock() {
                                                    progress.insert(download_id, (current, total));
                                                }
                                            })
                                        ).await
                                            .map_err(|e| e.to_string())
                                    },
                                    move |result| Message::HistoricalDownloadComplete {
                                        ticker: ticker_clone,
                                        date_range: date_range_clone,
                                        result,
                                    }
                                );
                            }
                            crate::modal::pane::historical_download::Action::DatasetCreated(feed) => {
                                let mut feed_manager = self.data_feed_manager.lock()
                                    .unwrap_or_else(|e| e.into_inner());
                                let feed_id = feed.id;
                                feed_manager.add(feed);
                                self.data_feeds_modal.sync_snapshot(&feed_manager);
                                self.connections_menu.sync_snapshot(&feed_manager);
                                drop(feed_manager);
                                self.historical_download_modal = None;
                                self.historical_download_id = None;
                                let windows = std::collections::HashMap::new();
                                self.save_state_to_disk(&windows);
                            }
                            crate::modal::pane::historical_download::Action::ApiKeySaved { provider, key } => {
                                let secrets = data::SecretsManager::new();
                                if let Err(e) = secrets.set_api_key(provider, &key) {
                                    log::warn!("Failed to save API key: {}", e);
                                } else {
                                    log::info!("API key saved for {:?}", provider);
                                    // Reinitialize service
                                    return Task::done(Message::ReinitializeService(provider));
                                }
                            }
                            crate::modal::pane::historical_download::Action::Closed => {
                                self.historical_download_modal = None;
                                self.historical_download_id = None;
                            }
                        }
                    }
                }
            }
            Message::HistoricalDownloadCostEstimated { result } => {
                if let Some(modal) = &mut self.historical_download_modal {
                    match result {
                        Ok((total_days, cached_days, uncached_days, _gaps_desc, actual_cost_usd, cached_dates)) => {
                            modal.set_cache_status(
                                crate::modal::pane::historical_download::CacheStatus {
                                    total_days,
                                    cached_days,
                                    uncached_days,
                                },
                                cached_dates,
                            );
                            modal.set_actual_cost(actual_cost_usd);
                        }
                        Err(e) => {
                            log::error!("Historical download cost estimation failed: {}", e);
                            self.notifications.push(Toast::error(format!("Estimation failed: {}", e)));
                        }
                    }
                }
            }
            Message::HistoricalDownloadComplete { ticker, date_range, result } => {
                match result {
                    Ok(days_downloaded) => {
                        log::info!("Historical download complete: {} days for {}", days_downloaded, ticker);
                        self.notifications.push(Toast::new(Notification::Info(
                            format!("Downloaded {} days of data", days_downloaded)
                        )));

                        self.downloaded_tickers.lock().unwrap().register(ticker.clone(), date_range.clone());
                        let ticker_symbols: std::collections::HashSet<String> =
                            self.downloaded_tickers.lock().unwrap().list_tickers().into_iter().collect();
                        self.tickers_table.set_cached_filter(ticker_symbols);

                        // Create the dataset feed
                        if let Some(modal) = &self.historical_download_modal {
                            let name = modal.auto_name();
                            let schema_idx = modal.selected_schema_idx();
                            let (_, schema_name, _) = crate::modal::pane::SCHEMAS[schema_idx];
                            let ticker_idx = modal.selected_ticker_idx();
                            let (ticker_sym, _) = crate::modal::pane::FUTURES_PRODUCTS[ticker_idx];

                            let info = data::HistoricalDatasetInfo {
                                ticker: ticker_sym.to_string(),
                                date_range: date_range.clone(),
                                schema: schema_name.to_string(),
                                trade_count: None,
                                file_size_bytes: None,
                            };
                            let feed = data::DataFeed::new_historical_databento(name, info);
                            let feed_id = feed.id;

                            let mut feed_manager = self.data_feed_manager.lock()
                                .unwrap_or_else(|e| e.into_inner());
                            feed_manager.add(feed);
                            self.data_feeds_modal.sync_snapshot(&feed_manager);
                            self.connections_menu.sync_snapshot(&feed_manager);
                            drop(feed_manager);

                            self.historical_download_modal = None;
                            self.historical_download_id = None;

                            let windows = std::collections::HashMap::new();
                            self.save_state_to_disk(&windows);
                        }
                    }
                    Err(e) => {
                        log::error!("Historical download failed: {}", e);
                        self.notifications.push(Toast::error(format!("Download failed: {}", e)));
                        if let Some(modal) = &mut self.historical_download_modal {
                            modal.set_download_progress(
                                crate::modal::pane::historical_download::DownloadProgress::Error(e)
                            );
                        }
                    }
                }
            }
            Message::DataFeeds(msg) => {
                let mut feed_manager = self
                    .data_feed_manager
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());

                if let Some(action) = self.data_feeds_modal.update(msg, &mut feed_manager) {
                    match action {
                        crate::modal::pane::data_feeds::Action::ConnectFeed(feed_id) => {
                            log::info!("Connect feed requested: {}", feed_id);

                            if let Some(feed) = feed_manager.get(feed_id) {
                                match feed.provider {
                                    data::FeedProvider::Databento => {
                                        let secrets = data::SecretsManager::new();
                                        if secrets.has_api_key(data::ApiProvider::Databento) {
                                            feed_manager.set_status(
                                                feed_id,
                                                data::FeedStatus::Connected,
                                            );
                                            self.connections_menu
                                                .sync_snapshot(&feed_manager);
                                            self.data_feeds_modal
                                                .sync_snapshot(&feed_manager);
                                            drop(feed_manager);

                                            // Re-populate tickers table from
                                            // downloaded tickers registry
                                            let ticker_symbols: std::collections::HashSet<String> =
                                                self.downloaded_tickers
                                                    .lock()
                                                    .unwrap()
                                                    .list_tickers()
                                                    .into_iter()
                                                    .collect();
                                            self.tickers_table
                                                .set_cached_filter(ticker_symbols);
                                            log::info!(
                                                "Databento feed connected - restored ticker list"
                                            );

                                            // Re-affiliate disconnected panes and
                                            // reload any that were in error state
                                            let main_window = self.main_window.id;
                                            let mut reload_tasks = Vec::new();
                                            let active_layout_id = self
                                                .layout_manager
                                                .active_layout_id()
                                                .map(|id| id.unique);
                                            for layout in &mut self.layout_manager.layouts {
                                                let reloads = layout
                                                    .dashboard
                                                    .affiliate_and_collect_reloads(
                                                        feed_id, main_window,
                                                    );
                                                for (pane_id, config, ticker_info) in reloads {
                                                    if let Some(lid) = active_layout_id {
                                                        reload_tasks.push(Task::done(
                                                            Message::LoadChartData {
                                                                layout_id: lid,
                                                                pane_id,
                                                                config,
                                                                ticker_info,
                                                            },
                                                        ));
                                                    }
                                                }
                                            }

                                            if !reload_tasks.is_empty() {
                                                log::info!(
                                                    "Reloading {} pane(s) after reconnect",
                                                    reload_tasks.len()
                                                );
                                                return Task::batch(reload_tasks);
                                            }
                                            return Task::none();
                                        } else {
                                            self.data_feeds_modal.sync_snapshot(&feed_manager);
                                            self.notifications.push(Toast::error(
                                                "Databento API key not configured. Set it in connection settings."
                                                    .to_string(),
                                            ));
                                            return Task::none();
                                        }
                                    }
                                    data::FeedProvider::Rithmic => {
                                        let secrets = data::SecretsManager::new();
                                        let password_status =
                                            secrets.get_api_key(data::ApiProvider::Rithmic);

                                        if let Some(password) = password_status.key() {
                                            let rithmic_config = match &feed.config {
                                                data::feed::FeedConfig::Rithmic(cfg) => {
                                                    cfg.clone()
                                                }
                                                _ => unreachable!(),
                                            };
                                            let password = password.to_string();

                                            feed_manager.set_status(
                                                feed_id,
                                                data::FeedStatus::Connecting,
                                            );
                                            self.data_feeds_modal
                                                .sync_snapshot(&feed_manager);
                                            drop(feed_manager);

                                            // Run on blocking thread since rithmic_rs
                                            // futures are not Send
                                            return Task::perform(
                                                async move {
                                                    let handle =
                                                        tokio::runtime::Handle::current();
                                                    tokio::task::spawn_blocking(move || {
                                                        handle.block_on(async {
                                                            let result =
                                                                tokio::time::timeout(
                                                                    std::time::Duration::from_secs(
                                                                        30,
                                                                    ),
                                                                    services::initialize_rithmic_service(
                                                                        &rithmic_config,
                                                                        &password,
                                                                    ),
                                                                )
                                                                .await;
                                                            match result {
                                                                Ok(Ok(service_result)) => {
                                                                    let mut staging =
                                                                        super::get_rithmic_service_staging()
                                                                            .lock()
                                                                            .unwrap_or_else(|e| {
                                                                                e.into_inner()
                                                                            });
                                                                    *staging =
                                                                        Some(service_result);
                                                                    Ok(())
                                                                }
                                                                Ok(Err(e)) => Err(e),
                                                                Err(_) => Err(
                                                                    "Connection timed out after 30 seconds"
                                                                        .to_string(),
                                                                ),
                                                            }
                                                        })
                                                    })
                                                    .await
                                                    .map_err(|e| {
                                                        format!("Task join error: {}", e)
                                                    })?
                                                },
                                                move |result| Message::RithmicConnected {
                                                    feed_id,
                                                    result,
                                                },
                                            );
                                        } else {
                                            self.data_feeds_modal
                                                .sync_snapshot(&feed_manager);
                                            self.notifications.push(Toast::error(
                                                "Rithmic password not configured. Set it in connection settings."
                                                    .to_string(),
                                            ));
                                            return Task::none();
                                        }
                                    }
                                }
                            }
                        }
                        crate::modal::pane::data_feeds::Action::DisconnectFeed(feed_id) => {
                            log::info!("Disconnect feed requested: {}", feed_id);

                            let provider = feed_manager.get(feed_id).map(|f| f.provider);

                            // Check if this is the active Rithmic feed
                            if self.rithmic_feed_id == Some(feed_id) {
                                let client = self.rithmic_client.take();
                                self.rithmic_trade_repo = None;
                                self.rithmic_depth_repo = None;
                                self.rithmic_feed_id = None;

                                if let Some(client) = client {
                                    feed_manager.set_status(
                                        feed_id,
                                        data::FeedStatus::Disconnected,
                                    );
                                    self.data_feeds_modal
                                        .sync_snapshot(&feed_manager);
                                    self.connections_menu
                                        .sync_snapshot(&feed_manager);
                                    drop(feed_manager);

                                    return Task::perform(
                                        async move {
                                            client.lock().await.disconnect().await;
                                        },
                                        move |_| {
                                            Message::DataFeeds(
                                                crate::modal::pane::data_feeds::DataFeedsMessage::FeedStatusChanged(
                                                    feed_id,
                                                    data::FeedStatus::Disconnected,
                                                ),
                                            )
                                        },
                                    );
                                }
                            }

                            feed_manager.set_status(
                                feed_id,
                                data::FeedStatus::Disconnected,
                            );

                            if provider == Some(data::FeedProvider::Databento) {
                                // Check if another Databento feed is still connected
                                let alt_feed_id = feed_manager
                                    .connected_feed_id_for_provider(
                                        data::FeedProvider::Databento,
                                    );

                                self.connections_menu.sync_snapshot(&feed_manager);
                                self.data_feeds_modal.sync_snapshot(&feed_manager);
                                drop(feed_manager);

                                let main_window = self.main_window.id;
                                if let Some(alt_fid) = alt_feed_id {
                                    // Another Databento feed is connected - silently
                                    // re-affiliate panes
                                    for layout in &mut self.layout_manager.layouts {
                                        let reloads = layout
                                            .dashboard
                                            .affiliate_and_collect_reloads(
                                                alt_fid, main_window,
                                            );
                                        if !reloads.is_empty() {
                                            log::info!(
                                                "Re-affiliated panes to alt feed {}",
                                                alt_fid
                                            );
                                        }
                                    }
                                } else {
                                    // No other feed connected - keep charts visible
                                    // but mark panes as unaffiliated
                                    for layout in &mut self.layout_manager.layouts {
                                        layout
                                            .dashboard
                                            .unaffiliate_panes_for_feed(
                                                feed_id, main_window,
                                            );
                                    }
                                    self.notifications.push(Toast::warn(
                                        "Feed disconnected. Charts preserved - will reload when reconnected."
                                            .to_string(),
                                    ));
                                }
                            } else {
                                self.connections_menu.sync_snapshot(&feed_manager);
                                self.data_feeds_modal.sync_snapshot(&feed_manager);
                            }
                            return Task::none();
                        }
                        crate::modal::pane::data_feeds::Action::FeedsUpdated => {
                            log::info!("Data feeds updated, persisting to disk");
                            drop(feed_manager);
                            let windows = std::collections::HashMap::new();
                            self.save_state_to_disk(&windows);
                            let feed_manager = self
                                .data_feed_manager
                                .lock()
                                .unwrap_or_else(|e| e.into_inner());
                            self.data_feeds_modal.sync_snapshot(&feed_manager);
                            self.connections_menu.sync_snapshot(&feed_manager);
                            return Task::none();
                        }
                        crate::modal::pane::data_feeds::Action::OpenHistoricalDownload => {
                            drop(feed_manager);
                            self.historical_download_modal = Some(
                                crate::modal::pane::historical_download::HistoricalDownloadModal::new()
                            );
                            return Task::none();
                        }
                        crate::modal::pane::data_feeds::Action::LoadPreview(feed_id, info) => {
                            drop(feed_manager);
                            if let Some(service) = self.market_data_service.clone() {
                                let ticker_str = info.ticker.clone();
                                let date_range = info.date_range.clone();
                                let schema = info.schema.clone();
                                return Task::perform(
                                    async move {
                                        let ticker = data::FuturesTicker::new(
                                            &ticker_str,
                                            data::FuturesVenue::CMEGlobex,
                                        );
                                        let trades = service
                                            .get_trades_for_preview(&ticker, &date_range)
                                            .await
                                            .map_err(|e| e.to_string())?;

                                        // Build preview data from trades
                                        let total_trades = trades.len();

                                        // Sample price line (every Nth trade)
                                        let step = (total_trades / 200).max(1);
                                        let price_line: Vec<(u64, f64)> = trades
                                            .iter()
                                            .step_by(step)
                                            .map(|t| (t.time.0, t.price.to_f64()))
                                            .collect();

                                        // First 100 trades for the table
                                        let trade_rows: Vec<
                                            crate::modal::pane::data_feeds::TradePreviewRow,
                                        > = trades
                                            .iter()
                                            .take(100)
                                            .map(|t| {
                                                let dt = chrono::DateTime::from_timestamp_millis(
                                                    t.time.0 as i64,
                                                );
                                                let time_str = dt
                                                    .map(|d| d.format("%H:%M:%S%.3f").to_string())
                                                    .unwrap_or_default();
                                                crate::modal::pane::data_feeds::TradePreviewRow {
                                                    time: time_str,
                                                    price: format!("{:.2}", t.price.to_f64()),
                                                    size: format!("{}", t.quantity.0 as u32),
                                                    side: match t.side {
                                                        data::Side::Buy => "Buy".to_string(),
                                                        data::Side::Sell => "Sell".to_string(),
                                                        _ => "?".to_string(),
                                                    },
                                                }
                                            })
                                            .collect();

                                        let date_range_str = format!(
                                            "{} - {}",
                                            date_range.start, date_range.end
                                        );

                                        Ok(crate::modal::pane::data_feeds::PreviewData {
                                            feed_id,
                                            price_line,
                                            trades: trade_rows,
                                            total_trades,
                                            date_range_str,
                                        })
                                    },
                                    move |result| Message::DataFeedPreviewLoaded {
                                        feed_id,
                                        result,
                                    },
                                );
                            }
                            return Task::none();
                        }
                    }
                }

                // Sync snapshot after any update
                self.data_feeds_modal.sync_snapshot(&feed_manager);
            }
            Message::DataFeedPreviewLoaded { feed_id, result } => {
                self.data_feeds_modal.update(
                    crate::modal::pane::data_feeds::DataFeedsMessage::PreviewLoaded(
                        feed_id, result,
                    ),
                    &mut *self
                        .data_feed_manager
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()),
                );
            }
            Message::AudioStream(message) => self.audio_stream.update(message),
            Message::ReinitializeService(provider) => {
                match provider {
                    data::ApiProvider::Databento => {
                        log::info!("Reinitializing Databento service with new API key...");
                        // Reinitialize market data service
                        if let Some(result) = services::initialize_market_data_service() {
                            self.market_data_service = Some(result.service.clone());
                            self.replay_engine = services::create_replay_engine(Some(&result));
                            self.notifications.push(Toast::new(Notification::Info(
                                "Databento service initialized".to_string()
                            )));
                        } else {
                            self.notifications.push(Toast::error(
                                "Failed to initialize Databento service".to_string()
                            ));
                        }
                    }
                    data::ApiProvider::Massive => {
                        log::info!("Reinitializing Massive service with new API key...");
                        // Reinitialize options services
                        let (options_service, _) = services::initialize_options_services();
                        self.options_service = options_service;
                        if self.options_service.is_some() {
                            self.notifications.push(Toast::new(Notification::Info(
                                "Options service initialized".to_string()
                            )));
                        }
                    }
                    data::ApiProvider::Rithmic => {
                        log::info!("Reinitializing Rithmic service with new password...");
                        if let Some(feed_id) = self.rithmic_feed_id {
                            return Task::done(Message::DataFeeds(
                                crate::modal::pane::data_feeds::DataFeedsMessage::ConnectFeed(
                                    feed_id,
                                ),
                            ));
                        } else {
                            self.notifications.push(Toast::new(Notification::Info(
                                "Rithmic password saved. Configure a Rithmic feed to connect."
                                    .to_string(),
                            )));
                        }
                    }
                }
            }
            Message::RithmicConnected { feed_id, result } => {
                match result {
                    Ok(()) => {
                        // Take service result from global staging
                        let service_result = {
                            let mut staging =
                                super::get_rithmic_service_staging()
                                    .lock()
                                    .unwrap_or_else(|e| e.into_inner());
                            staging.take()
                        };

                        if let Some(sr) = service_result {
                            self.rithmic_client = Some(sr.client.clone());
                            self.rithmic_trade_repo = Some(sr.trade_repo);
                            self.rithmic_depth_repo = Some(sr.depth_repo);
                            self.rithmic_feed_id = Some(feed_id);

                            let mut feed_manager = self
                                .data_feed_manager
                                .lock()
                                .unwrap_or_else(|e| e.into_inner());
                            feed_manager
                                .set_status(feed_id, data::FeedStatus::Connected);

                            let subscribed_tickers =
                                if let Some(feed) = feed_manager.get(feed_id) {
                                    if let data::feed::FeedConfig::Rithmic(cfg) =
                                        &feed.config
                                    {
                                        cfg.subscribed_tickers.clone()
                                    } else {
                                        vec![]
                                    }
                                } else {
                                    vec![]
                                };

                            self.data_feeds_modal.sync_snapshot(&feed_manager);
                            drop(feed_manager);

                            self.notifications.push(Toast::new(Notification::Info(
                                "Rithmic connected".to_string(),
                            )));

                            // Spawn background streaming task
                            let client = sr.client.clone();
                            let events_buf = super::get_rithmic_events().clone();

                            return Task::perform(
                                async move {
                                    // Subscribe to configured tickers
                                    {
                                        let mut guard = client.lock().await;
                                        for ticker in &subscribed_tickers {
                                            if let Err(e) =
                                                guard.subscribe(ticker, "CME").await
                                            {
                                                log::warn!(
                                                    "Failed to subscribe to {}: {}",
                                                    ticker,
                                                    e
                                                );
                                            }
                                        }
                                    }

                                    // Take ticker handle and start streaming
                                    let handle = {
                                        let mut guard = client.lock().await;
                                        guard.take_ticker_handle()
                                    };

                                    if let Some(handle) = handle {
                                        let (event_tx, mut event_rx) =
                                            tokio::sync::mpsc::unbounded_channel();
                                        let stream =
                                            exchange::RithmicStream::new(handle);
                                        // Use a generic ES ticker info for
                                        // stream identification
                                        let default_ticker =
                                            exchange::FuturesTicker::new(
                                                "ES",
                                                exchange::FuturesVenue::CMEGlobex,
                                            );
                                        let stream_kind =
                                            exchange::adapter::StreamKind::DepthAndTrades {
                                                ticker_info:
                                                    exchange::FuturesTickerInfo::new(
                                                        default_ticker,
                                                        0.25,
                                                        1.0,
                                                        50.0,
                                                    ),
                                                depth_aggr:
                                                    exchange::adapter::StreamTicksize::Client,
                                                push_freq:
                                                    exchange::PushFrequency::ServerDefault,
                                            };

                                        // Spawn stream reader that pushes to global buffer
                                        let buf = events_buf.clone();
                                        tokio::spawn(async move {
                                            stream.run(stream_kind, event_tx).await;
                                        });

                                        // Read events from channel, push to global buffer
                                        while let Some(event) = event_rx.recv().await {
                                            if let Ok(mut buf) = buf.lock() {
                                                buf.push(event);
                                            }
                                        }
                                    }
                                },
                                |_| {
                                    Message::RithmicStreamEvent(
                                        exchange::Event::ConnectionLost,
                                    )
                                },
                            );
                        } else {
                            log::error!(
                                "Rithmic service result not found in staging"
                            );
                            self.notifications.push(Toast::error(
                                "Internal error: Rithmic service result lost"
                                    .to_string(),
                            ));
                        }
                    }
                    Err(e) => {
                        let mut feed_manager = self
                            .data_feed_manager
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        feed_manager.set_status(
                            feed_id,
                            data::FeedStatus::Error(e.clone()),
                        );
                        self.data_feeds_modal.sync_snapshot(&feed_manager);
                        self.notifications.push(Toast::error(format!(
                            "Rithmic connection failed: {}",
                            e
                        )));
                    }
                }
            }
            Message::RithmicStreamEvent(event) => {
                match event {
                    exchange::Event::TradeReceived(stream_kind, ref _trade) => {
                        // Guard: only route if Rithmic is still connected
                        if self.rithmic_feed_id.is_none() {
                            return Task::none();
                        }
                        // Route trade events to active dashboard panes
                        return Task::done(Message::Dashboard {
                            layout_id: None,
                            event: dashboard::Message::ExchangeEvent(
                                exchange::Event::TradeReceived(
                                    stream_kind,
                                    _trade.clone(),
                                ),
                            ),
                        });
                    }
                    exchange::Event::DepthReceived(
                        stream_kind,
                        ts,
                        ref depth,
                        ref trades,
                    ) => {
                        // Guard: only route if Rithmic is still connected
                        if self.rithmic_feed_id.is_none() {
                            return Task::none();
                        }
                        // Route depth events to active dashboard panes
                        return Task::done(Message::Dashboard {
                            layout_id: None,
                            event: dashboard::Message::ExchangeEvent(
                                exchange::Event::DepthReceived(
                                    stream_kind,
                                    ts,
                                    depth.clone(),
                                    trades.clone(),
                                ),
                            ),
                        });
                    }
                    exchange::Event::ConnectionLost => {
                        if let Some(feed_id) = self.rithmic_feed_id {
                            let mut feed_manager = self
                                .data_feed_manager
                                .lock()
                                .unwrap_or_else(|e| e.into_inner());
                            let auto_reconnect = feed_manager
                                .get(feed_id)
                                .and_then(|f| f.rithmic_config())
                                .map(|c| c.auto_reconnect)
                                .unwrap_or(false);

                            if auto_reconnect {
                                feed_manager.set_status(
                                    feed_id,
                                    data::FeedStatus::Connecting,
                                );
                                self.data_feeds_modal
                                    .sync_snapshot(&feed_manager);
                                drop(feed_manager);
                                self.notifications.push(Toast::new(
                                    Notification::Info(
                                        "Rithmic reconnecting...".to_string(),
                                    ),
                                ));
                                return Task::done(Message::DataFeeds(
                                    crate::modal::pane::data_feeds::DataFeedsMessage::ConnectFeed(feed_id),
                                ));
                            } else {
                                feed_manager.set_status(
                                    feed_id,
                                    data::FeedStatus::Error(
                                        "Connection lost".to_string(),
                                    ),
                                );
                                self.data_feeds_modal
                                    .sync_snapshot(&feed_manager);
                                self.notifications.push(Toast::error(
                                    "Rithmic connection lost".to_string(),
                                ));
                            }
                        }
                    }
                    _ => {}
                }
            }
            Message::DataFolderRequested => {
                if let Err(err) = data::open_data_folder() {
                    self.notifications
                        .push(Toast::error(format!("Failed to open data folder: {err}")));
                }
            }
            Message::ThemeEditor(msg) => {
                let action = self.theme_editor.update(msg, &self.theme.clone().into());

                match action {
                    Some(crate::modal::theme_editor::Action::Exit) => {
                        self.sidebar.set_menu(Some(data::sidebar::Menu::Settings));
                    }
                    Some(crate::modal::theme_editor::Action::UpdateTheme(theme)) => {
                        self.theme = data::Theme(theme);

                        let main_window = self.main_window.id;

                        self.active_dashboard_mut()
                            .invalidate_all_panes(main_window);
                    }
                    None => {}
                }
            }
            Message::Sidebar(message) => {
                // Handle date range preset change - update all dashboards
                if let dashboard::sidebar::Message::SetDateRangePreset(preset) = &message {
                    self.layout_manager.set_date_range_preset(*preset);
                }

                // Sync feed manager snapshot when opening Connections or DataFeeds menu
                if matches!(
                    &message,
                    dashboard::sidebar::Message::ToggleSidebarMenu(Some(
                        data::sidebar::Menu::Connections
                    ))
                ) {
                    let feed_manager = self
                        .data_feed_manager
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    self.connections_menu.sync_snapshot(&feed_manager);
                }

                if let dashboard::sidebar::Message::ToggleSidebarMenu(Some(data::sidebar::Menu::DataFeeds)) = &message {
                    let feed_manager = self
                        .data_feed_manager
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    self.data_feeds_modal.sync_snapshot(&feed_manager);
                }

                // Trigger initial estimation when opening DataFeeds menu
                if let dashboard::sidebar::Message::ToggleSidebarMenu(Some(data::sidebar::Menu::DataFeeds)) = &message {
                    if let Some(action) = self.data_management_panel.request_initial_estimation() {
                        match action {
                            crate::modal::pane::data_management::Action::EstimateRequested {
                                ticker,
                                schema,
                                date_range,
                            } => {
                                // Process the sidebar message first, then trigger estimation
                                let (task, _) = self.sidebar.update(message);

                                return task.map(Message::Sidebar).chain(Task::done(
                                    Message::EstimateDataCost {
                                        pane_id: uuid::Uuid::nil(),
                                        ticker,
                                        schema,
                                        date_range,
                                    },
                                ));
                            }
                            crate::modal::pane::data_management::Action::DownloadRequested { .. } => {
                                // Shouldn't happen on initial open, but handle it anyway
                            }
                        }
                    }

                let (task, drawing_action) = self.sidebar.update(message);

                // Handle drawing tool actions from the sidebar
                if let Some(action) = drawing_action {
                    match action {
                        crate::modal::drawing_tools::Action::SelectTool(tool) => {
                            return task.map(Message::Sidebar).chain(Task::done(Message::Dashboard {
                                layout_id: None,
                                event: dashboard::Message::DrawingToolSelected(tool),
                            }));
                        }
                        crate::modal::drawing_tools::Action::ToggleSnap => {
                            return task.map(Message::Sidebar).chain(Task::done(Message::Dashboard {
                                layout_id: None,
                                event: dashboard::Message::DrawingSnapToggled,
                            }));
                        }
                    }
                }

                return task.map(Message::Sidebar);
            }
            Message::TickersTable(msg) => {
                let action = self.tickers_table.update(msg);

                match action {
                    Some(tickers_table::Action::Fetch(task)) => {
                        return task.map(Message::TickersTable);
                    }
                    Some(tickers_table::Action::ErrorOccurred(err)) => {
                        self.notifications.push(Toast::error(err.to_string()));
                    }
                    Some(tickers_table::Action::FocusWidget(id)) => {
                        return iced::widget::operation::focus(id);
                    }
                    // TickerSelected is handled by pane modals directly, not here
                    Some(tickers_table::Action::TickerSelected(_, _)) => {}
                    None => {}
                }
            }
        }
        Task::none()
    }

}
