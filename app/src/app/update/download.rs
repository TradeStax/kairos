use iced::Task;

use crate::components::display::toast::{Notification, Toast};
use crate::screen::dashboard;
use super::super::{DownloadMessage, Kairos, Message};
use super::super::core::globals::{get_download_sender, DownloadProgressEvent};

impl Kairos {
    pub(crate) fn handle_estimate_data_cost(
        &mut self,
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        schema: exchange::DownloadSchema,
        date_range: data::DateRange,
    ) -> Task<Message> {
        log::info!("EstimateDataCost message received");
        log::info!(
            "Ticker={:?}, Schema={:?}, Range={:?}",
            ticker,
            schema,
            date_range
        );

        let Some(service) = self.require_market_service() else {
            return Task::none();
        };

        let schema_discriminant = schema.as_discriminant();
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
        result: Result<data::DataRequestEstimate, String>,
    ) -> Task<Message> {
        match result {
            Ok(estimate) => {
                let cached_days = estimate.cached_dates.len();
                log::info!(
                    "Cost estimated: {}/{} days cached, ${:.4} USD",
                    cached_days,
                    estimate.total_days,
                    estimate.estimated_cost_usd
                );

                if pane_id == uuid::Uuid::nil() {
                    self.modals.data_management_panel.set_cache_status(
                        crate::modals::download::CacheStatus {
                            total_days: estimate.total_days,
                            cached_days,
                            uncached_days: estimate.uncached_count,
                            gaps_description: None,
                        },
                        estimate.cached_dates.clone(),
                    );
                    self.modals.data_management_panel
                        .set_actual_cost(estimate.estimated_cost_usd);
                } else {
                    log::info!(
                        "Cost estimated for pane {}: {}/{} days cached",
                        pane_id,
                        cached_days,
                        estimate.total_days
                    );
                }
            }
            Err(e) => {
                log::error!("Failed to estimate cost: {}", e);
                self.ui.notifications
                    .push(Toast::error(format!("Estimation failed: {}", e)));
            }
        }
        Task::none()
    }

