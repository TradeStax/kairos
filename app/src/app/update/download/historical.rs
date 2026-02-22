use iced::Task;

use crate::components::display::toast::{Notification, Toast};
use super::super::super::{DownloadMessage, Kairos, Message};
use super::super::super::globals::get_download_progress;

impl Kairos {
    pub(crate) fn handle_api_key_setup(
        &mut self,
        msg: crate::modals::download::ApiKeySetupMessage,
    ) -> Task<Message> {
        if let Some(modal) = &mut self.api_key_setup_modal
            && let Some(action) = modal.update(msg)
        {
            match action {
                crate::modals::download::api_key_modal::Action::Saved {
                    provider,
                    key,
                } => {
                    let secrets = crate::infra::secrets::SecretsManager::new();
                    if let Err(e) = secrets.set_api_key(provider, &key) {
                        log::warn!("Failed to save API key: {}", e);
                        self.notifications
                            .push(Toast::error(format!("Failed to save API key: {}", e)));
                        return Task::none();
                    }
                    log::info!("API key saved for {:?}", provider);
                    self.api_key_setup_modal = None;
                    self.historical_download_modal =
                        Some(crate::modals::download::HistoricalDownloadModal::new());
                    return Task::done(Message::ReinitializeService(provider));
                }
                crate::modals::download::api_key_modal::Action::Closed => {
                    self.api_key_setup_modal = None;
                }
            }
        }
        Task::none()
    }

    pub(crate) fn handle_historical_download(
        &mut self,
        msg: crate::modals::download::HistoricalDownloadMessage,
    ) -> Task<Message> {
        if let Some(modal) = &mut self.historical_download_modal
            && let Some(action) = modal.update(msg)
        {
            match action {
                crate::modals::download::historical::Action::EstimateRequested {
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
                    let Some(service) = self.market_data_service.clone() else {
                        self.notifications
                            .push(Toast::error("Databento API key required".to_string()));
                        return Task::none();
                    };
                    let schema_discriminant = schema.as_discriminant();
                    let download_id = uuid::Uuid::new_v4();
                    self.historical_download_id = Some(download_id);
                    {
                        let mut progress = data::lock_or_recover(get_download_progress());
                        progress.insert(download_id, (0, date_range.num_days() as usize));
                    }
                    super::super::super::globals::set_download_active(true);
                    let ticker_clone = ticker;
                    let date_range_clone = date_range;
                    return Task::perform(
                        async move {
                            service
                                .download_to_cache_with_progress(
                                    &ticker,
                                    schema_discriminant,
                                    &date_range,
                                    Box::new(move |current, total| {
                                        if let Ok(mut progress) = get_download_progress().lock() {
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
                crate::modals::download::historical::Action::Closed => {
                    self.historical_download_modal = None;
                    self.historical_download_id = None;
                }
            }
        }
        Task::none()
    }

    pub(crate) fn handle_historical_download_cost_estimated(
        &mut self,
        result: Result<data::DataRequestEstimate, String>,
    ) {
        if let Some(modal) = &mut self.historical_download_modal {
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
        super::super::super::globals::set_download_active(false);
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

                data::lock_or_recover(&self.downloaded_tickers)
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
                    data::lock_or_recover(&self.data_index)
                        .add_contribution(key, uuid::Uuid::nil(), dates, false);
                }

                let available: std::collections::HashSet<String> =
                    data::lock_or_recover(&self.data_index)
                        .available_tickers()
                        .into_iter()
                        .collect();
                self.tickers_info = super::super::super::build_tickers_info(available);
                self.ticker_ranges =
                    Kairos::build_ticker_ranges(&self.data_index);

                // Create the dataset feed
                if let Some(modal) = &self.historical_download_modal {
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
                    modal
                        .set_download_progress(
                            crate::modals::download::DownloadProgress::Error(e),
                        );
                }
            }
        }
    }
}
