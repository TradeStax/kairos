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
                // Update connection status in the ConnectionManager
                self.connections.with_connection_manager(|cm| {
                    cm.set_status(feed_id, data::ConnectionStatus::Connected);
                });
                let cm_arc = self.connections.connection_manager.clone();
                let cm = data::lock_or_recover(&cm_arc);
                self.sync_feed_snapshots(&cm);
                Task::none()
            }
            data::DataEvent::Disconnected { feed_id, reason } => {
                log::info!("DataEngine: disconnected feed {} — {}", feed_id, reason);
                self.connections.with_connection_manager(|cm| {
                    cm.set_status(feed_id, data::ConnectionStatus::Disconnected);
                });
                let cm_arc = self.connections.connection_manager.clone();
                let cm = data::lock_or_recover(&cm_arc);
                self.sync_feed_snapshots(&cm);
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
                self.connections.with_connection_manager(|cm| {
                    cm.set_status(
                        feed_id,
                        data::ConnectionStatus::Reconnecting { attempt },
                    );
                });
                let cm_arc = self.connections.connection_manager.clone();
                let cm = data::lock_or_recover(&cm_arc);
                self.sync_feed_snapshots(&cm);
                Task::none()
            }
            data::DataEvent::TradeReceived { .. } => self.handle_dashboard(
                None,
                crate::screen::dashboard::Message::LiveData(event),
            ),
            #[cfg(feature = "heatmap")]
            data::DataEvent::DepthReceived { .. } => self.handle_dashboard(
                None,
                crate::screen::dashboard::Message::LiveData(event),
            ),
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
            data::DataEvent::DataIndexUpdated(index) => {
                // Merge updated index into our shared DataIndex.
                data::lock_or_recover(&self.persistence.data_index).merge(index);
                self.rebuild_ticker_data();
                Task::none()
            }
        }
    }
}
