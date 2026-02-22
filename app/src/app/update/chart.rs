use iced::Task;

use crate::components::display::toast::Toast;
use crate::screen::dashboard;
use data::LoadingStatus;

use super::super::{ChartMessage, Kairos, Message};

impl Kairos {
    pub(crate) fn handle_load_chart_data(
        &mut self,
        layout_id: uuid::Uuid,
        pane_id: uuid::Uuid,
        config: data::ChartConfig,
        ticker_info: exchange::FuturesTickerInfo,
    ) -> Task<Message> {
        log::info!(
            "LoadChartData message received for pane {}: {:?} chart",
            pane_id,
            config.chart_type
        );

        // Validate that a Databento feed is connected and track which feed
        let databento_feed_id = {
            let feed_manager = self
                .data_feed_manager
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            match feed_manager.connected_feed_id_for_provider(data::FeedProvider::Databento) {
                Some(fid) => fid,
                None => {
                    log::warn!("No Databento feed connected - cannot load chart data");
                    self.notifications.push(Toast::error(
                        "No data feed connected. Connect a feed in \
                         connection settings."
                            .to_string(),
                    ));
                    return Task::done(Message::Dashboard {
                        layout_id: Some(layout_id),
                        event: dashboard::Message::ChangePaneStatus(
                            pane_id,
                            LoadingStatus::Error {
                                message: "No data feed connected".to_string(),
                            },
                        ),
                    });
                }
            }
        };

        // Set feed_id on the pane so we know which feed owns its data
        if let Some(dashboard) = self.layout_manager.mut_dashboard(layout_id) {
            let main_window = self.main_window.id;
            if let Some(pane_state) = dashboard.get_mut_pane_state_by_uuid(main_window, pane_id) {
                pane_state.feed_id = Some(databento_feed_id);
            }
        }

        let Some(service) = self.market_data_service.clone() else {
            log::warn!("Market data service not available (API key not configured)");
            self.notifications.push(Toast::error(
                "Databento API key not configured. Set it in connection \
                 settings."
                    .to_string(),
            ));
            return Task::none();
        };

        Task::perform(
            async move {
                log::info!(
                    "Starting async get_chart_data for {:?}...",
                    config.chart_type
                );
                let result = service.get_chart_data(&config, &ticker_info).await;
                log::info!(
                    "get_chart_data completed: {}",
                    if result.is_ok() { "SUCCESS" } else { "ERROR" }
                );
                result.map_err(|e| e.to_string())
            },
            move |result| {
                Message::Chart(ChartMessage::ChartDataLoaded {
                    layout_id,
                    pane_id,
                    result,
                })
            },
        )
    }

    pub(crate) fn handle_chart_data_loaded(
        &mut self,
        layout_id: uuid::Uuid,
        pane_id: uuid::Uuid,
        result: Result<data::ChartData, String>,
    ) -> Task<Message> {
        match result {
            Ok(chart_data) => {
                log::info!(
                    "Chart data loaded for pane {}: {} trades, {} candles",
                    pane_id,
                    chart_data.trades.len(),
                    chart_data.candles.len()
                );

                let load_task = Task::done(Message::Dashboard {
                    layout_id: Some(layout_id),
                    event: dashboard::Message::ChartDataLoaded {
                        pane_id,
                        chart_data,
                    },
                });

                // Check if replay is active and this pane's ticker matches
                let replay_sync_task = self.maybe_sync_pane_to_replay(layout_id, pane_id);

                Task::batch([load_task, replay_sync_task])
            }
            Err(e) => {
                log::error!("Failed to load chart data for pane {}: {}", pane_id, e);
                self.notifications
                    .push(Toast::error(format!("Failed to load chart data: {}", e)));

                Task::done(Message::Dashboard {
                    layout_id: Some(layout_id),
                    event: dashboard::Message::ChangePaneStatus(
                        pane_id,
                        LoadingStatus::Error { message: e },
                    ),
                })
            }
        }
    }

