use iced::Task;

use super::super::{DownloadMessage, Kairos, Message};
use crate::components::display::toast::{Notification, Toast};
use crate::screen::dashboard;

/// Sentinel UUID representing the global data-management panel context
/// (as opposed to a specific pane). Used as a `pane_id` in download
/// messages when the operation originates from the sidebar data-management
/// panel rather than an individual chart pane.
pub(crate) const GLOBAL_PANE_ID: uuid::Uuid = uuid::Uuid::nil();

impl Kairos {
    pub(crate) fn handle_estimate_data_cost(
        &mut self,
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        _schema: data::DownloadSchema,
        date_range: data::DateRange,
    ) -> Task<Message> {
        let Some(engine) = self.require_data_engine() else {
            return Task::none();
        };

        let symbol = ticker.as_str().to_string();

        Task::perform(
            async move {
                let engine = engine.lock().await;
                let cached = engine
                    .list_cached_dates(
                        &symbol,
                        data::cache::CacheSchema::Trades,
                    )
                    .await;
                let cached_in_range: Vec<_> = cached
                    .into_iter()
                    .filter(|d| date_range.contains(*d))
                    .collect();
                Ok((date_range.num_days() as usize, cached_in_range))
            },
            move |result| {
                Message::Download(DownloadMessage::DataCostEstimated {
                    pane_id,
                    result,
                })
            },
        )
    }

    pub(crate) fn handle_data_cost_estimated(
        &mut self,
        pane_id: uuid::Uuid,
        result: Result<(usize, Vec<chrono::NaiveDate>), String>,
    ) -> Task<Message> {
        match result {
            Ok((total_days, cached_dates)) => {
                let cached_days = cached_dates.len();
                let uncached_days = total_days.saturating_sub(cached_days);
                log::info!("Cost estimated: {}/{} days cached", cached_days, total_days,);

                if pane_id == GLOBAL_PANE_ID {
                    self.modals.data_management_panel.set_cache_status(
                        crate::modals::download::CacheStatus {
                            total_days,
                            cached_days,
                            uncached_days,
                            gaps_description: None,
                        },
                        cached_dates,
                    );
                } else {
                    log::info!(
                        "Cost estimated for pane {}: {}/{} days cached",
                        pane_id,
                        cached_days,
                        total_days
                    );
                }
            }
            Err(e) => {
                log::error!("Failed to estimate cost: {}", e);
                self.ui
                    .push_notification(Toast::error(format!("Estimation failed: {}", e)));
            }
        }
        Task::none()
    }