    pub(crate) fn handle_download_data(
        &mut self,
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        schema: exchange::DownloadSchema,
        date_range: data::DateRange,
    ) -> Task<Message> {
        let Some(service) = self.require_market_service() else {
            return Task::none();
        };

        let schema_discriminant = schema.as_discriminant();
        let ticker_clone = ticker;
        let date_range_clone = date_range;

        // Send initial 0/N progress event
        let _ = get_download_sender().send(DownloadProgressEvent {
            pane_id,
            current: 0,
            total: date_range.num_days() as usize,
        });

        let sender = get_download_sender();
        Task::perform(
            async move {
                service
                    .download_to_cache_with_progress(
                        &ticker,
                        schema_discriminant,
                        &date_range,
                        Box::new(move |current, total| {
                            let _ = sender.send(DownloadProgressEvent {
                                pane_id,
                                current,
                                total,
                            });
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

        if self.modals.historical_download_id == Some(pane_id) {
            if let Some(modal) = &mut self.modals.historical_download_modal {
                modal.set_download_progress(
                    crate::modals::download::DownloadProgress::Downloading {
                        current_day: current,
                        total_days: total,
                    },
                );
            }
        } else if pane_id == uuid::Uuid::nil() {
            self.modals.data_management_panel.set_download_progress(
                crate::modals::download::DownloadProgress::Downloading {
                    current_day: current,
                    total_days: total,
                },
            );
        } else {
            return Task::done(Message::Dashboard {
                layout_id: self.persistence.layout_manager.active_layout_id().map(|l| l.unique),
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
        match result {
            Ok(days_downloaded) => {
                log::info!(
                    "Downloaded {} days for {} ({} to {})",
                    days_downloaded,
                    ticker,
                    date_range.start,
                    date_range.end
                );
                self.ui.notifications
                    .push(Toast::new(Notification::Info(format!(
                        "Successfully downloaded {} days of data",
                        days_downloaded
                    ))));

                data::lock_or_recover(&self.persistence.downloaded_tickers)
                    .register(ticker, date_range);
                log::info!("Registered {} in downloaded tickers registry", ticker);

                // Re-scan cache to rebuild the DataIndex with new data
                let cache_root =
                    crate::infra::platform::data_path(Some("cache/databento"));
                let scan_feed_id = {
                    let fm =
                        data::lock_or_recover(&self.connections.data_feed_manager);
                    fm.connected_feed_id_for_provider(data::FeedProvider::Databento)
                        .unwrap_or(uuid::Uuid::nil())
                };
                let scan_task = Task::perform(
                    async move {
                        exchange::scan_databento_cache(&cache_root, scan_feed_id)
                            .await
                    },
                    Message::DataIndexRebuilt,
                );

                if pane_id == uuid::Uuid::nil() {
                    self.modals.data_management_panel
                        .set_download_progress(
                            crate::modals::download::DownloadProgress::Idle,
                        );

                    let estimate_ticker = data::FuturesTicker::new(
                        crate::modals::download::FUTURES_PRODUCTS
                            [self.modals.data_management_panel.selected_ticker_idx()]
                        .0,
                        data::FuturesVenue::CMEGlobex,
                    );
                    let schema = crate::modals::download::SCHEMAS
                        [self.modals.data_management_panel.selected_schema_idx()]
                    .0;
                    let estimate_date_range =
                        self.modals.data_management_panel.current_date_range();

                    return Task::batch([
                        scan_task,
                        Task::done(Message::Download(
                            DownloadMessage::EstimateDataCost {
                                pane_id: uuid::Uuid::nil(),
                                ticker: estimate_ticker,
                                schema,
                                date_range: estimate_date_range,
                            },
                        )),
                    ]);
                } else {
                    let layout_id = self
                        .persistence.layout_manager
                        .active_layout_id()
                        .map(|id| id.unique)
                        .or_else(|| {
                            self.persistence.layout_manager
                                .layouts
                                .first()
                                .map(|l| l.id.unique)
                        });

                    let Some(layout_id) = layout_id else {
                        log::error!(
                            "No layout available for DataDownloadComplete"
                        );
                        return scan_task;
                    };

                    return Task::batch([
                        scan_task,
                        Task::done(Message::Dashboard {
                            layout_id: Some(layout_id),
                            event: dashboard::Message::DataDownloadComplete {
                                pane_id,
                                days_downloaded,
                            },
                        }),
                    ]);
                }
            }
            Err(e) => {
                log::error!("Failed to download data: {}", e);
                self.ui.notifications
                    .push(Toast::error(format!("Download failed: {}", e)));
            }
        }
        Task::none()
    }

    pub(crate) fn handle_api_key_setup(
        &mut self,
        msg: crate::modals::download::ApiKeySetupMessage,
    ) -> Task<Message> {
        if let Some(modal) = &mut self.modals.api_key_setup_modal
            && let Some(action) = modal.update(msg)
        {
            match action {
                crate::modals::download::api_key_modal::Action::Saved {
                    provider,
                    key,
                } => {
                    if let Err(e) = self.secrets.set_api_key(provider, &key) {
                        log::warn!("Failed to save API key: {}", e);
                        self.ui.notifications
                            .push(Toast::error(format!("Failed to save API key: {}", e)));
                        return Task::none();
                    }
                    log::info!("API key saved for {:?}", provider);
                    self.modals.api_key_setup_modal = None;
                    self.modals.historical_download_modal =
                        Some(crate::modals::download::HistoricalDownloadModal::new());
                    return Task::done(Message::ReinitializeService(provider));
                }
                crate::modals::download::api_key_modal::Action::Closed => {
                    self.modals.api_key_setup_modal = None;
                }
            }
        }
        Task::none()
    }

    pub(crate) fn handle_historical_download(
        &mut self,
        msg: crate::modals::download::HistoricalDownloadMessage,
    ) -> Task<Message> {
        if let Some(modal) = &mut self.modals.historical_download_modal
            && let Some(action) = modal.update(msg)
        {
            match action {
                crate::modals::download::historical::Action::EstimateRequested {
                    ticker,
                    schema,
                    date_range,
                } => {
                    let Some(service) = self.require_market_service() else {
                        return Task::none();
                    };
                    let schema_discriminant = schema.as_discriminant();
                    return Task::perform(
                        async move {
                            service
                                .estimate_data_request(&ticker, schema_discriminant, &date_range)
                                .await
                                .map_err(|e| e.to_string())
                        },
                        move |result| {
                            Message::Download(DownloadMessage::HistoricalDownloadCostEstimated {
                                result,
                            })
                        },
                    );
                }
                crate::modals::download::historical::Action::DownloadRequested {
                    ticker,
                    schema,
                    date_range,
                } => {
                    let Some(service) = self.require_market_service() else {
                        return Task::none();
                    };
                    let schema_discriminant = schema.as_discriminant();
                    let download_id = uuid::Uuid::new_v4();
                    self.modals.historical_download_id = Some(download_id);
                    // Send initial 0/N progress event
                    let _ = get_download_sender().send(DownloadProgressEvent {
                        pane_id: download_id,
                        current: 0,
                        total: date_range.num_days() as usize,
                    });
                    let ticker_clone = ticker;
                    let date_range_clone = date_range;
                    let sender = get_download_sender();
                    return Task::perform(
                        async move {
                            service
                                .download_to_cache_with_progress(
                                    &ticker,
                                    schema_discriminant,
                                    &date_range,
                                    Box::new(move |current, total| {
                                        let _ = sender.send(DownloadProgressEvent {
                                            pane_id: download_id,
                                            current,
                                            total,
                                        });
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
                crate::modals::download::historical::Action::Closed => {
                    self.modals.historical_download_modal = None;
                    self.modals.historical_download_id = None;
                }
            }
        }
        Task::none()
    }

    pub(crate) fn handle_historical_download_cost_estimated(
        &mut self,
        result: Result<data::DataRequestEstimate, String>,
    ) {
        if let Some(modal) = &mut self.modals.historical_download_modal {
            match result {
                Ok(estimate) => {
                    modal.set_cache_status(
                        crate::modals::download::CacheStatus {
                            total_days: estimate.total_days,
                            cached_days: estimate.cached_dates.len(),
                            uncached_days: estimate.uncached_count,
                            gaps_description: None,
                        },
                        estimate.cached_dates,
                    );
                    modal.set_actual_cost(estimate.estimated_cost_usd);
                }
                Err(e) => {
                    log::error!(
                        "Historical download cost estimation failed: {}",
                        e
                    );
                    self.ui.notifications
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
                self.ui.notifications
                    .push(Toast::new(Notification::Info(format!(
                        "Downloaded {} days of data",
                        days_downloaded
                    ))));

                data::lock_or_recover(&self.persistence.downloaded_tickers)
                    .register(ticker, date_range);

                // Add downloaded range to DataIndex immediately so
                // tickers appear before the async cache scan completes
                {
                    let mut dates = std::collections::BTreeSet::new();
                    for d in date_range.dates() {
                        dates.insert(d);
                    }
                    let key = data::DataKey {
                        ticker: ticker.to_string(),
                        schema: "trades".to_string(),
                    };
                    data::lock_or_recover(&self.persistence.data_index)
                        .add_contribution(key, uuid::Uuid::nil(), dates, false);
                }

                self.rebuild_ticker_data();

                // Create the dataset feed
                if let Some(modal) = &self.modals.historical_download_modal {
                    let name = modal.auto_name();
                    let schema_idx = modal.selected_schema_idx();
                    let (_, schema_name, _) = crate::modals::download::SCHEMAS[schema_idx];
                    let ticker_idx = modal.selected_ticker_idx();
                    let (ticker_sym, _) = crate::modals::download::FUTURES_PRODUCTS[ticker_idx];

                    let info = data::HistoricalDatasetInfo {
                        ticker: ticker_sym.to_string(),
                        date_range,
                        schema: schema_name.to_string(),
                        trade_count: None,
                        file_size_bytes: None,
                    };
                    let feed = data::DataFeed::new_historical_databento(name, info);
                    let _feed_id = feed.id;

                    let dm_arc = self.connections.data_feed_manager.clone();
                    let mut feed_manager = data::lock_or_recover(&dm_arc);
                    feed_manager.add(feed);
                    self.sync_feed_snapshots(&feed_manager);
                    drop(feed_manager);

                    self.modals.historical_download_modal = None;
                    self.modals.historical_download_id = None;

                    let windows = std::collections::HashMap::new();
                    self.save_state_to_disk(&windows);
                }
            }
            Err(e) => {
                log::error!("Historical download failed: {}", e);
                self.ui.notifications
                    .push(Toast::error(format!("Download failed: {}", e)));
                if let Some(modal) = &mut self.modals.historical_download_modal {
                    modal
                        .set_download_progress(
                            crate::modals::download::DownloadProgress::Error(e),
                        );
                }
            }
        }
    }
}
