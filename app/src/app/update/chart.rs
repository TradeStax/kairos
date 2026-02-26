use iced::Task;

use crate::app::core::globals;
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
        ticker_info: data::FuturesTickerInfo,
    ) -> Task<Message> {
        log::info!(
            "LoadChartData message received for pane {}: {:?} chart",
            pane_id,
            config.chart_type
        );

        // Resolve the best available data feed (Databento > Rithmic)
        let resolved = {
            let feed_manager = data::lock_or_recover(&self.connections.connection_manager);
            feed_manager.resolve_for_chart()
        };

        let Some(resolved) = resolved else {
            log::warn!("No data feed connected - cannot load chart data");
            self.ui.push_notification(Toast::error(
                "No data feed connected. Connect a feed in \
                 connection settings."
                    .to_string(),
            ));
            return Task::done(Message::Dashboard {
                layout_id: Some(layout_id),
                event: Box::new(dashboard::Message::ChangePaneStatus(
                    pane_id,
                    LoadingStatus::Error {
                        message: "No data feed connected".to_string(),
                    },
                )),
            });
        };

        // Set feed_id on the pane so we know which feed owns its data
        if let Some(dashboard) = self.persistence.layout_manager.mut_dashboard(layout_id) {
            let main_window = self.main_window.id;
            if let Some(pane_state) = dashboard.get_mut_pane_state_by_uuid(main_window, pane_id) {
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
                ticker_info,
                result: Ok(chart_data),
            }));
        }

        log::info!("Chart load: provider={:?}", resolved.provider,);

        let Some(engine) = self.services.engine.clone() else {
            log::warn!("No data engine available");
            self.ui.push_notification(Toast::error(
                "No market data service available. Please check your \
                 data feed connection."
                    .to_string(),
            ));
            return Task::done(Message::Dashboard {
                layout_id: Some(layout_id),
                event: Box::new(dashboard::Message::ChangePaneStatus(
                    pane_id,
                    LoadingStatus::Error {
                        message: "No market data service available".to_string(),
                    },
                )),
            });
        };

        Task::perform(
            async move {
                log::info!(
                    "Starting async get_chart_data: {:?} {} range={} to {}",
                    config.chart_type,
                    config.ticker,
                    config.date_range.start,
                    config.date_range.end,
                );

                let mut guard = engine.lock().await;

                // Set progress callback so the per-day Rithmic loop
                // reports (days_loaded, days_total) to the UI poll.
                guard.progress_callback =
                    Some(Box::new(move |loaded, total| {
                        globals::set_chart_progress(pane_id, loaded, total);
                    }));

                let result = guard
                    .get_chart_data(
                        &config.ticker,
                        config.basis,
                        &config.date_range,
                        &ticker_info,
                    )
                    .await;

                guard.progress_callback = None;
                drop(guard);
                globals::clear_chart_progress(pane_id);

                match &result {
                    Ok(cd) => log::info!(
                        "get_chart_data completed: {} trades, {} candles",
                        cd.trades.len(),
                        cd.candles.len(),
                    ),
                    Err(e) => log::error!("get_chart_data failed: {}", e),
                }
                result.map_err(|e| e.to_string())
            },
            move |result| {
                Message::Chart(ChartMessage::ChartDataLoaded {
                    layout_id,
                    pane_id,
                    ticker_info,
                    result,
                })
            },
        )
    }

    pub(crate) fn handle_chart_data_loaded(
        &mut self,
        layout_id: uuid::Uuid,
        pane_id: uuid::Uuid,
        ticker_info: data::FuturesTickerInfo,
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
                    event: Box::new(dashboard::Message::ChartDataLoaded {
                        pane_id,
                        ticker_info,
                        chart_data,
                    }),
                });

                // Check if replay is active and this pane's ticker matches
                let replay_sync_task = self.maybe_sync_pane_to_replay(layout_id, pane_id);

                Task::batch([load_task, replay_sync_task])
            }
            Err(e) => {
                log::error!("Failed to load chart data for pane {}: {}", pane_id, e);
                self.ui
                    .push_notification(Toast::error(format!("Failed to load chart data: {}", e)));

                Task::done(Message::Dashboard {
                    layout_id: Some(layout_id),
                    event: Box::new(dashboard::Message::ChangePaneStatus(
                        pane_id,
                        LoadingStatus::Error { message: e },
                    )),
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
            .is_some_and(|ti| ti.ticker == replay_ticker);

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
                        event: Box::new(dashboard::Message::ReplaySyncPane { pane_id, trades }),
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
            .persistence
            .layout_manager
            .active_layout_id()
            .map(|id| id.unique)
        else {
            return Task::none();
        };

        let main_window = self.main_window.id;
        let mut reload_tasks = Vec::new();

        let fallback_days = self.ui.sidebar.date_range_preset().days();

        for layout in &mut self.persistence.layout_manager.layouts {
            for (_, _, state) in layout.dashboard.iter_all_panes(main_window) {
                let Some(ticker_info) = state.ticker_info else {
                    continue;
                };

                // Skip panes already loaded and actively receiving live
                // data.  DataIndexRebuilt should only trigger loads for
                // idle/errored panes — not wipe charts that are streaming.
                if state.feed_id.is_some()
                    && state.content.initialized()
                    && matches!(state.loading_status, LoadingStatus::Ready)
                {
                    continue;
                }

                let chart_type = state.content.kind().to_chart_type();
                let resolved_range = {
                    let index = data::lock_or_recover(&self.persistence.data_index);
                    index.resolve_chart_range(
                        ticker_info.ticker.as_str(),
                        chart_type,
                        Some(fallback_days),
                    )
                };

                let Some(range) = resolved_range else {
                    continue;
                };

                // Skip if already loaded with this range (unless in error state)
                if state.loaded_date_range == Some(range)
                    && !matches!(state.loading_status, LoadingStatus::Error { .. })
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

                reload_tasks.push(Task::done(Message::Chart(ChartMessage::LoadChartData {
                    layout_id: lid,
                    pane_id: state.unique_id(),
                    config,
                    ticker_info,
                })));
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
        let progress = globals::take_chart_progress();
        if progress.is_empty() {
            return Task::none();
        }

        let lid = self
            .persistence
            .layout_manager
            .active_layout_id()
            .map(|id| id.unique);

        let Some(layout_id) = lid else {
            return Task::none();
        };

        let tasks: Vec<_> = progress
            .into_iter()
            .map(|(pane_id, (days_loaded, days_total))| {
                let status = LoadingStatus::LoadingFromCache {
                    schema: data::DataSchema::Trades,
                    days_total,
                    days_loaded,
                    items_loaded: 0,
                };
                Task::done(Message::Dashboard {
                    layout_id: Some(layout_id),
                    event: Box::new(
                        dashboard::Message::ChangePaneStatus(pane_id, status),
                    ),
                })
            })
            .collect();

        Task::batch(tasks)
    }
}