    pub(crate) fn handle_download_data(
        &mut self,
        pane_id: uuid::Uuid,
        ticker: data::FuturesTicker,
        schema: data::DownloadSchema,
        date_range: data::DateRange,
    ) -> Task<Message> {
        let Some(engine) = self.require_data_engine() else {
            return Task::none();
        };

        let ticker_clone = ticker;
        let date_range_clone = date_range;

        Task::perform(
            async move {
                engine
                    .lock()
                    .await
                    .download_to_cache(&ticker, schema, &date_range)
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
        } else if pane_id == GLOBAL_PANE_ID {
            self.modals.data_management_panel.set_download_progress(
                crate::modals::download::DownloadProgress::Downloading {
                    current_day: current,
                    total_days: total,
                },
            );
        } else {
            return Task::done(Message::Dashboard {
                layout_id: self
                    .persistence
                    .layout_manager
                    .active_layout_id()
                    .map(|l| l.unique),
                event: Box::new(dashboard::Message::DataDownloadProgress {
                    pane_id,
                    current,
                    total,
                }),
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
                self.ui
                    .push_notification(Toast::new(Notification::Info(format!(
                        "Successfully downloaded {} days of data",
                        days_downloaded
                    ))));

                data::lock_or_recover(&self.persistence.downloaded_tickers)
                    .register(ticker, date_range);
                log::info!("Registered {} in downloaded tickers registry", ticker);

                // Re-scan cache to rebuild the DataIndex with new data
                let scan_task = if let Some(engine) = self.require_data_engine() {
                    Task::perform(
                        async move {
                            let eng = engine.lock().await;
                            let index = eng.scan_cache().await;
                            Ok(index)
                        },
                        Message::DataIndexRebuilt,
                    )
                } else {
                    // Fallback: Databento-specific scan
                    let cache_root =
                        crate::infra::platform::data_path(Some("cache/databento"));
                    let scan_feed_id = {
                        let fm =
                            data::lock_or_recover(&self.connections.connection_manager);
                        fm.connected_id_for_provider(data::ConnectionProvider::Databento)
                            .unwrap_or(Kairos::REGISTRY_SENTINEL_FEED)
                    };
                    Task::perform(
                        async move {
                            data::scan_databento_cache(&cache_root, scan_feed_id).await
                        },
                        Message::DataIndexRebuilt,
                    )
                };

                if pane_id == GLOBAL_PANE_ID {
                    self.modals
                        .data_management_panel
                        .set_download_progress(crate::modals::download::DownloadProgress::Idle);

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
                        Task::done(Message::Download(DownloadMessage::EstimateDataCost {
                            pane_id: GLOBAL_PANE_ID,
                            ticker: estimate_ticker,
                            schema,
                            date_range: estimate_date_range,
                        })),
                    ]);
                } else {
                    let layout_id = self
                        .persistence
                        .layout_manager
                        .active_layout_id()
                        .map(|id| id.unique)
                        .or_else(|| {
                            self.persistence
                                .layout_manager
                                .layouts
                                .first()
                                .map(|l| l.id.unique)
                        });

                    let Some(layout_id) = layout_id else {
                        log::error!("No layout available for DataDownloadComplete");
                        return scan_task;
                    };

                    return Task::batch([
                        scan_task,
                        Task::done(Message::Dashboard {
                            layout_id: Some(layout_id),
                            event: Box::new(dashboard::Message::DataDownloadComplete {
                                pane_id,
                                days_downloaded,
                            }),
                        }),
                    ]);
                }
            }
            Err(e) => {
                log::error!("Failed to download data: {}", e);
                self.ui
                    .push_notification(Toast::error(format!("Download failed: {}", e)));
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
                crate::modals::download::api_key_modal::Action::Saved { provider, key } => {
                    if let Err(e) = self.secrets.set_api_key(provider, &key) {
                        log::warn!("Failed to save API key: {}", e);
                        self.ui.push_notification(Toast::error(format!(
                            "Failed to save API key: {}",
                            e
                        )));
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
                    schema: _schema,
                    date_range,
                } => {
                    let Some(engine) = self.require_data_engine() else {
                        return Task::none();
                    };
                    let symbol = ticker.as_str().to_string();

                    return Task::perform(
                        async move {
                            let engine = engine.lock().await;
                            let cached = engine
                                .list_cached_dates(
                                    &symbol,
                                    data::cache::CacheSchema::Trades,
                                )
                                .await;
                            let cached_in_range: Vec<_> = cached
                                .into_iter()
                                .filter(|d| date_range.contains(*d))
                                .collect();
                            Ok((
                                date_range.num_days() as usize,
                                cached_in_range,
                            ))
                        },
                        move |result| {
                            Message::Download(
                                DownloadMessage::HistoricalDownloadCostEstimated {
                                    result,
                                },
                            )
                        },
                    );
                }
                crate::modals::download::historical::Action::DownloadRequested {
                    ticker,
                    schema,
                    date_range,
                } => {
                    let Some(engine) = self.require_data_engine() else {
                        return Task::none();
                    };
                    let download_id = uuid::Uuid::new_v4();
                    self.modals.historical_download_id = Some(download_id);
                    let ticker_clone = ticker;
                    let date_range_clone = date_range;
                    return Task::perform(
                        async move {
                            engine
                                .lock()
                                .await
                                .download_to_cache(&ticker, schema, &date_range)
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
        result: Result<(usize, Vec<chrono::NaiveDate>), String>,
    ) {
        if let Some(modal) = &mut self.modals.historical_download_modal {
            match result {
                Ok((total_days, cached_dates)) => {
                    let cached_days = cached_dates.len();
                    let uncached_days = total_days.saturating_sub(cached_days);
                    modal.set_cache_status(
                        crate::modals::download::CacheStatus {
                            total_days,
                            cached_days,
                            uncached_days,
                            gaps_description: None,
                        },
                        cached_dates,
                    );
                }
                Err(e) => {
                    log::error!("Historical download cost estimation failed: {}", e);
                    self.ui
                        .push_notification(Toast::error(format!("Estimation failed: {}", e)));
                }
            }
        }
    }

    pub(crate) fn handle_historical_download_complete(
        &mut self,
        ticker: data::FuturesTicker,
        date_range: data::DateRange,
        result: Result<usize, String>,
    ) -> Task<Message> {
        match result {
            Ok(days_downloaded) => {
                log::info!(
                    "Historical download complete: {} days for {}",
                    days_downloaded,
                    ticker
                );
                self.ui
                    .push_notification(Toast::new(Notification::Info(format!(
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
                    data::lock_or_recover(&self.persistence.data_index).add_contribution(
                        key,
                        Kairos::REGISTRY_SENTINEL_FEED,
                        dates,
                        false,
                    );
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
                    let feed = data::Connection::new_historical_databento(name, info);
                    let _feed_id = feed.id;

                    let dm_arc = self.connections.connection_manager.clone();
                    let mut feed_manager = data::lock_or_recover(&dm_arc);
                    feed_manager.add(feed);
                    self.sync_feed_snapshots(&feed_manager);
                    drop(feed_manager);

                    self.modals.historical_download_modal = None;
                    self.modals.historical_download_id = None;

                    return self.collect_and_persist_state();
                }
            }
            Err(e) => {
                log::error!("Historical download failed: {}", e);
                self.ui
                    .push_notification(Toast::error(format!("Download failed: {}", e)));
                if let Some(modal) = &mut self.modals.historical_download_modal {
                    modal
                        .set_download_progress(crate::modals::download::DownloadProgress::Error(e));
                }
            }
        }
        Task::none()
    }
}