    /// If a replay is active and the pane's ticker matches, spawn a task to
    /// fetch trades up to the current position and sync the pane into replay.
    fn maybe_sync_pane_to_replay(
        &self,
        layout_id: uuid::Uuid,
        pane_id: uuid::Uuid,
    ) -> Task<Message> {
        if !self.replay_manager.data_loaded {
            return Task::none();
        }

        let Some(ref stream) = self.replay_manager.selected_stream else {
            return Task::none();
        };
        let replay_ticker = stream.ticker_info.ticker;

        // Find the pane in the layout and check its ticker
        let Some(layout) = self.layout_manager.get(layout_id) else {
            return Task::none();
        };

        let pane_state = layout
            .dashboard
            .panes
            .iter()
            .map(|(_, state)| state)
            .chain(
                layout
                    .dashboard
                    .popout
                    .values()
                    .flat_map(|(panes, _)| panes.iter().map(|(_, s)| s)),
            )
            .find(|state| state.unique_id() == pane_id);

        let Some(pane_state) = pane_state else {
            return Task::none();
        };

        let ticker_matches = pane_state
            .ticker_info
            .map_or(false, |ti| ti.ticker == replay_ticker);

        if !ticker_matches || pane_state.is_replaying() {
            return Task::none();
        }

        let Some(engine) = self.replay_engine.clone() else {
            return Task::none();
        };

        Task::perform(
            async move {
                let guard = engine.lock().await;
                guard.get_rebuild_trades().await
            },
            move |trades| {
                if let Some(trades) = trades {
                    Message::Dashboard {
                        layout_id: Some(layout_id),
                        event: dashboard::Message::ReplaySyncPane { pane_id, trades },
                    }
                } else {
                    Message::Tick(std::time::Instant::now())
                }
            },
        )
    }

    pub(crate) fn handle_data_index_rebuilt(
        &mut self,
        result: Result<data::DataIndex, String>,
    ) -> Task<Message> {
        let new_index = match result {
            Ok(idx) => idx,
            Err(e) => {
                log::error!("Failed to scan cache for DataIndex: {}", e);
                return Task::none();
            }
        };

        // 1. Merge the scanned index into the shared DataIndex
        {
            let mut index = data::lock_or_recover(&self.data_index);
            index.merge(new_index);
        }

        // 2. Rebuild tickers_info and ticker_ranges from the index
        let available_tickers: std::collections::HashSet<String> = {
            let index = data::lock_or_recover(&self.data_index);
            index.available_tickers().into_iter().collect()
        };
        self.tickers_info = super::super::build_tickers_info(available_tickers);
        self.ticker_ranges = Self::build_ticker_ranges(&self.data_index);
        log::info!(
            "DataIndex rebuilt - {} ticker(s) available",
            self.tickers_info.len()
        );

        // 3. For each pane with a ticker, resolve the range and reload
        //    if the resolved range differs from the pane's current loaded range
        let Some(lid) = self
            .layout_manager
            .active_layout_id()
            .map(|id| id.unique)
        else {
            return Task::none();
        };

        let main_window = self.main_window.id;
        let mut reload_tasks = Vec::new();

        for layout in &mut self.layout_manager.layouts {
            for (_, _, state) in layout.dashboard.iter_all_panes(main_window) {
                let Some(ticker_info) = state.ticker_info else {
                    continue;
                };

                let chart_type = state.content.kind().to_chart_type();
                let resolved_range = {
                    let index = self
                        .data_index
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    index.resolve_chart_range(
                        ticker_info.ticker.as_str(),
                        chart_type,
                    )
                };

                let Some(range) = resolved_range else {
                    continue;
                };

                // Skip if already loaded with this range
                if state.loaded_date_range == Some(range) {
                    continue;
                }

                let basis = state
                    .settings
                    .selected_basis
                    .unwrap_or(data::ChartBasis::Time(data::Timeframe::M5));

                let config = data::ChartConfig {
                    chart_type,
                    basis,
                    ticker: ticker_info.ticker,
                    date_range: range,
                };

                reload_tasks.push(Task::done(Message::Chart(
                    ChartMessage::LoadChartData {
                        layout_id: lid,
                        pane_id: state.unique_id(),
                        config,
                        ticker_info,
                    },
                )));
            }
        }

        if !reload_tasks.is_empty() {
            log::info!(
                "Reloading {} pane(s) after DataIndex rebuild",
                reload_tasks.len()
            );
            return Task::batch(reload_tasks);
        }

        Task::none()
    }

    pub(crate) fn fetch_loading_statuses(&mut self) -> Task<Message> {
        let Some(service) = self.market_data_service.clone() else {
            return Task::none();
        };

        Task::perform(
            async move { service.get_all_loading_statuses().await },
            |statuses| {
                Message::Chart(ChartMessage::LoadingStatusesReady(statuses))
            },
        )
    }

    pub(crate) fn dispatch_loading_statuses(
        &mut self,
        all_statuses: std::collections::HashMap<String, LoadingStatus>,
    ) -> Task<Message> {
        for (chart_key, status) in all_statuses {
            for layout in &self.layout_manager.layouts {
                if let Some((pane_id, _)) =
                    layout.dashboard.charts.iter().find(|(_, chart_state)| {
                        let config = &chart_state.config;
                        let key = format!(
                            "{}-{:?}-{:?}",
                            config.ticker, config.basis, config.date_range
                        );
                        key == chart_key
                    })
                {
                    return Task::done(Message::Dashboard {
                        layout_id: Some(layout.id.unique),
                        event: dashboard::Message::ChangePaneStatus(
                            *pane_id,
                            status.clone(),
                        ),
                    });
                }
            }
        }

        Task::none()
    }
}
