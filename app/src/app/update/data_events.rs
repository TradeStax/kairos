use iced::Task;

use crate::app::{Kairos, Message};

impl Kairos {
    /// Handle events from the DataEngine event channel.
    ///
    /// This replaces the old `handle_rithmic_stream_event` handler.
    /// `DataEvent` is the unified event type for all connection lifecycle,
    /// market data, and download events from the DataEngine.
    pub(crate) fn handle_data_event(&mut self, event: data::DataEvent) -> Task<Message> {
        match event {
            data::DataEvent::Connected { feed_id, provider } => {
                log::info!(
                    "DataEngine: {:?} connected (feed_id: {})",
                    provider,
                    feed_id
                );
                // Update connection status and sync snapshots in a single lock scope
                let cm_arc = self.connections.connection_manager.clone();
                let mut cm = data::lock_or_recover(&cm_arc);
                cm.set_status(feed_id, data::ConnectionStatus::Connected);
                self.sync_feed_snapshots(&cm);

                // Refresh replay streams if the sidebar is open
                if self
                    .ui
                    .sidebar
                    .is_menu_active(crate::config::sidebar::Menu::Replay)
                {
                    let ticker_infos: std::collections::HashMap<String, data::FuturesTickerInfo> =
                        self.persistence
                            .tickers_info
                            .iter()
                            .map(|(t, i)| (t.to_string(), *i))
                            .collect();
                    self.modals
                        .replay_manager
                        .refresh_streams(&cm, &ticker_infos);
                }
                Task::none()
            }
            data::DataEvent::Disconnected { feed_id, reason } => {
                log::info!("DataEngine: disconnected feed {} — {}", feed_id, reason);
                let cm_arc = self.connections.connection_manager.clone();
                let mut cm = data::lock_or_recover(&cm_arc);
                cm.set_status(feed_id, data::ConnectionStatus::Disconnected);
                self.sync_feed_snapshots(&cm);

                // Refresh replay streams if the sidebar is open
                if self
                    .ui
                    .sidebar
                    .is_menu_active(crate::config::sidebar::Menu::Replay)
                {
                    let ticker_infos: std::collections::HashMap<String, data::FuturesTickerInfo> =
                        self.persistence
                            .tickers_info
                            .iter()
                            .map(|(t, i)| (t.to_string(), *i))
                            .collect();
                    self.modals
                        .replay_manager
                        .refresh_streams(&cm, &ticker_infos);
                }
                Task::none()
            }
            data::DataEvent::ConnectionLost { feed_id: _ } => {
                // Delegate to the reconnection handler which clears the
                // stale client, unaffiliates panes, and either starts
                // auto-reconnect with exponential backoff or sets Error
                // status.
                self.handle_rithmic_connection_lost()
            }
            data::DataEvent::Reconnecting { feed_id, attempt } => {
                log::info!(
                    "DataEngine: reconnecting feed {} (attempt {})",
                    feed_id,
                    attempt
                );
                let cm_arc = self.connections.connection_manager.clone();
                let mut cm = data::lock_or_recover(&cm_arc);
                cm.set_status(feed_id, data::ConnectionStatus::Reconnecting { attempt });
                self.sync_feed_snapshots(&cm);
                Task::none()
            }
            data::DataEvent::TradeReceived { .. } => {
                self.handle_dashboard(None, crate::screen::dashboard::Message::LiveData(event))
            }
            #[cfg(feature = "heatmap")]
            data::DataEvent::DepthReceived { .. } => {
                self.handle_dashboard(None, crate::screen::dashboard::Message::LiveData(event))
            }
            data::DataEvent::SubscriptionActive { ticker } => {
                log::debug!("DataEngine: subscription active for {}", ticker);
                Task::none()
            }
            data::DataEvent::SubscriptionFailed { ticker, reason } => {
                log::warn!("DataEngine: subscription failed for {}: {}", ticker, reason);
                self.ui
                    .push_notification(crate::components::display::toast::Toast::error(format!(
                        "Failed to subscribe to {}: {}",
                        ticker, reason
                    )));
                Task::none()
            }
            data::DataEvent::ProductCodesReceived(codes) => {
                let feed_id = self.services.rithmic_feed_id.unwrap_or(data::FeedId::nil());
                Task::done(Message::RithmicProductCodes {
                    _feed_id: feed_id,
                    result: Ok(codes),
                })
            }
            data::DataEvent::DownloadProgress {
                request_id: _,
                current_day,
                total_days,
                sub_day_fraction,
            } => {
                // Route download progress to the active download pane.
                // The pane_id is not available here; progress is broadcast.
                let pane_id = self
                    .modals
                    .historical_download_id
                    .unwrap_or(uuid::Uuid::nil());
                Task::done(Message::Download(
                    crate::app::DownloadMessage::DataDownloadProgress {
                        pane_id,
                        current: current_day,
                        total: total_days,
                        sub_day_fraction,
                    },
                ))
            }
            data::DataEvent::DownloadComplete {
                request_id: _,
                days_cached: _,
            } => {
                // Trigger a DataIndex rebuild after download completes.
                self.rebuild_ticker_data();
                Task::none()
            }
            data::DataEvent::ChartLoadProgress {
                pane_id,
                days_loaded,
                days_total,
            } => {
                // Push-based chart loading progress — replaces the old
                // polled CHART_LOAD_PROGRESS HashMap.
                let layout_id = self
                    .persistence
                    .layout_manager
                    .active_layout_id()
                    .map(|id| id.unique);

                let Some(layout_id) = layout_id else {
                    return Task::none();
                };

                let progress_fraction = if days_total > 0 {
                    Some(days_loaded / days_total as f32)
                } else {
                    None
                };
                let status = data::LoadingStatus::LoadingFromCache {
                    schema: data::DataSchema::Trades,
                    days_total,
                    days_loaded: days_loaded.floor() as usize,
                    items_loaded: 0,
                    progress_fraction,
                };
                Task::done(Message::Dashboard {
                    layout_id: Some(layout_id),
                    event: Box::new(crate::screen::dashboard::Message::ChangePaneStatus(
                        pane_id, status,
                    )),
                })
            }
            data::DataEvent::DataIndexUpdated(index) => {
                // Merge updated index into our shared DataIndex, then
                // strip contributions from disconnected feeds.
                let active_feeds = {
                    let cm = data::lock_or_recover(&self.connections.connection_manager);
                    cm.active_feed_ids()
                };
                let mut idx = data::lock_or_recover(&self.persistence.data_index);
                idx.merge(index);
                idx.retain_feeds(&active_feeds);
                drop(idx);
                self.rebuild_ticker_data();
                Task::none()
            }
        }
    }
}
