use iced::Task;

use crate::screen::dashboard;
use crate::widget::toast::{Notification, Toast};

use super::super::{DownloadMessage, Flowsurface, Message, get_download_progress};

impl Flowsurface {
    pub(crate) fn handle_estimate_data_cost(
        &mut self,
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        schema: exchange::DatabentoSchema,
        date_range: data::DateRange,
    ) -> Task<Message> {
        log::info!("EstimateDataCost message received");
        log::info!(
            "Ticker={:?}, Schema={:?}, Range={:?}",
            ticker,
            schema,
            date_range
        );

        let Some(service) = self.market_data_service.clone() else {
            log::warn!("Market data service not available (API key not configured)");
            self.notifications.push(Toast::error(
                "Databento API key not configured. Set it in connection \
                 settings."
                    .to_string(),
            ));
            return Task::none();
        };

        let schema_discriminant = schema as u16;
        Task::perform(
            async move {
                log::info!("Async block entered, about to call service");
                let result = service
                    .estimate_data_request(&ticker, schema_discriminant, &date_range)
                    .await;
                log::info!("Service call completed, result success: {}", result.is_ok());
                if let Err(ref e) = result {
                    log::error!("Service error: {}", e);
                }
                result.map_err(|e| e.to_string())
            },
            move |result| {
                log::info!("Task finished, sending DataCostEstimated");
                Message::Download(DownloadMessage::DataCostEstimated { pane_id, result })
            },
        )
    }

