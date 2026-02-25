use iced::Task;

use crate::app::{ChartMessage, Kairos, Message};
use crate::app::core::globals;
use crate::app::init::services;
use crate::components::display::toast::{Notification, Toast};
use crate::screen::dashboard;

impl Kairos {
    pub(crate) fn connect_rithmic_feed(
        &mut self,
        feed_id: data::FeedId,
        mut feed_manager: std::sync::MutexGuard<'_, data::DataFeedManager>,
    ) -> Task<Message> {
        let Some(password) = self.secrets.get_feed_password(&feed_id.to_string()) else {
            self.modals.data_feeds_modal.sync_snapshot(&feed_manager);
            self.ui.notifications.push(Toast::error(
                "Rithmic password not configured. Set it in Manage Connections."
                    .to_string(),
            ));
            return Task::none();
        };

        let Some(feed) = feed_manager.get(feed_id) else {
            log::warn!("Rithmic feed {} not found in feed manager", feed_id);
            return Task::none();
        };
        let Some(rithmic_config) = feed.rithmic_config().cloned() else {
            log::warn!("Feed {} is not a Rithmic feed", feed_id);
            return Task::none();
        };
        feed_manager.set_status(feed_id, data::FeedStatus::Connecting);
        self.modals.data_feeds_modal.sync_snapshot(&feed_manager);
        drop(feed_manager);

        // Run on blocking thread since rithmic_rs futures are not Send
        Task::perform(
            rithmic_connect_task(rithmic_config, password),
            move |result| Message::RithmicConnected { feed_id, result },
        )
    }

    pub(super) fn disconnect_rithmic_feed(
        &mut self,
        feed_id: data::FeedId,
        mut feed_manager: std::sync::MutexGuard<'_, data::DataFeedManager>,
    ) -> Task<Message> {
        // Remove this feed's contributions from the shared DataIndex
        data::lock_or_recover(&self.persistence.data_index).remove_feed(feed_id);
        self.rebuild_ticker_data();

        let client = self.connections.rithmic_client.take();
        self.connections.rithmic_trade_repo = None;
        self.connections.rithmic_depth_repo = None;
        self.connections.rithmic_feed_id = None;

        // Unaffiliate panes so they show "Disconnected" status
        let main_window = self.main_window.id;
        for layout in &mut self.persistence.layout_manager.layouts {
            layout
                .dashboard
                .unaffiliate_panes_for_feed(feed_id, main_window);
        }

        if let Some(client) = client {
            feed_manager.set_status(feed_id, data::FeedStatus::Disconnected);
            self.sync_feed_snapshots(&feed_manager);
            drop(feed_manager);

            return Task::perform(
                async move {
                    client.lock().await.disconnect().await;
                },
                move |_| {
                    Message::DataFeeds(
                        crate::modals::data_feeds::DataFeedsMessage::FeedStatusChanged(
                            feed_id,
                            data::FeedStatus::Disconnected,
                        ),
                    )
                },
            );
        }

        feed_manager.set_status(feed_id, data::FeedStatus::Disconnected);
        self.sync_feed_snapshots(&feed_manager);
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

        let mut feed_manager =
            data::lock_or_recover(&self.connections.data_feed_manager);
        feed_manager.set_status(feed_id, data::FeedStatus::Error(e.clone()));
        self.modals.data_feeds_modal.sync_snapshot(&feed_manager);
        self.ui.notifications
            .push(Toast::error(format!("Rithmic connection failed: {}", e)));
        Task::none()
    }

