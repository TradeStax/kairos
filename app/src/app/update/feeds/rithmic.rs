use iced::Task;

use crate::app::{ChartMessage, Kairos, Message};
use crate::components::display::toast::{Notification, Toast};

impl Kairos {
    pub(crate) fn connect_rithmic_feed(
        &mut self,
        feed_id: data::FeedId,
        mut feed_manager: std::sync::MutexGuard<'_, data::ConnectionManager>,
    ) -> Task<Message> {
        let Some(password) = self.secrets.get_feed_password(&feed_id.to_string()) else {
            self.modals.data_feeds_modal.sync_snapshot(&feed_manager);
            self.ui.push_notification(Toast::error(
                "Rithmic password not configured. Set it in Manage Connections.".to_string(),
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
        feed_manager.set_status(feed_id, data::ConnectionStatus::Connecting);
        self.modals.data_feeds_modal.sync_snapshot(&feed_manager);
        drop(feed_manager);

        let Some(engine) = self.require_data_engine() else {
            return Task::none();
        };

        // Run on blocking thread since rithmic_rs futures are not Send
        Task::perform(
            rithmic_connect_task(engine, rithmic_config, password),
            move |result| Message::RithmicConnected { feed_id, result },
        )
    }

    pub(super) fn disconnect_rithmic_feed(
        &mut self,
        feed_id: data::FeedId,
        mut feed_manager: std::sync::MutexGuard<'_, data::ConnectionManager>,
    ) -> Task<Message> {
        // Remove this feed's contributions from the shared DataIndex
        data::lock_or_recover(&self.persistence.data_index).remove_feed(feed_id);
        self.rebuild_ticker_data();

        // Clear the Rithmic client
        self.services.rithmic_client = None;

        let client_was_some = self.services.rithmic_feed_id == Some(feed_id);

        let main_window = self.main_window.id;
        for layout in &mut self.persistence.layout_manager.layouts {
            layout
                .dashboard
                .unaffiliate_panes_for_feed(feed_id, main_window);
        }

        feed_manager.set_status(feed_id, data::ConnectionStatus::Disconnected);
        self.sync_feed_snapshots(&feed_manager);

        if client_was_some {
            self.services.rithmic_feed_id = None;
            // Disconnect via engine asynchronously
            let engine = self.services.engine.clone();
            return Task::perform(
                async move {
                    if let Some(engine) = engine {
                        let result =
                            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                                engine.lock().await.disconnect(feed_id).await
                            })
                            .await;
                        if result.is_err() {
                            log::warn!("Rithmic disconnect timed out after 5s");
                        }
                    }
                },
                move |_| {
                    Message::DataFeeds(
                        crate::modals::data_feeds::DataFeedsMessage::FeedStatusChanged(
                            feed_id,
                            data::ConnectionStatus::Disconnected,
                        ),
                    )
                },
            );
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

        let mut feed_manager = data::lock_or_recover(&self.connections.connection_manager);
        feed_manager.set_status(feed_id, data::ConnectionStatus::Error(e.clone()));
        self.modals.data_feeds_modal.sync_snapshot(&feed_manager);
        self.ui
            .push_notification(Toast::error(format!("Rithmic connection failed: {}", e)));
        Task::none()
    }

    fn apply_rithmic_connected(&mut self, feed_id: data::FeedId) -> Task<Message> {
        // Take service result from staging
        let service_result = {
            let mut staging =
                data::lock_or_recover(crate::app::core::globals::get_rithmic_client_staging());
            staging.take()
        };

        let Some(client) = service_result else {
            log::error!("Rithmic client not found in staging after connect");
            self.ui.push_notification(Toast::error(
                "Internal error: Rithmic client result lost".to_string(),
            ));
            return Task::none();
        };

        self.services.rithmic_client = Some(client.clone());
        self.services.rithmic_feed_id = Some(feed_id);
        self.services.rithmic_reconnect_attempts = 0;

        let mut feed_manager = data::lock_or_recover(&self.connections.connection_manager);
        feed_manager.set_status(feed_id, data::ConnectionStatus::Connected);

        let (subscribed_tickers, backfill_days) = feed_manager
            .get(feed_id)
            .and_then(|f| f.rithmic_config())
            .map(|cfg| (cfg.subscribed_tickers.clone(), cfg.backfill_days))
            .unwrap_or_default();

        self.modals.data_feeds_modal.sync_snapshot(&feed_manager);
        drop(feed_manager);

        // Build ticker_map and ticker_infos from FUTURES_PRODUCTS
        let mut ticker_map: rustc_hash::FxHashMap<String, data::FuturesTickerInfo> =
            rustc_hash::FxHashMap::default();
        let mut ticker_infos: Vec<data::FuturesTickerInfo> = Vec::new();

        for sym in &subscribed_tickers {
            if let Some(info) = crate::app::init::ticker_registry::FUTURES_PRODUCTS
                .iter()
                .find(|(full, ..)| full.split('.').next() == Some(sym.as_str()))
                .map(|(full, name, tick_size, min_qty, contract_size)| {
                    let ticker = data::FuturesTicker::new_with_display(
                        full,
                        data::FuturesVenue::CMEGlobex,
                        Some(sym.as_str()),
                        Some(name),
                    );
                    data::FuturesTickerInfo::new(ticker, *tick_size, *min_qty, *contract_size)
                })
            {
                ticker_map.insert(sym.clone(), info);
                ticker_infos.push(info);
            } else {
                log::warn!("No FUTURES_PRODUCTS entry for subscribed ticker '{}'", sym);
            }
        }

        // Build contribution with resolved ticker format (ES.c.0, not ES)
        let resolved_ticker_strings: Vec<String> = ticker_infos
            .iter()
            .map(|info| info.ticker.as_str().to_string())
            .collect();
        let rithmic_index = data::build_rithmic_contribution(feed_id, &resolved_ticker_strings);
        data::lock_or_recover(&self.persistence.data_index).merge(rithmic_index);
        self.rebuild_ticker_data();

        // Re-affiliate disconnected panes and collect any that need reloading
        let fallback_days = backfill_days;
        let main_window = self.main_window.id;
        let mut reload_tasks: Vec<Task<Message>> = Vec::new();
        for layout in &mut self.persistence.layout_manager.layouts {
            let lid = layout.id.unique;
            let reloads =
                layout
                    .dashboard
                    .affiliate_and_collect_reloads(feed_id, main_window, fallback_days);
            for (pane_id, config, ticker_info) in reloads {
                reload_tasks.push(Task::done(Message::Chart(ChartMessage::LoadChartData {
                    layout_id: lid,
                    pane_id,
                    config,
                    ticker_info,
                })));
            }
        }

        self.ui.push_notification(Toast::new(Notification::Info(
            "Rithmic connected".to_string(),
        )));

        // Clone the engine's event sender so streaming events are delivered
        // directly to the DataEngine event channel (and from there to the UI).
        let event_tx = self.services.event_tx.clone();

        // Start streaming task — sends DataEvents via the DataEngine event channel
        let streaming_task = Task::perform(
            rithmic_streaming_task(client, subscribed_tickers, ticker_map, event_tx),
            move |_| {
                // Streaming ended — treat as connection lost
                Message::DataEvent(data::DataEvent::ConnectionLost { feed_id })
            },
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

    pub(crate) fn handle_rithmic_connection_lost(&mut self) -> Task<Message> {
        let Some(feed_id) = self.services.rithmic_feed_id else {
            return Task::none();
        };

        // Clear stale client
        self.services.rithmic_client = None;

        // Unaffiliate panes so they show stale/disconnected status
        let main_window = self.main_window.id;
        for layout in &mut self.persistence.layout_manager.layouts {
            layout
                .dashboard
                .unaffiliate_panes_for_feed(feed_id, main_window);
        }

        let mut feed_manager = data::lock_or_recover(&self.connections.connection_manager);
        let auto_reconnect = feed_manager
            .get(feed_id)
            .and_then(|f| f.rithmic_config())
            .map(|c| c.auto_reconnect)
            .unwrap_or(false);

        const MAX_RECONNECT_ATTEMPTS: u32 = 10;

        if auto_reconnect {
            self.services.rithmic_reconnect_attempts += 1;
            let attempts = self.services.rithmic_reconnect_attempts;

            if attempts > MAX_RECONNECT_ATTEMPTS {
                feed_manager.set_status(
                    feed_id,
                    data::ConnectionStatus::Error(
                        "Max reconnect attempts exhausted".to_string(),
                    ),
                );
                drop(feed_manager);
                let cm_arc = self.connections.connection_manager.clone();
                let cm = data::lock_or_recover(&cm_arc);
                self.sync_feed_snapshots(&cm);
                self.ui.push_notification(Toast::error(
                    "Rithmic: max reconnect attempts exhausted"
                        .to_string(),
                ));
                return Task::none();
            }

            // Exponential backoff: 1s, 5s, 15s, 30s (capped)
            let delay_secs = match attempts {
                1 => 1,
                2 => 5,
                3 => 15,
                _ => 30,
            };

            feed_manager.set_status(
                feed_id,
                data::ConnectionStatus::Reconnecting { attempt: attempts },
            );
            drop(feed_manager);
            let cm_arc = self.connections.connection_manager.clone();
            let cm = data::lock_or_recover(&cm_arc);
            self.sync_feed_snapshots(&cm);
            drop(cm);
            self.ui
                .push_notification(Toast::new(Notification::Info(format!(
                    "Rithmic reconnecting in {}s (attempt {})...",
                    delay_secs, attempts
                ))));

            let delay = std::time::Duration::from_secs(delay_secs);
            return Task::perform(async move { tokio::time::sleep(delay).await }, move |_| {
                Message::DataFeeds(crate::modals::data_feeds::DataFeedsMessage::ConnectFeed(
                    feed_id,
                ))
            });
        }

        feed_manager.set_status(
            feed_id,
            data::ConnectionStatus::Error("Connection lost".to_string()),
        );
        drop(feed_manager);
        let cm_arc = self.connections.connection_manager.clone();
        let cm = data::lock_or_recover(&cm_arc);
        self.sync_feed_snapshots(&cm);
        self.ui
            .push_notification(Toast::error("Rithmic connection lost".to_string()));
        Task::none()
    }

    pub(crate) fn handle_rithmic_system_names(
        &mut self,
        server: data::RithmicServer,
        result: Result<Vec<String>, String>,
    ) {
        self.modals.data_feeds_modal.update(
            crate::modals::data_feeds::DataFeedsMessage::SystemNamesLoaded(server, result),
            &mut data::lock_or_recover(&self.connections.connection_manager),
        );
    }

    pub(crate) fn handle_rithmic_product_codes(&mut self, result: Result<Vec<String>, String>) {
        self.modals.data_feeds_modal.update(
            crate::modals::data_feeds::DataFeedsMessage::AvailableTickersLoaded(result),
            &mut data::lock_or_recover(&self.connections.connection_manager),
        );
    }
}

async fn rithmic_connect_task(
    engine: std::sync::Arc<tokio::sync::Mutex<data::engine::DataEngine>>,
    rithmic_config: data::RithmicConnectionConfig,
    password: String,
) -> Result<(), String> {
    let handle = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || {
        handle.block_on(rithmic_init_and_stage(engine, &rithmic_config, &password))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

async fn rithmic_init_and_stage(
    engine: std::sync::Arc<tokio::sync::Mutex<data::engine::DataEngine>>,
    rithmic_conn_config: &data::RithmicConnectionConfig,
    password: &str,
) -> Result<(), String> {
    // Build adapter-level configs from the connection config
    let (rithmic_config, protocol_config) =
        data::RithmicConfig::from_connection_config(rithmic_conn_config, password)
            .map_err(|e| e.to_string())?;

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        engine
            .lock()
            .await
            .connect_rithmic(rithmic_config, &protocol_config),
    )
    .await;

    match result {
        Ok(Ok((_feed_id, client))) => {
            let mut staging =
                data::lock_or_recover(crate::app::core::globals::get_rithmic_client_staging());
            *staging = Some(client);
            Ok(())
        }
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("Connection timed out after 30 seconds".to_string()),
    }
}

async fn rithmic_streaming_task(
    client: std::sync::Arc<tokio::sync::Mutex<data::RithmicClient>>,
    subscribed_tickers: Vec<String>,
    ticker_map: rustc_hash::FxHashMap<String, data::FuturesTickerInfo>,
    event_tx: Option<tokio::sync::mpsc::UnboundedSender<data::DataEvent>>,
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

    // Fetch available product codes from CME
    {
        let guard = client.lock().await;
        match guard.get_product_codes(Some("CME")).await {
            Ok(codes) if !codes.is_empty() => {
                log::info!("Rithmic: received {} product codes", codes.len());
                // Product codes are surfaced via DataEvent::ProductCodesReceived
                // but without a static sender here we just log them.
            }
            Ok(_) => {}
            Err(e) => {
                log::warn!("Failed to fetch product codes: {}", e);
            }
        }
    }

    if !sub_failures.is_empty() {
        log::warn!("Rithmic subscription failures: {:?}", sub_failures);
    }

    // Take ticker handle and start streaming
    let handle = {
        let mut guard = client.lock().await;
        guard.take_ticker_handle()
    };

    let Some(handle) = handle else {
        return;
    };

    if ticker_map.is_empty() {
        log::warn!(
            "No ticker info resolved for Rithmic subscription {:?}; \
             streaming will skip events without symbol match",
            subscribed_tickers
        );
    }

    let Some(event_tx) = event_tx else {
        log::error!("No DataEngine event sender available for Rithmic streaming");
        return;
    };

    let stream = data::adapter::rithmic::RithmicStream::new(handle);

    // Run streaming — events are sent directly to the DataEngine's event
    // channel, which the subscription monitor delivers to the UI as
    // Message::DataEvent.
    stream.run(ticker_map, event_tx).await;
}
