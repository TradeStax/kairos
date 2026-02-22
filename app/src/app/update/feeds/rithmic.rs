use iced::Task;

use crate::components::display::toast::{Notification, Toast};
use crate::screen::dashboard;
use super::super::super::{Kairos, Message, services};

impl Kairos {
    pub(super) fn connect_rithmic_feed(
        &mut self,
        feed_id: data::FeedId,
        mut feed_manager: std::sync::MutexGuard<'_, data::DataFeedManager>,
    ) -> Task<Message> {
        let secrets = crate::infra::secrets::SecretsManager::new();
        let password_status = secrets.get_api_key(data::config::secrets::ApiProvider::Rithmic);

        let Some(password) = password_status.key() else {
            self.data_feeds_modal.sync_snapshot(&feed_manager);
            self.notifications.push(Toast::error(
                "Rithmic password not configured. Set it in connection \
                 settings."
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

    pub(super) fn disconnect_rithmic_feed(
        &mut self,
        feed_id: data::FeedId,
        mut feed_manager: std::sync::MutexGuard<'_, data::DataFeedManager>,
    ) -> Task<Message> {
        // Remove this feed's contributions from the shared DataIndex
        data::lock_or_recover(&self.data_index).remove_feed(feed_id);
        let tickers: std::collections::HashSet<String> =
            data::lock_or_recover(&self.data_index)
                .available_tickers()
                .into_iter()
                .collect();
        self.tickers_info = super::super::super::build_tickers_info(tickers);
        self.ticker_ranges = Kairos::build_ticker_ranges(&self.data_index);

        let client = self.rithmic_client.take();
        self.rithmic_trade_repo = None;
        self.rithmic_depth_repo = None;
        self.rithmic_feed_id = None;
        super::super::super::globals::set_rithmic_active(false);

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
                        crate::modals::data_feeds::DataFeedsMessage::FeedStatusChanged(
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
            let mut staging = super::super::super::globals::get_rithmic_service_staging()
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
        super::super::super::globals::set_rithmic_active(true);

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

        // Merge Rithmic contribution into the shared DataIndex
        let rithmic_index =
            exchange::build_rithmic_contribution(feed_id, &subscribed_tickers);
        data::lock_or_recover(&self.data_index).merge(rithmic_index);

        self.notifications.push(Toast::new(Notification::Info(
            "Rithmic connected".to_string(),
        )));

        let client = sr.client.clone();
        let events_buf = super::super::super::globals::get_rithmic_events().clone();

        // Resolve FuturesTickerInfo for each subscribed ticker symbol
        let ticker_infos: Vec<exchange::FuturesTickerInfo> = subscribed_tickers
            .iter()
            .filter_map(|sym| {
                self.tickers_info.iter().find_map(|(ticker, info)| {
                    if ticker.as_str() == sym || ticker.product() == sym {
                        Some(*info)
                    } else {
                        None
                    }
                })
            })
            .collect();

        Task::perform(
            rithmic_streaming_task(client, events_buf, subscribed_tickers, ticker_infos),
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
                super::super::super::globals::set_rithmic_active(false);
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
                crate::modals::data_feeds::DataFeedsMessage::ConnectFeed(feed_id),
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
            let mut staging = super::super::super::globals::get_rithmic_service_staging()
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            *staging = Some(service_result);
            Ok(())
        }
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("Connection timed out after 30 seconds".to_string()),
    }
}

async fn rithmic_streaming_task(
    client: std::sync::Arc<tokio::sync::Mutex<exchange::RithmicClient>>,
    events_buf: std::sync::Arc<std::sync::Mutex<Vec<exchange::Event>>>,
    subscribed_tickers: Vec<String>,
    ticker_infos: Vec<exchange::FuturesTickerInfo>,
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

    // Derive stream_kind from the first resolved ticker info; fall back to ES defaults
    // if no ticker info could be resolved (e.g. on first connection before data is loaded)
    let stream_kind = ticker_infos
        .into_iter()
        .next()
        .map(|info| exchange::adapter::StreamKind::DepthAndTrades {
            ticker_info: info,
            depth_aggr: exchange::adapter::StreamTicksize::Client,
            push_freq: exchange::PushFrequency::ServerDefault,
        })
        .unwrap_or_else(|| {
            log::warn!(
                "No ticker info resolved for Rithmic subscription {:?}; \
                 using default ES parameters",
                subscribed_tickers
            );
            let default_ticker =
                exchange::FuturesTicker::new("ES", exchange::FuturesVenue::CMEGlobex);
            exchange::adapter::StreamKind::DepthAndTrades {
                ticker_info: exchange::FuturesTickerInfo::new(default_ticker, 0.25, 1.0, 50.0),
                depth_aggr: exchange::adapter::StreamTicksize::Client,
                push_freq: exchange::PushFrequency::ServerDefault,
            }
        });

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    let stream = exchange::RithmicStream::new(handle);

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