    fn apply_rithmic_connected(&mut self, feed_id: data::FeedId) -> Task<Message> {
        // Take service result from global staging
        let service_result = {
            let mut staging =
                data::lock_or_recover(globals::get_rithmic_service_staging());
            staging.take()
        };

        let Some(sr) = service_result else {
            log::error!("Rithmic service result not found in staging");
            self.ui.notifications.push(Toast::error(
                "Internal error: Rithmic service result lost".to_string(),
            ));
            return Task::none();
        };

        self.connections.rithmic_client = Some(sr.client.clone());
        self.connections.rithmic_trade_repo = Some(sr.trade_repo);
        self.connections.rithmic_depth_repo = Some(sr.depth_repo);
        self.connections.rithmic_feed_id = Some(feed_id);
        self.connections.rithmic_reconnect_attempts = 0;

        let mut feed_manager =
            data::lock_or_recover(&self.connections.data_feed_manager);
        feed_manager.set_status(feed_id, data::FeedStatus::Connected);

        let subscribed_tickers = feed_manager
            .get(feed_id)
            .and_then(|f| f.rithmic_config())
            .map(|cfg| cfg.subscribed_tickers.clone())
            .unwrap_or_default();

        self.modals.data_feeds_modal.sync_snapshot(&feed_manager);
        drop(feed_manager);

        // Merge Rithmic contribution into the shared DataIndex
        let rithmic_index =
            exchange::build_rithmic_contribution(feed_id, &subscribed_tickers);
        data::lock_or_recover(&self.persistence.data_index).merge(rithmic_index);

        // Re-affiliate disconnected panes and collect any that need reloading
        let main_window = self.main_window.id;
        let mut reload_tasks: Vec<Task<Message>> = Vec::new();
        for layout in &mut self.persistence.layout_manager.layouts {
            let lid = layout.id.unique;
            let reloads = layout
                .dashboard
                .affiliate_and_collect_reloads(feed_id, main_window);
            for (pane_id, config, ticker_info) in reloads {
                reload_tasks.push(Task::done(Message::Chart(
                    ChartMessage::LoadChartData {
                        layout_id: lid,
                        pane_id,
                        config,
                        ticker_info,
                    },
                )));
            }
        }

        self.ui.notifications.push(Toast::new(Notification::Info(
            "Rithmic connected".to_string(),
        )));

        let client = sr.client.clone();
        let rithmic_sender = globals::get_rithmic_sender();

        // Resolve FuturesTickerInfo for each subscribed ticker symbol
        let ticker_infos: Vec<exchange::FuturesTickerInfo> = subscribed_tickers
            .iter()
            .filter_map(|sym| {
                self.persistence.tickers_info.iter().find_map(|(ticker, info)| {
                    if ticker.as_str() == sym || ticker.product() == sym {
                        Some(*info)
                    } else {
                        None
                    }
                })
            })
            .collect();

        let streaming_task = Task::perform(
            rithmic_streaming_task(client, rithmic_sender, subscribed_tickers, ticker_infos),
            |_| Message::RithmicStreamEvent(exchange::Event::ConnectionLost),
        );

        if reload_tasks.is_empty() {
            streaming_task
        } else {
            log::info!(
                "Reloading {} pane(s) after Rithmic connect",
                reload_tasks.len()
            );
            reload_tasks.push(streaming_task);
            Task::batch(reload_tasks)
        }
    }

