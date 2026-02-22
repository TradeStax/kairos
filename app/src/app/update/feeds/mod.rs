mod databento;
mod lifecycle;
mod rithmic;

use iced::Task;

use super::super::{Kairos, Message};

impl Kairos {
    pub(crate) fn handle_data_feeds(
        &mut self,
        msg: crate::modals::data_feeds::DataFeedsMessage,
    ) -> Task<Message> {
        let mut feed_manager = self
            .data_feed_manager
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        let action = self.data_feeds_modal.update(msg, &mut feed_manager);

        if let Some(action) = action {
            // Drop feed_manager before calling self methods to avoid borrow conflicts
            drop(feed_manager);

            match action {
                crate::modals::data_feeds::Action::ConnectFeed(feed_id) => {
                    let mgr = self.data_feed_manager.clone();
                    let feed_manager = mgr.lock().unwrap_or_else(|e| e.into_inner());
                    return self.connect_feed(feed_id, feed_manager);
                }
                crate::modals::data_feeds::Action::DisconnectFeed(feed_id) => {
                    let mgr = self.data_feed_manager.clone();
                    let feed_manager = mgr.lock().unwrap_or_else(|e| e.into_inner());
                    return self.disconnect_feed(feed_id, feed_manager);
                }
                crate::modals::data_feeds::Action::FeedsUpdated => {
                    let mgr = self.data_feed_manager.clone();
                    let feed_manager = mgr.lock().unwrap_or_else(|e| e.into_inner());
                    return self.handle_feeds_updated(feed_manager);
                }
                crate::modals::data_feeds::Action::OpenHistoricalDownload => {
                    let has_key = crate::infra::secrets::SecretsManager::new()
                        .has_api_key(data::config::secrets::ApiProvider::Databento);
                    if has_key {
                        self.historical_download_modal =
                            Some(crate::modals::download::HistoricalDownloadModal::new());
                    } else {
                        self.api_key_setup_modal =
                            Some(crate::modals::download::ApiKeySetupModal::new());
                    }
                    return Task::none();
                }
                crate::modals::data_feeds::Action::LoadPreview(feed_id, info) => {
                    return self.load_feed_preview(feed_id, info);
                }
                crate::modals::data_feeds::Action::SaveApiKey { provider, key } => {
                    let secrets = crate::infra::secrets::SecretsManager::new();
                    if let Err(e) = secrets.set_api_key(provider, &key) {
                        log::warn!("Failed to save API key for {:?}: {}", provider, e);
                    }
                    let mgr = self.data_feed_manager.clone();
                    let feed_manager = mgr.lock().unwrap_or_else(|e| e.into_inner());
                    return self.handle_feeds_updated(feed_manager);
                }
            }
        }

        // Sync snapshot after any update
        self.data_feeds_modal.sync_snapshot(&feed_manager);
        Task::none()
    }

    fn connect_feed(
        &mut self,
        feed_id: data::FeedId,
        feed_manager: std::sync::MutexGuard<'_, data::DataFeedManager>,
    ) -> Task<Message> {
        log::info!("Connect feed requested: {}", feed_id);

        let Some(feed) = feed_manager.get(feed_id) else {
            return Task::none();
        };

        match feed.provider {
            data::FeedProvider::Databento => self.connect_databento_feed(feed_id, feed_manager),
            data::FeedProvider::Rithmic => self.connect_rithmic_feed(feed_id, feed_manager),
        }
    }

    fn disconnect_feed(
        &mut self,
        feed_id: data::FeedId,
        mut feed_manager: std::sync::MutexGuard<'_, data::DataFeedManager>,
    ) -> Task<Message> {
        log::info!("Disconnect feed requested: {}", feed_id);

        let provider = feed_manager.get(feed_id).map(|f| f.provider);

        // Check if this is the active Rithmic feed
        if self.rithmic_feed_id == Some(feed_id) {
            return self.disconnect_rithmic_feed(feed_id, feed_manager);
        }

        feed_manager.set_status(feed_id, data::FeedStatus::Disconnected);

        if provider == Some(data::FeedProvider::Databento) {
            self.disconnect_databento_feed(feed_id, feed_manager)
        } else {
            self.connections_menu.sync_snapshot(&feed_manager);
            self.data_feeds_modal.sync_snapshot(&feed_manager);
            Task::none()
        }
    }

    fn load_feed_preview(
        &mut self,
        feed_id: data::FeedId,
        info: data::HistoricalDatasetInfo,
    ) -> Task<Message> {
        if let Some(service) = self.market_data_service.clone() {
            let ticker_str = info.ticker.clone();
            let date_range = info.date_range;
            let _schema = info.schema.clone();
            return Task::perform(
                async move {
                    let ticker =
                        data::FuturesTicker::new(&ticker_str, data::FuturesVenue::CMEGlobex);
                    let trades = service
                        .get_trades_for_preview(&ticker, &date_range)
                        .await
                        .map_err(|e| e.to_string())?;

                    let total_trades = trades.len();

                    // Sample price line (every Nth trade)
                    let step = (total_trades / 200).max(1);
                    let price_line: Vec<(u64, f64)> = trades
                        .iter()
                        .step_by(step)
                        .map(|t| (t.time.0, t.price.to_f64()))
                        .collect();

                    // First 100 trades for the table
                    let trade_rows: Vec<crate::modals::data_feeds::TradePreviewRow> = trades
                        .iter()
                        .take(100)
                        .map(|t| {
                            let dt =
                                chrono::DateTime::from_timestamp_millis(t.time.0 as i64);
                            let time_str = dt
                                .map(|d| d.format("%H:%M:%S%.3f").to_string())
                                .unwrap_or_default();
                            crate::modals::data_feeds::TradePreviewRow {
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

                    Ok(crate::modals::data_feeds::PreviewData {
                        feed_id,
                        price_line,
                        trades: trade_rows,
                        total_trades,
                    })
                },
                move |result| Message::DataFeedPreviewLoaded { feed_id, result },
            );
        }
        Task::none()
    }

    pub(crate) fn handle_data_feed_preview_loaded(
        &mut self,
        feed_id: data::FeedId,
        result: Result<crate::modals::data_feeds::PreviewData, String>,
    ) {
        self.data_feeds_modal.update(
            crate::modals::data_feeds::DataFeedsMessage::PreviewLoaded(feed_id, result),
            &mut self
                .data_feed_manager
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
        );
    }

    pub(crate) fn handle_connections_menu(
        &mut self,
        msg: crate::modals::connections::ConnectionsMenuMessage,
    ) -> Task<Message> {
        if let Some(action) = self.connections_menu.update(msg) {
            match action {
                crate::modals::connections::Action::ConnectFeed(feed_id) => {
                    self.sidebar.set_menu(None);
                    return Task::done(Message::DataFeeds(
                        crate::modals::data_feeds::DataFeedsMessage::ConnectFeed(feed_id),
                    ));
                }
                crate::modals::connections::Action::DisconnectFeed(feed_id) => {
                    self.sidebar.set_menu(None);
                    return Task::done(Message::DataFeeds(
                        crate::modals::data_feeds::DataFeedsMessage::DisconnectFeed(feed_id),
                    ));
                }
                crate::modals::connections::Action::OpenManageDialog => {
                    self.sidebar.set_menu(Some(data::sidebar::Menu::DataFeeds));
                    let feed_manager = self
                        .data_feed_manager
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    self.data_feeds_modal.sync_snapshot(&feed_manager);
                }
            }
        }
        Task::none()
    }
}
