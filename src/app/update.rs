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

                // Warn user if heatmap date range will be truncated
                if config.chart_type == data::ChartType::Heatmap
                    && config.date_range.num_days() > 1
                {
                    self.notifications.push(Toast::new(
                        Notification::Info(format!(
                            "Heatmap limited to 1 day (showing {}). \
                             Multi-day heatmaps are not supported.",
                            config.date_range.end
                        )),
                    ));
                }

                let Some(service) = self.market_data_service.clone() else {
                    log::warn!("Market data service not available (API key not configured)");
                    self.notifications.push(Toast::error("Databento API key required for chart data".to_string()));
                    return Task::done(Message::ShowApiKeyConfig {
                        provider: data::ApiProvider::Databento,
                        triggered_by: Some(crate::modal::api_key_config::TriggeredBy::DataDownload),
                    });
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
                    self.notifications.push(Toast::error("Databento API key required for cost estimation".to_string()));
                    return Task::done(Message::ShowApiKeyConfig {
                        provider: data::ApiProvider::Databento,
                        triggered_by: Some(crate::modal::api_key_config::TriggeredBy::DataDownload),
                    });
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
                        "Databento API key required for data download".to_string()
                    ));
                    return Task::done(Message::ShowApiKeyConfig {
                        provider: data::ApiProvider::Databento,
                        triggered_by: Some(crate::modal::api_key_config::TriggeredBy::DataDownload),
                    });
                };

                let schema_discriminant = schema as u16;
                let ticker_clone = ticker.clone();
                let date_range_clone = date_range.clone();

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

                if pane_id == uuid::Uuid::nil() {
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
                            .register(ticker.clone(), date_range);
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
            Message::AudioStream(message) => self.audio_stream.update(message),
            // API Key configuration
            Message::ApiKeyConfig(msg) => {
                if let Some(modal) = &mut self.api_key_config_modal {
                    if let Some(action) = modal.update(msg) {
                        match action {
                            crate::modal::api_key_config::Action::Close => {
                                self.api_key_config_modal = None;
                                self.sidebar.set_menu(None);
                            }
                            crate::modal::api_key_config::Action::ReinitializeService(provider) => {
                                // Immediately reinitialize service after save (don't close modal)
                                return Task::done(Message::ReinitializeService(provider));
                            }
                            crate::modal::api_key_config::Action::ShowError(err) => {
                                self.notifications.push(Toast::error(err));
                            }
                        }
                    }
                } else {
                    // Modal was shown inline without being stored, handle the message
                    let mut temp_modal = crate::modal::api_key_config::ApiKeyConfigModal::new(
                        data::ApiProvider::Databento,
                    );
                    if let Some(action) = temp_modal.update(msg) {
                        match action {
                            crate::modal::api_key_config::Action::Close => {
                                self.sidebar.set_menu(None);
                            }
                            crate::modal::api_key_config::Action::ReinitializeService(provider) => {
                                return Task::done(Message::ReinitializeService(provider));
                            }
                            crate::modal::api_key_config::Action::ShowError(err) => {
                                self.notifications.push(Toast::error(err));
                            }
                        }
                    }
                    // Store the modal for future updates
                    self.api_key_config_modal = Some(temp_modal);
                }
            }
            Message::ShowApiKeyConfig { provider, triggered_by } => {
                self.api_key_config_modal = Some(
                    crate::modal::api_key_config::ApiKeyConfigModal::new(provider, triggered_by)
                );
                self.sidebar.set_menu(Some(data::sidebar::Menu::ApiKeys));
            }
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

                // Check if we're opening the ApiKeys menu - need to initialize the modal
                if let dashboard::sidebar::Message::ToggleSidebarMenu(Some(data::sidebar::Menu::ApiKeys)) = &message {
                    if self.api_key_config_modal.is_none() {
                        self.api_key_config_modal = Some(
                            crate::modal::api_key_config::ApiKeyConfigModal::new(
                                data::ApiProvider::Databento,
                                None,
                            )
                        );
                    }
                }

                // Trigger initial estimation when opening DataManagement menu
                if let dashboard::sidebar::Message::ToggleSidebarMenu(Some(data::sidebar::Menu::DataManagement)) = &message {
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
