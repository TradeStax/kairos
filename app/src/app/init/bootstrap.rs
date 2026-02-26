use iced::Task;

use super::super::{Kairos, Message};
use super::services;

impl Kairos {
    /// Seed the DataIndex from the persisted DownloadedTickersRegistry.
    pub(crate) fn seed_data_index_from_registry(
        registry: &data::DownloadedTickersRegistry,
        data_index: &std::sync::Arc<std::sync::Mutex<data::DataIndex>>,
    ) {
        let mut idx = data::lock_or_recover(data_index);
        for ticker_str in registry.list_tickers() {
            if let Some(range) = registry.get_range_by_ticker_str(&ticker_str) {
                let mut dates = std::collections::BTreeSet::new();
                for d in range.dates() {
                    dates.insert(d);
                }
                idx.add_contribution(
                    data::DataKey {
                        ticker: ticker_str,
                        schema: "trades".to_string(),
                    },
                    Self::REGISTRY_SENTINEL_FEED,
                    dates,
                    false,
                );
            }
        }
    }

    /// Auto-connect feeds with `auto_connect` enabled and an API key
    /// present. Returns tasks for async operations.
    pub(crate) fn auto_connect_feeds(
        state: &mut Self,
        secrets: &crate::infra::secrets::SecretsManager,
    ) -> Vec<Task<Message>> {
        let mut scan_tasks: Vec<Task<Message>> = Vec::new();
        let mut connection_manager = data::lock_or_recover(&state.connections.connection_manager);

        let auto_connect_ids: Vec<data::FeedId> = connection_manager
            .connections()
            .iter()
            .filter(|c| c.auto_connect && c.enabled)
            .map(|c| c.id)
            .collect();

        let mut rithmic_auto_connect: Vec<data::FeedId> = Vec::new();

        for fid in &auto_connect_ids {
            let conn_snapshot = connection_manager.get(*fid).map(|c| c.provider);

            let Some(provider) = conn_snapshot else {
                continue;
            };

            match provider {
                data::ConnectionProvider::Databento => {
                    let has_key =
                        secrets.has_api_key(crate::config::secrets::ApiProvider::Databento);
                    if !has_key {
                        continue;
                    }
                    connection_manager.set_status(*fid, data::ConnectionStatus::Connected);
                    log::info!("Auto-connected Databento connection {} on startup", fid);
                }
                data::ConnectionProvider::Rithmic => {
                    if secrets.has_feed_password(&fid.to_string()) {
                        rithmic_auto_connect.push(*fid);
                    } else {
                        log::info!(
                            "Skipping Rithmic auto-connect for connection {}: \
                             no password stored",
                            fid
                        );
                    }
                }
            }
        }

        // Drop the lock before issuing connect tasks
        drop(connection_manager);

        // Initiate actual Rithmic connections via DataFeeds message.
        // The feeds handler will look up the connection in ConnectionManager.
        for fid in rithmic_auto_connect {
            scan_tasks.push(iced::Task::done(Message::DataFeeds(
                crate::modals::data_feeds::DataFeedsMessage::ConnectFeed(fid),
            )));
        }

        scan_tasks
    }

    /// Wire up the DataEngine after async init completes, load the layout,
    /// and auto-connect feeds.
    pub(crate) fn handle_data_engine_ready(
        &mut self,
        result: Result<services::DataEngineInit, String>,
    ) -> Task<Message> {
        match result {
            Ok(init) => {
                // Take the event receiver from the init wrapper and place it
                // in the global slot so the subscription stream can take it
                // exactly once.
                if let Ok(mut guard) = init.event_rx.lock()
                    && let Some(rx) = guard.take()
                {
                    crate::app::core::globals::set_data_event_receiver(rx);
                }

                // Store the wrapped engine
                self.services.engine = Some(init.engine.clone());

                // Clone the event sender for direct use by streaming tasks
                // (avoids locking the engine mutex at connect time)
                let event_tx = init.engine.blocking_lock().event_sender();
                self.services.event_tx = Some(event_tx);

                // Wire up the trade provider for the backtest engine
                self.modals.backtest.backtest_trade_provider = Some(std::sync::Arc::new(
                    crate::services::trade_provider::EngineTradeProvider::new(init.engine),
                )
                    as std::sync::Arc<dyn backtest::TradeProvider>);

                log::info!("DataEngine ready — services wired up");
            }
            Err(e) => {
                log::error!("DataEngine initialization failed: {}", e);
                self.ui
                    .push_notification(crate::components::display::toast::Toast::error(format!(
                        "Data engine failed to initialize: {}",
                        e
                    )));
            }
        }

        // Update layout manager (no market_data_service anymore)
        self.persistence
            .layout_manager
            .update_shared_state(self.persistence.data_index.clone());

        // Load the active layout now that services are ready
        let main_window_id = self.main_window.id;
        let load_layout = if let Some(active_layout_id) = self
            .persistence
            .layout_manager
            .active_layout_id()
            .or_else(|| {
                self.persistence
                    .layout_manager
                    .layouts
                    .first()
                    .map(|l| &l.id)
            }) {
            self.load_layout(active_layout_id.unique, main_window_id)
        } else {
            log::error!("No layouts available at startup");
            Task::none()
        };

        // Auto-connect feeds
        let mut scan_tasks = Self::auto_connect_feeds(self, &self.secrets.clone());

        // Populate tickers from DataIndex
        self.rebuild_ticker_data();
        if !self.persistence.tickers_info.is_empty() {
            log::info!(
                "Populated {} tickers from DataIndex at startup",
                self.persistence.tickers_info.len()
            );
        }

        {
            let cm_arc = self.connections.connection_manager.clone();
            let connection_manager = data::lock_or_recover(&cm_arc);
            self.sync_feed_snapshots(&connection_manager);
        }

        let mut all_tasks = vec![load_layout];
        all_tasks.append(&mut scan_tasks);
        Task::batch(all_tasks)
    }
}
