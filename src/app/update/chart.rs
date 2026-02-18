use iced::Task;

use crate::screen::dashboard;
use crate::widget::toast::{Notification, Toast};
use data::LoadingStatus;

use super::super::{ChartMessage, Flowsurface, Message};

impl Flowsurface {
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

                Task::done(Message::Dashboard {
                    layout_id: Some(layout_id),
                    event: dashboard::Message::ChartDataLoaded {
                        pane_id,
                        chart_data,
                    },
                })
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

    pub(crate) fn handle_replay_event(&mut self, event: data::services::ReplayEvent) {
        log::debug!("Replay event: {:?}", event);
        use data::services::ReplayEvent;

        match event {
            ReplayEvent::DataLoaded {
                ticker,
                trade_count,
                depth_count,
                time_range,
            } => {
                log::info!(
                    "Replay data loaded for {:?}: {} trades, \
                     {} depth snapshots, range: {:?}",
                    ticker,
                    trade_count,
                    depth_count,
                    time_range
                );
                self.notifications
                    .push(Toast::new(Notification::Info(format!(
                        "Replay data loaded: {} trades",
                        trade_count
                    ))));
            }
            ReplayEvent::LoadingProgress { progress, message } => {
                log::debug!(
                    "Replay loading: {}% - {}",
                    (progress * 100.0) as u32,
                    message
                );
            }
            ReplayEvent::MarketData {
                timestamp,
                trades,
                depth,
            } => {
                log::debug!(
                    "Replay market data at {}: {} trades, depth: {}",
                    timestamp,
                    trades.len(),
                    depth.is_some()
                );
            }
            ReplayEvent::PositionUpdate {
                timestamp,
                progress,
            } => {
                log::debug!("Replay position: {} ({:.1}%)", timestamp, progress * 100.0);
            }
            ReplayEvent::StatusChanged(status) => {
                log::info!("Replay status changed: {:?}", status);
            }
            ReplayEvent::PlaybackComplete => {
                log::info!("Replay playback completed");
                self.notifications.push(Toast::new(Notification::Info(
                    "Replay completed".to_string(),
                )));
            }
            ReplayEvent::PlaybackStarted => {
                log::info!("Replay playback started");
            }
            ReplayEvent::PlaybackPaused => {
                log::info!("Replay playback paused");
            }
            ReplayEvent::PlaybackStopped => {
                log::info!("Replay playback stopped");
            }
            ReplayEvent::SeekCompleted {
                timestamp,
                progress,
            } => {
                log::info!(
                    "Replay seek completed to {} ({:.1}%)",
                    timestamp,
                    progress * 100.0
                );
            }
            ReplayEvent::SpeedChanged(speed) => {
                log::info!("Replay speed changed to {:?}", speed);
            }
            ReplayEvent::CacheHit { symbol, date_range } => {
                log::debug!("Replay cache hit for {} in range {:?}", symbol, date_range);
            }
            ReplayEvent::MemoryUsage {
                bytes,
                trades,
                depth_snapshots,
            } => {
                log::debug!(
                    "Replay memory usage: {:.2} MB ({} trades, \
                     {} depth snapshots)",
                    bytes as f32 / 1024.0 / 1024.0,
                    trades,
                    depth_snapshots
                );
            }
            ReplayEvent::Error(err) => {
                log::error!("Replay error: {}", err);
                self.notifications
                    .push(Toast::error(format!("Replay error: {}", err)));
            }
        }
    }

    pub(crate) fn handle_update_loading_status(&mut self) -> Task<Message> {
        let Some(service) = &self.market_data_service else {
            return Task::none();
        };

        let all_statuses = service.get_all_loading_statuses();

        for (chart_key, status) in all_statuses {
            for layout in &self.layout_manager.layouts {
                if let Some((pane_id, _)) =
                    layout.dashboard.charts.iter().find(|(_, chart_state)| {
                        let config = &chart_state.config;
                        let key = format!(
                            "{:?}-{:?}-{:?}",
                            config.ticker, config.basis, config.date_range
                        );
                        key == chart_key
                    })
                {
                    return Task::done(Message::Dashboard {
                        layout_id: Some(layout.id.unique),
                        event: dashboard::Message::ChangePaneStatus(*pane_id, status.clone()),
                    });
                }
            }
        }

        Task::none()
    }
}
