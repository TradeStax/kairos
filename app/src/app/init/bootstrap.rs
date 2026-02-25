use iced::Task;

use super::services;
use super::super::{Kairos, Message};

impl Kairos {
    /// Seed the DataIndex from the persisted DownloadedTickersRegistry.
    pub(crate) fn seed_data_index_from_registry(
        registry: &data::DownloadedTickersRegistry,
        data_index: &std::sync::Arc<std::sync::Mutex<data::DataIndex>>,
    ) {
        let mut idx = data::lock_or_recover(data_index);
        for ticker_str in registry.list_tickers() {
            if let Some(range) =
                registry.get_range_by_ticker_str(&ticker_str)
            {
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
    /// present. Returns tasks for async cache scans.
    pub(crate) fn auto_connect_feeds(
        state: &mut Self,
        secrets: &crate::infra::secrets::SecretsManager,
    ) -> Vec<Task<Message>> {
        let mut scan_tasks: Vec<Task<Message>> = Vec::new();
        let mut feed_manager =
            data::lock_or_recover(&state.connections.data_feed_manager);

        let auto_connect_ids: Vec<data::FeedId> = feed_manager
            .feeds()
            .iter()
            .filter(|f| f.auto_connect && f.enabled)
            .map(|f| f.id)
            .collect();

        let mut rithmic_auto_connect: Vec<data::FeedId> = Vec::new();

        for fid in &auto_connect_ids {
            let feed_snapshot = feed_manager
                .get(*fid)
                .map(|f| (f.provider, f.dataset_info().cloned()));

            let Some((provider, dataset_info)) = feed_snapshot else {
                continue;
            };

            match provider {
                data::FeedProvider::Databento => {
                    let has_key = secrets.has_api_key(
                        data::config::secrets::ApiProvider::Databento,
                    );
                    if !has_key {
                        continue;
                    }
                    feed_manager
                        .set_status(*fid, data::FeedStatus::Connected);
                    log::info!(
                        "Auto-connected Databento feed {} on startup",
                        fid
                    );

                    if let Some(info) = &dataset_info {
                        let mut dates =
                            std::collections::BTreeSet::new();
                        for d in info.date_range.dates() {
                            dates.insert(d);
                        }
                        let mut idx =
                            data::lock_or_recover(&state.persistence.data_index);
                        idx.add_contribution(
                            data::DataKey {
                                ticker: info.ticker.clone(),
                                schema: "trades".to_string(),
                            },
                            *fid,
                            dates,
                            false,
                        );
                    }

                    let cache_root =
                        crate::infra::platform::data_path(Some(
                            "cache/databento",
                        ));
                    let feed_id = *fid;
                    scan_tasks.push(Task::perform(
                        async move {
                            exchange::scan_databento_cache(
                                &cache_root, feed_id,
                            )
                            .await
                        },
                        Message::DataIndexRebuilt,
                    ));
                }
                data::FeedProvider::Rithmic => {
                    // C2: Check per-feed password (not global API key)
                    if secrets.has_feed_password(&fid.to_string()) {
                        rithmic_auto_connect.push(*fid);
                    } else {
                        log::info!(
                            "Skipping Rithmic auto-connect for feed {}: \
                             no password stored",
                            fid
                        );
                    }
                }
            }
        }

        // C2: Drop the lock before issuing connect tasks
        drop(feed_manager);

        // Initiate actual Rithmic connections (returns async tasks)
        for fid in rithmic_auto_connect {
            let dm_arc = state.connections.data_feed_manager.clone();
            let fm = data::lock_or_recover(&dm_arc);
            let task = state.connect_rithmic_feed(fid, fm);
            scan_tasks.push(task);
        }

        scan_tasks
    }

    /// Wire up services after async init completes, load the layout,
    /// and auto-connect feeds.
    pub(crate) fn handle_services_ready(
        &mut self,
        result: services::AllServicesResult,
    ) -> Task<Message> {
        let market_data_service =
            result.market_data.as_ref().map(|r| r.service.clone());
        let replay_engine =
            services::create_replay_engine(result.market_data.as_ref());

        self.services.market_data_service = market_data_service.clone();
        self.services.replay_engine = replay_engine;

        // Wire up the trade repo for the backtest engine (same repo as market data)
        self.modals.backtest.backtest_trade_repo = result
            .market_data
            .as_ref()
            .map(|r| std::sync::Arc::clone(&r.trade_repo) as std::sync::Arc<dyn data::TradeRepository>);

        #[cfg(feature = "options")]
        {
            self.services.options_service = result.options;
        }

        // Update layout manager with the live service
        self.persistence.layout_manager.update_shared_state(
            market_data_service,
            self.persistence.data_index.clone(),
        );

        // Load the active layout now that services are ready
        let main_window_id = self.main_window.id;
        let load_layout = if let Some(active_layout_id) = self
            .persistence.layout_manager
            .active_layout_id()
            .or_else(|| {
                self.persistence.layout_manager.layouts.first().map(|l| &l.id)
            })
        {
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
            let dm_arc = self.connections.data_feed_manager.clone();
            let feed_manager = data::lock_or_recover(&dm_arc);
            self.sync_feed_snapshots(&feed_manager);
        }

        let mut all_tasks = vec![load_layout];
        all_tasks.append(&mut scan_tasks);
        Task::batch(all_tasks)
    }
}
