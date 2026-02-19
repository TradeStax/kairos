use iced::Task;

use crate::component::display::toast::{Notification, Toast};
use crate::screen::dashboard;

use super::super::{ChartMessage, DownloadMessage, Flowsurface, Message, services};

impl Flowsurface {
    pub(crate) fn handle_data_feeds(
        &mut self,
        msg: crate::modal::pane::data_feeds::DataFeedsMessage,
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
                crate::modal::pane::data_feeds::Action::ConnectFeed(feed_id) => {
                    let mgr = self.data_feed_manager.clone();
                    let feed_manager = mgr.lock().unwrap_or_else(|e| e.into_inner());
                    return self.connect_feed(feed_id, feed_manager);
                }
                crate::modal::pane::data_feeds::Action::DisconnectFeed(feed_id) => {
                    let mgr = self.data_feed_manager.clone();
                    let feed_manager = mgr.lock().unwrap_or_else(|e| e.into_inner());
                    return self.disconnect_feed(feed_id, feed_manager);
                }
                crate::modal::pane::data_feeds::Action::FeedsUpdated => {
                    let mgr = self.data_feed_manager.clone();
                    let feed_manager = mgr.lock().unwrap_or_else(|e| e.into_inner());
                    return self.handle_feeds_updated(feed_manager);
                }
                crate::modal::pane::data_feeds::Action::OpenHistoricalDownload => {
                    self.historical_download_modal =
                        Some(crate::modal::pane::download::HistoricalDownloadModal::new());
                    return Task::none();
                }
                crate::modal::pane::data_feeds::Action::LoadPreview(feed_id, info) => {
                    return self.load_feed_preview(feed_id, info);
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

    fn connect_databento_feed(
        &mut self,
        feed_id: data::FeedId,
        mut feed_manager: std::sync::MutexGuard<'_, data::DataFeedManager>,
    ) -> Task<Message> {
        let secrets = data::SecretsManager::new();
        if !secrets.has_api_key(data::ApiProvider::Databento) {
            self.data_feeds_modal.sync_snapshot(&feed_manager);
            self.notifications.push(Toast::error(
                "Databento API key not configured. Set it in connection \
                 settings."
                    .to_string(),
            ));
            return Task::none();
        }

        feed_manager.set_status(feed_id, data::FeedStatus::Connected);
        self.connections_menu.sync_snapshot(&feed_manager);
        self.data_feeds_modal.sync_snapshot(&feed_manager);
        drop(feed_manager);

        // Re-populate tickers table from downloaded tickers registry
        let ticker_symbols: std::collections::HashSet<String> = self
            .downloaded_tickers
            .lock()
            .unwrap()
            .list_tickers()
            .into_iter()
            .collect();
        self.tickers_info = super::super::build_tickers_info(ticker_symbols);
        log::info!("Databento feed connected - restored ticker list");

        // Re-affiliate disconnected panes and reload any in error state
        let main_window = self.main_window.id;
        let Some(lid) = self.layout_manager.active_layout_id().map(|id| id.unique) else {
            return Task::none();
        };

        let reload_tasks: Vec<_> = self
            .layout_manager
            .layouts
            .iter_mut()
            .flat_map(|layout| {
                layout
                    .dashboard
                    .affiliate_and_collect_reloads(feed_id, main_window)
            })
            .map(|(pane_id, config, ticker_info)| {
                Task::done(Message::Chart(ChartMessage::LoadChartData {
                    layout_id: lid,
                    pane_id,
                    config,
                    ticker_info,
                }))
            })
            .collect();

        if !reload_tasks.is_empty() {
            log::info!("Reloading {} pane(s) after reconnect", reload_tasks.len());
            return Task::batch(reload_tasks);
        }
        Task::none()
    }

    fn connect_rithmic_feed(
        &mut self,
        feed_id: data::FeedId,
        mut feed_manager: std::sync::MutexGuard<'_, data::DataFeedManager>,
    ) -> Task<Message> {
        let secrets = data::SecretsManager::new();
        let password_status = secrets.get_api_key(data::ApiProvider::Rithmic);

        let Some(password) = password_status.key() else {
            self.data_feeds_modal.sync_snapshot(&feed_manager);
            self.notifications.push(Toast::error(
                "Rithmic password not configured. Set it in connection \
                 settings."
                    .to_string(),
            ));
            return Task::none();
        };

        let feed = feed_manager.get(feed_id).unwrap();
        let rithmic_config = match &feed.config {
            data::feed::FeedConfig::Rithmic(cfg) => cfg.clone(),
            _ => unreachable!(),
        };
        let password = password.to_string();

        feed_manager.set_status(feed_id, data::FeedStatus::Connecting);
        self.data_feeds_modal.sync_snapshot(&feed_manager);
        drop(feed_manager);

        // Run on blocking thread since rithmic_rs futures are not Send
        Task::perform(
            rithmic_connect_task(rithmic_config, password),
            move |result| Message::RithmicConnected { feed_id, result },
        )
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

    fn disconnect_rithmic_feed(
        &mut self,
        feed_id: data::FeedId,
        mut feed_manager: std::sync::MutexGuard<'_, data::DataFeedManager>,
    ) -> Task<Message> {
        let client = self.rithmic_client.take();
        self.rithmic_trade_repo = None;
        self.rithmic_depth_repo = None;
        self.rithmic_feed_id = None;

        if let Some(client) = client {
            feed_manager.set_status(feed_id, data::FeedStatus::Disconnected);
            self.data_feeds_modal.sync_snapshot(&feed_manager);
            self.connections_menu.sync_snapshot(&feed_manager);
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

        feed_manager.set_status(feed_id, data::FeedStatus::Disconnected);
        self.connections_menu.sync_snapshot(&feed_manager);
        self.data_feeds_modal.sync_snapshot(&feed_manager);
        Task::none()
    }

    fn disconnect_databento_feed(
        &mut self,
        feed_id: data::FeedId,
        feed_manager: std::sync::MutexGuard<'_, data::DataFeedManager>,
    ) -> Task<Message> {
        // Check if another Databento feed is still connected
        let alt_feed_id =
            feed_manager.connected_feed_id_for_provider(data::FeedProvider::Databento);

        self.connections_menu.sync_snapshot(&feed_manager);
        self.data_feeds_modal.sync_snapshot(&feed_manager);
        drop(feed_manager);

        let main_window = self.main_window.id;
        if let Some(alt_fid) = alt_feed_id {
            // Another Databento feed is connected - silently re-affiliate
            for layout in &mut self.layout_manager.layouts {
                let reloads = layout
                    .dashboard
                    .affiliate_and_collect_reloads(alt_fid, main_window);
                if !reloads.is_empty() {
                    log::info!("Re-affiliated panes to alt feed {}", alt_fid);
                }
            }
        } else {
            // No other feed connected - keep charts visible but
            // mark panes as unaffiliated
            for layout in &mut self.layout_manager.layouts {
                layout
                    .dashboard
                    .unaffiliate_panes_for_feed(feed_id, main_window);
            }
        }
        Task::none()
    }

    fn handle_feeds_updated(
        &mut self,
        feed_manager: std::sync::MutexGuard<'_, data::DataFeedManager>,
    ) -> Task<Message> {
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
        Task::none()
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
                    let trade_rows: Vec<crate::modal::pane::data_feeds::TradePreviewRow> = trades
                        .iter()
                        .take(100)
                        .map(|t| {
                            let dt = chrono::DateTime::from_timestamp_millis(t.time.0 as i64);
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

                    Ok(crate::modal::pane::data_feeds::PreviewData {
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
        result: Result<crate::modal::pane::data_feeds::PreviewData, String>,
    ) {
        self.data_feeds_modal.update(
            crate::modal::pane::data_feeds::DataFeedsMessage::PreviewLoaded(feed_id, result),
            &mut self
                .data_feed_manager
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
        );
    }

    pub(crate) fn handle_connections_menu(
        &mut self,
        msg: crate::modal::pane::connections::ConnectionsMenuMessage,
    ) -> Task<Message> {
        if let Some(action) = self.connections_menu.update(msg) {
            match action {
                crate::modal::pane::connections::Action::ConnectFeed(feed_id) => {
                    self.sidebar.set_menu(None);
                    return Task::done(Message::DataFeeds(
                        crate::modal::pane::data_feeds::DataFeedsMessage::ConnectFeed(feed_id),
                    ));
                }
                crate::modal::pane::connections::Action::DisconnectFeed(feed_id) => {
                    self.sidebar.set_menu(None);
                    return Task::done(Message::DataFeeds(
                        crate::modal::pane::data_feeds::DataFeedsMessage::DisconnectFeed(feed_id),
                    ));
                }
                crate::modal::pane::connections::Action::OpenManageDialog => {
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

    pub(crate) fn handle_rithmic_connected(
        &mut self,
        feed_id: data::FeedId,
        result: Result<(), String>,
    ) -> Task<Message> {
        let Err(e) = result.as_ref() else {
            return self.apply_rithmic_connected(feed_id);
        };

        let mut feed_manager = self
            .data_feed_manager
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        feed_manager.set_status(feed_id, data::FeedStatus::Error(e.clone()));
        self.data_feeds_modal.sync_snapshot(&feed_manager);
        self.notifications
            .push(Toast::error(format!("Rithmic connection failed: {}", e)));
        Task::none()
    }

    fn apply_rithmic_connected(&mut self, feed_id: data::FeedId) -> Task<Message> {
        // Take service result from global staging
        let service_result = {
            let mut staging = super::super::get_rithmic_service_staging()
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            staging.take()
        };

        let Some(sr) = service_result else {
            log::error!("Rithmic service result not found in staging");
            self.notifications.push(Toast::error(
                "Internal error: Rithmic service result lost".to_string(),
            ));
            return Task::none();
        };

        self.rithmic_client = Some(sr.client.clone());
        self.rithmic_trade_repo = Some(sr.trade_repo);
        self.rithmic_depth_repo = Some(sr.depth_repo);
        self.rithmic_feed_id = Some(feed_id);

        let mut feed_manager = self
            .data_feed_manager
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        feed_manager.set_status(feed_id, data::FeedStatus::Connected);

        let subscribed_tickers = feed_manager
            .get(feed_id)
            .and_then(|f| f.rithmic_config())
            .map(|cfg| cfg.subscribed_tickers.clone())
            .unwrap_or_default();

        self.data_feeds_modal.sync_snapshot(&feed_manager);
        drop(feed_manager);

        self.notifications.push(Toast::new(Notification::Info(
            "Rithmic connected".to_string(),
        )));

        let client = sr.client.clone();
        let events_buf = super::super::get_rithmic_events().clone();

        Task::perform(
            rithmic_streaming_task(client, events_buf, subscribed_tickers),
            |_| Message::RithmicStreamEvent(exchange::Event::ConnectionLost),
        )
    }

    pub(crate) fn handle_rithmic_stream_event(&mut self, event: exchange::Event) -> Task<Message> {
        match event {
            exchange::Event::TradeReceived(stream_kind, ref _trade) => {
                if self.rithmic_feed_id.is_none() {
                    return Task::none();
                }
                return Task::done(Message::Dashboard {
                    layout_id: None,
                    event: dashboard::Message::ExchangeEvent(exchange::Event::TradeReceived(
                        stream_kind,
                        *_trade,
                    )),
                });
            }
            exchange::Event::DepthReceived(stream_kind, ts, ref depth, ref trades) => {
                if self.rithmic_feed_id.is_none() {
                    return Task::none();
                }
                return Task::done(Message::Dashboard {
                    layout_id: None,
                    event: dashboard::Message::ExchangeEvent(exchange::Event::DepthReceived(
                        stream_kind,
                        ts,
                        depth.clone(),
                        trades.clone(),
                    )),
                });
            }
            exchange::Event::ConnectionLost => {
                return self.handle_rithmic_connection_lost();
            }
            _ => {}
        }
        Task::none()
    }

    fn handle_rithmic_connection_lost(&mut self) -> Task<Message> {
        let Some(feed_id) = self.rithmic_feed_id else {
            return Task::none();
        };

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
            feed_manager.set_status(feed_id, data::FeedStatus::Connecting);
            self.data_feeds_modal.sync_snapshot(&feed_manager);
            drop(feed_manager);
            self.notifications.push(Toast::new(Notification::Info(
                "Rithmic reconnecting...".to_string(),
            )));
            return Task::done(Message::DataFeeds(
                crate::modal::pane::data_feeds::DataFeedsMessage::ConnectFeed(feed_id),
            ));
        }

        feed_manager.set_status(
            feed_id,
            data::FeedStatus::Error("Connection lost".to_string()),
        );
        self.data_feeds_modal.sync_snapshot(&feed_manager);
        self.notifications
            .push(Toast::error("Rithmic connection lost".to_string()));
        Task::none()
    }
}

async fn rithmic_connect_task(
    rithmic_config: data::feed::RithmicFeedConfig,
    password: String,
) -> Result<(), String> {
    let handle = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || {
        handle.block_on(rithmic_init_and_stage(&rithmic_config, &password))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

async fn rithmic_init_and_stage(
    rithmic_config: &data::feed::RithmicFeedConfig,
    password: &str,
) -> Result<(), String> {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        services::initialize_rithmic_service(rithmic_config, password),
    )
    .await;

    match result {
        Ok(Ok(service_result)) => {
            let mut staging = super::super::get_rithmic_service_staging()
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            *staging = Some(service_result);
            Ok(())
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Err("Connection timed out after 30 seconds".to_string()),
    }
}

async fn rithmic_streaming_task(
    client: std::sync::Arc<tokio::sync::Mutex<exchange::RithmicClient>>,
    events_buf: std::sync::Arc<std::sync::Mutex<Vec<exchange::Event>>>,
    subscribed_tickers: Vec<String>,
) {
    // Subscribe to configured tickers
    {
        let mut guard = client.lock().await;
        for ticker in &subscribed_tickers {
            if let Err(e) = guard.subscribe(ticker, "CME").await {
                log::warn!("Failed to subscribe to {}: {}", ticker, e);
            }
        }
    }

    // Take ticker handle and start streaming
    let handle = {
        let mut guard = client.lock().await;
        guard.take_ticker_handle()
    };

    let Some(handle) = handle else {
        return;
    };

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    let stream = exchange::RithmicStream::new(handle);
    let default_ticker = exchange::FuturesTicker::new("ES", exchange::FuturesVenue::CMEGlobex);
    let stream_kind = exchange::adapter::StreamKind::DepthAndTrades {
        ticker_info: exchange::FuturesTickerInfo::new(default_ticker, 0.25, 1.0, 50.0),
        depth_aggr: exchange::adapter::StreamTicksize::Client,
        push_freq: exchange::PushFrequency::ServerDefault,
    };

    // Spawn stream reader
    let buf = events_buf.clone();
    tokio::spawn(async move {
        stream.run(stream_kind, event_tx).await;
    });

    // Read events from channel
    while let Some(event) = event_rx.recv().await {
        if let Ok(mut buf) = buf.lock() {
            buf.push(event);
        }
    }
}