    pub(crate) fn handle_rithmic_stream_event(&mut self, event: exchange::Event) -> Task<Message> {
        match event {
            exchange::Event::TradeReceived(stream_kind, ref _trade) => {
                if self.connections.rithmic_feed_id.is_none() {
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
                if self.connections.rithmic_feed_id.is_none() {
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
            exchange::Event::SubscriptionFailed(tickers) => {
                let msg = format!(
                    "Failed to subscribe to: {}",
                    tickers.join(", ")
                );
                self.ui.notifications.push(Toast::error(msg));
                return Task::none();
            }
            exchange::Event::ProductCodesReceived(codes) => {
                let feed_id = self
                    .connections
                    .rithmic_feed_id
                    .unwrap_or(data::FeedId::nil());
                return Task::done(Message::RithmicProductCodes {
                    feed_id,
                    result: Ok(codes),
                });
            }
            _ => {}
        }
        Task::none()
    }

    fn handle_rithmic_connection_lost(&mut self) -> Task<Message> {
        let Some(feed_id) = self.connections.rithmic_feed_id else {
            return Task::none();
        };

        // Clear stale client/repos so the old streaming task can't
        // push events tagged with outdated state.
        self.connections.rithmic_client = None;
        self.connections.rithmic_trade_repo = None;
        self.connections.rithmic_depth_repo = None;

        // Unaffiliate panes so they show stale/disconnected status
        let main_window = self.main_window.id;
        for layout in &mut self.persistence.layout_manager.layouts {
            layout
                .dashboard
                .unaffiliate_panes_for_feed(feed_id, main_window);
        }

        let mut feed_manager =
            data::lock_or_recover(&self.connections.data_feed_manager);
        let auto_reconnect = feed_manager
            .get(feed_id)
            .and_then(|f| f.rithmic_config())
            .map(|c| c.auto_reconnect)
            .unwrap_or(false);

        if auto_reconnect {
            self.connections.rithmic_reconnect_attempts += 1;
            let attempts = self.connections.rithmic_reconnect_attempts;

            // Exponential backoff: 1s, 5s, 15s, 30s (capped)
            let delay_secs = match attempts {
                1 => 1,
                2 => 5,
                3 => 15,
                _ => 30,
            };

            feed_manager.set_status(feed_id, data::FeedStatus::Connecting);
            self.modals.data_feeds_modal.sync_snapshot(&feed_manager);
            drop(feed_manager);
            self.ui.notifications.push(Toast::new(Notification::Info(
                format!(
                    "Rithmic reconnecting in {}s (attempt {})...",
                    delay_secs, attempts
                ),
            )));

            let delay = std::time::Duration::from_secs(delay_secs);
            return Task::perform(
                async move { tokio::time::sleep(delay).await },
                move |_| {
                    Message::DataFeeds(
                        crate::modals::data_feeds::DataFeedsMessage::ConnectFeed(
                            feed_id,
                        ),
                    )
                },
            );
        }

        feed_manager.set_status(
            feed_id,
            data::FeedStatus::Error("Connection lost".to_string()),
        );
        self.modals.data_feeds_modal.sync_snapshot(&feed_manager);
        self.ui.notifications
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
            let mut staging =
                data::lock_or_recover(globals::get_rithmic_service_staging());
            *staging = Some(service_result);
            Ok(())
        }
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("Connection timed out after 30 seconds".to_string()),
    }
}

async fn rithmic_streaming_task(
    client: std::sync::Arc<tokio::sync::Mutex<exchange::RithmicClient>>,
    sender: &'static tokio::sync::mpsc::UnboundedSender<exchange::Event>,
    subscribed_tickers: Vec<String>,
    ticker_infos: Vec<exchange::FuturesTickerInfo>,
) {
    // Subscribe to configured tickers, collect failures
    let mut sub_failures: Vec<String> = Vec::new();
    {
        let mut guard = client.lock().await;
        for ticker in &subscribed_tickers {
            if let Err(e) = guard.subscribe(ticker, "CME").await {
                log::warn!("Failed to subscribe to {}: {}", ticker, e);
                sub_failures.push(ticker.clone());
            }
        }
    }

    // Fetch available product codes from CME and send them to the UI
    {
        let guard = client.lock().await;
        match guard.get_product_codes(Some("CME")).await {
            Ok(codes) if !codes.is_empty() => {
                let _ = sender.send(
                    exchange::Event::ProductCodesReceived(codes),
                );
            }
            Ok(_) => {}
            Err(e) => {
                log::warn!("Failed to fetch product codes: {}", e);
            }
        }
    }

    // Notify about subscription failures via the channel
    if !sub_failures.is_empty() {
        log::warn!(
            "Rithmic subscription failures: {:?}",
            sub_failures
        );
        let _ = sender.send(exchange::Event::SubscriptionFailed(sub_failures));
    }

    // Take ticker handle and start streaming
    let handle = {
        let mut guard = client.lock().await;
        guard.take_ticker_handle()
    };

    let Some(handle) = handle else {
        return;
    };

    // Build a symbol → FuturesTickerInfo map so each streaming
    // event is tagged with the correct contract metadata (C1 fix).
    let ticker_map: rustc_hash::FxHashMap<String, exchange::FuturesTickerInfo> =
        subscribed_tickers
            .iter()
            .zip(ticker_infos.iter())
            .map(|(sym, info)| (sym.clone(), *info))
            .collect();

    if ticker_map.is_empty() {
        log::warn!(
            "No ticker info resolved for Rithmic subscription {:?}; \
             streaming will skip events without symbol match",
            subscribed_tickers
        );
    }

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    let stream = exchange::RithmicStream::new(handle);

    // Spawn stream reader
    tokio::spawn(async move {
        stream.run(ticker_map, event_tx).await;
    });

    // Forward events directly to the global channel sender
    while let Some(event) = event_rx.recv().await {
        let _ = sender.send(event);
    }
}
