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

        // Resolve the best available data feed (Databento > Rithmic)
        let resolved = {
            let feed_manager =
                data::lock_or_recover(&self.connections.data_feed_manager);
            feed_manager.resolve_feed_for_chart()
        };

        let Some(resolved) = resolved else {
            log::warn!("No data feed connected - cannot load chart data");
            self.ui.notifications.push(Toast::error(
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
        };

        // Set feed_id on the pane so we know which feed owns its data
        if let Some(dashboard) = self.persistence.layout_manager.mut_dashboard(layout_id) {
            let main_window = self.main_window.id;
            if let Some(pane_state) =
                dashboard.get_mut_pane_state_by_uuid(main_window, pane_id)
            {
                pane_state.feed_id = Some(resolved.feed_id);
            }
        }

        // Realtime-only feed (e.g. Rithmic): create empty chart data
        // and let the live stream populate it
        if !resolved.has_historical {
            log::info!(
                "Realtime-only feed for pane {} — empty chart, \
                 live data will populate",
                pane_id
            );
            let chart_data = data::ChartData::from_trades(vec![], vec![]);
            return Task::done(Message::Chart(ChartMessage::ChartDataLoaded {
                layout_id,
                pane_id,
                result: Ok(chart_data),
            }));
        }

        let Some(service) = self.require_market_service() else {
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
                self.ui.notifications
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
        if !self.modals.replay_manager.data_loaded {
            return Task::none();
        }

        let Some(ref stream) = self.modals.replay_manager.selected_stream else {
            return Task::none();
        };
        let replay_ticker = stream.ticker_info.ticker;

        // Find the pane in the layout and check its ticker
        let Some(layout) = self.persistence.layout_manager.get(layout_id) else {
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

        let Some(engine) = self.services.replay_engine.clone() else {
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
            let mut index = data::lock_or_recover(&self.persistence.data_index);
            index.merge(new_index);
        }

        // 2. Rebuild tickers_info and ticker_ranges from the index
        self.rebuild_ticker_data();
        log::info!(
            "DataIndex rebuilt - {} ticker(s) available",
            self.persistence.tickers_info.len()
        );

        // 3. For each pane with a ticker, resolve the range and reload
        //    if the resolved range differs from the pane's current loaded range
        let Some(lid) = self
            .persistence.layout_manager
            .active_layout_id()
            .map(|id| id.unique)
        else {
            return Task::none();
        };

        let main_window = self.main_window.id;
        let mut reload_tasks = Vec::new();

        for layout in &mut self.persistence.layout_manager.layouts {
            for (_, _, state) in layout.dashboard.iter_all_panes(main_window) {
                let Some(ticker_info) = state.ticker_info else {
                    continue;
                };

                let chart_type = state.content.kind().to_chart_type();
                let resolved_range = {
                    let index =
                        data::lock_or_recover(&self.persistence.data_index);
                    index.resolve_chart_range(
                        ticker_info.ticker.as_str(),
                        chart_type,
                    )
                };

                let Some(range) = resolved_range else {
                    continue;
                };

                // Skip if already loaded with this range (unless in error state)
                if state.loaded_date_range == Some(range)
                    && !matches!(
                        state.loading_status,
                        LoadingStatus::Error { .. }
                    )
                {
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
        let Some(service) = self.services.market_data_service.clone() else {
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
        let mut tasks = Vec::new();

        for (chart_key, status) in all_statuses {
            for layout in &self.persistence.layout_manager.layouts {
                // Check main panes
                for (_, state) in layout.dashboard.panes.iter() {
                    let Some(ticker_info) = state.ticker_info else {
                        continue;
                    };
                    let basis = state
                        .settings
                        .selected_basis
                        .unwrap_or(data::ChartBasis::Time(data::Timeframe::M5));
                    let Some(date_range) = state.loaded_date_range else {
                        continue;
                    };
                    let key = format!(
                        "{}-{:?}-{:?}",
                        ticker_info.ticker, basis, date_range
                    );
                    if key == chart_key {
                        tasks.push(Task::done(Message::Dashboard {
                            layout_id: Some(layout.id.unique),
                            event: dashboard::Message::ChangePaneStatus(
                                state.unique_id(),
                                status.clone(),
                            ),
                        }));
                    }
                }
                // Check popout panes
                for (_, (popout_panes, _)) in &layout.dashboard.popout {
                    for (_, state) in popout_panes.iter() {
                        let Some(ticker_info) = state.ticker_info else {
                            continue;
                        };
                        let basis = state
                            .settings
                            .selected_basis
                            .unwrap_or(
                                data::ChartBasis::Time(data::Timeframe::M5),
                            );
                        let Some(date_range) = state.loaded_date_range
                        else {
                            continue;
                        };
                        let key = format!(
                            "{}-{:?}-{:?}",
                            ticker_info.ticker, basis, date_range
                        );
                        if key == chart_key {
                            tasks.push(Task::done(Message::Dashboard {
                                layout_id: Some(layout.id.unique),
                                event: dashboard::Message::ChangePaneStatus(
                                    state.unique_id(),
                                    status.clone(),
                                ),
                            }));
                        }
                    }
                }
            }
        }

        if tasks.is_empty() {
            Task::none()
        } else {
            Task::batch(tasks)
        }
    }
}