    pub(crate) fn handle_data_cost_estimated(
        &mut self,
        pane_id: uuid::Uuid,
        result: Result<(usize, usize, usize, String, f64, Vec<chrono::NaiveDate>), String>,
    ) -> Task<Message> {
        match result {
            Ok((
                total_days,
                cached_days,
                uncached_days,
                gaps_desc,
                actual_cost_usd,
                cached_dates,
            )) => {
                log::info!(
                    "Cost estimated: {}/{} days cached, ${:.4} USD",
                    cached_days,
                    total_days,
                    actual_cost_usd
                );

                if pane_id == uuid::Uuid::nil() {
                    self.data_management_panel.set_cache_status(
                        crate::modal::pane::data_management::CacheStatus {
                            total_days,
                            cached_days,
                            uncached_days,
                            gaps_description: gaps_desc.clone(),
                        },
                        cached_dates,
                    );
                    self.data_management_panel.set_actual_cost(actual_cost_usd);
                } else {
                    let layout_id = self
                        .layout_manager
                        .active_layout_id()
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
                self.notifications
                    .push(Toast::error(format!("Estimation failed: {}", e)));
            }
        }
        Task::none()
    }

    pub(crate) fn handle_download_data(
        &mut self,
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        schema: exchange::DatabentoSchema,
        date_range: data::DateRange,
    ) -> Task<Message> {
        let Some(service) = self.market_data_service.clone() else {
            log::warn!("Market data service not available (API key not configured)");
            self.notifications.push(Toast::error(
                "Databento API key not configured. Set it in connection \
                 settings."
                    .to_string(),
            ));
            return Task::none();
        };

        let schema_discriminant = schema as u16;
        let ticker_clone = ticker.clone();
        let date_range_clone = date_range.clone();

        {
            let mut progress = get_download_progress().lock().unwrap();
            progress.insert(pane_id, (0, date_range.num_days() as usize));
        }

        Task::perform(
            async move {
                service
                    .download_to_cache_with_progress(
                        &ticker,
                        schema_discriminant,
                        &date_range,
                        Box::new(move |current, total| {
                            if let Ok(mut progress) = get_download_progress().lock() {
                                progress.insert(pane_id, (current, total));
                            }
                            log::info!("Download progress: {}/{} days", current, total);
                        }),
                    )
                    .await
                    .map_err(|e| e.to_string())
            },
            move |result| {
                Message::Download(DownloadMessage::DataDownloadComplete {
                    pane_id,
                    ticker: ticker_clone,
                    date_range: date_range_clone,
                    result,
                })
            },
        )
    }

    pub(crate) fn handle_download_progress(
        &mut self,
        pane_id: uuid::Uuid,
        current: usize,
        total: usize,
    ) -> Task<Message> {
        log::info!(
            "Download progress for pane {}: {}/{}",
            pane_id,
            current,
            total
        );

        if self.historical_download_id == Some(pane_id) {
            if let Some(modal) = &mut self.historical_download_modal {
                modal.set_download_progress(
                    crate::modal::pane::historical_download::DownloadProgress::Downloading {
                        current_day: current,
                        total_days: total,
                    },
                );
            }
        } else if pane_id == uuid::Uuid::nil() {
            self.data_management_panel.set_download_progress(
                crate::modal::pane::data_management::DownloadProgress::Downloading {
                    current_day: current,
                    total_days: total,
                },
            );
        } else {
            return Task::done(Message::Dashboard {
                layout_id: self.layout_manager.active_layout_id().map(|l| l.unique),
                event: dashboard::Message::DataDownloadProgress {
                    pane_id,
                    current,
                    total,
                },
            });
        }
        Task::none()
    }

    pub(crate) fn handle_download_complete(
        &mut self,
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        date_range: data::DateRange,
        result: Result<usize, String>,
    ) -> Task<Message> {
        {
            let mut progress = get_download_progress().lock().unwrap();
            progress.remove(&pane_id);
        }

        match result {
            Ok(days_downloaded) => {
                log::info!(
                    "Downloaded {} days for {} ({} to {})",
                    days_downloaded,
                    ticker,
                    date_range.start,
                    date_range.end
                );
                self.notifications
                    .push(Toast::new(Notification::Info(format!(
                        "Successfully downloaded {} days of data",
                        days_downloaded
                    ))));

                self.downloaded_tickers
                    .lock()
                    .unwrap()
                    .register(ticker.clone(), date_range);
                log::info!("Registered {} in downloaded tickers registry", ticker);

                let ticker_symbols: std::collections::HashSet<String> = self
                    .downloaded_tickers
                    .lock()
                    .unwrap()
                    .list_tickers()
                    .into_iter()
                    .collect();
                self.tickers_table.set_cached_filter(ticker_symbols);
                log::info!(
                    "Updated ticker list with {} tickers",
                    self.downloaded_tickers.lock().unwrap().count()
                );

                if pane_id == uuid::Uuid::nil() {
                    self.data_management_panel.set_download_progress(
                        crate::modal::pane::data_management::DownloadProgress::Idle,
                    );

                    let estimate_ticker = data::FuturesTicker::new(
                        crate::modal::pane::FUTURES_PRODUCTS
                            [self.data_management_panel.selected_ticker_idx()]
                        .0,
                        data::FuturesVenue::CMEGlobex,
                    );
                    let schema = crate::modal::pane::SCHEMAS
                        [self.data_management_panel.selected_schema_idx()]
                    .0;
                    let estimate_date_range = self.data_management_panel.current_date_range();

                    return Task::done(Message::Download(DownloadMessage::EstimateDataCost {
                        pane_id: uuid::Uuid::nil(),
                        ticker: estimate_ticker,
                        schema,
                        date_range: estimate_date_range,
                    }));
                } else {
                    let layout_id = self
                        .layout_manager
                        .active_layout_id()
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
                self.notifications
                    .push(Toast::error(format!("Download failed: {}", e)));
            }
        }
        Task::none()
    }

    pub(crate) fn handle_historical_download(
        &mut self,
        msg: crate::modal::pane::historical_download::HistoricalDownloadMessage,
    ) -> Task<Message> {
        if let Some(modal) = &mut self.historical_download_modal {
            if let Some(action) = modal.update(msg) {
                match action {
                    crate::modal::pane::historical_download::Action::EstimateRequested {
                        ticker,
                        schema,
                        date_range,
                    } => {
                        let Some(service) = self.market_data_service.clone() else {
                            self.notifications.push(Toast::error(
                                "Databento API key not configured. Set it \
                                 in connection settings."
                                    .to_string(),
                            ));
                            return Task::none();
                        };
                        let schema_discriminant = schema as u16;
                        return Task::perform(
                            async move {
                                service
                                    .estimate_data_request(
                                        &ticker,
                                        schema_discriminant,
                                        &date_range,
                                    )
                                    .await
                                    .map_err(|e| e.to_string())
                            },
                            move |result| {
                                Message::Download(
                                    DownloadMessage::HistoricalDownloadCostEstimated { result },
                                )
                            },
                        );
                    }
                    crate::modal::pane::historical_download::Action::DownloadRequested {
                        ticker,
                        schema,
                        date_range,
                    } => {
                        let Some(service) = self.market_data_service.clone() else {
                            self.notifications
                                .push(Toast::error("Databento API key required".to_string()));
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
                                service
                                    .download_to_cache_with_progress(
                                        &ticker,
                                        schema_discriminant,
                                        &date_range,
                                        Box::new(move |current, total| {
                                            if let Ok(mut progress) = get_download_progress().lock()
                                            {
                                                progress.insert(download_id, (current, total));
                                            }
                                        }),
                                    )
                                    .await
                                    .map_err(|e| e.to_string())
                            },
                            move |result| {
                                Message::Download(DownloadMessage::HistoricalDownloadComplete {
                                    ticker: ticker_clone,
                                    date_range: date_range_clone,
                                    result,
                                })
                            },
                        );
                    }
                    crate::modal::pane::historical_download::Action::DatasetCreated(feed) => {
                        let mut feed_manager = self
                            .data_feed_manager
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        let _feed_id = feed.id;
                        feed_manager.add(feed);
                        self.data_feeds_modal.sync_snapshot(&feed_manager);
                        self.connections_menu.sync_snapshot(&feed_manager);
                        drop(feed_manager);
                        self.historical_download_modal = None;
                        self.historical_download_id = None;
                        let windows = std::collections::HashMap::new();
                        self.save_state_to_disk(&windows);
                    }
                    crate::modal::pane::historical_download::Action::ApiKeySaved {
                        provider,
                        key,
                    } => {
                        let secrets = data::SecretsManager::new();
                        if let Err(e) = secrets.set_api_key(provider, &key) {
                            log::warn!("Failed to save API key: {}", e);
                        } else {
                            log::info!("API key saved for {:?}", provider);
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
        Task::none()
    }

    pub(crate) fn handle_historical_download_cost_estimated(
        &mut self,
        result: Result<(usize, usize, usize, String, f64, Vec<chrono::NaiveDate>), String>,
    ) {
        if let Some(modal) = &mut self.historical_download_modal {
            match result {
                Ok((
                    total_days,
                    cached_days,
                    uncached_days,
                    _gaps_desc,
                    actual_cost_usd,
                    cached_dates,
                )) => {
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
                    self.notifications
                        .push(Toast::error(format!("Estimation failed: {}", e)));
                }
            }
        }
    }

    pub(crate) fn handle_historical_download_complete(
        &mut self,
        ticker: data::FuturesTicker,
        date_range: data::DateRange,
        result: Result<usize, String>,
    ) {
        match result {
            Ok(days_downloaded) => {
                log::info!(
                    "Historical download complete: {} days for {}",
                    days_downloaded,
                    ticker
                );
                self.notifications
                    .push(Toast::new(Notification::Info(format!(
                        "Downloaded {} days of data",
                        days_downloaded
                    ))));

                self.downloaded_tickers
                    .lock()
                    .unwrap()
                    .register(ticker.clone(), date_range.clone());
                let ticker_symbols: std::collections::HashSet<String> = self
                    .downloaded_tickers
                    .lock()
                    .unwrap()
                    .list_tickers()
                    .into_iter()
                    .collect();
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
                    let _feed_id = feed.id;

                    let mut feed_manager = self
                        .data_feed_manager
                        .lock()
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
                self.notifications
                    .push(Toast::error(format!("Download failed: {}", e)));
                if let Some(modal) = &mut self.historical_download_modal {
                    modal.set_download_progress(
                        crate::modal::pane::historical_download::DownloadProgress::Error(e),
                    );
                }
            }
        }
    }
}
