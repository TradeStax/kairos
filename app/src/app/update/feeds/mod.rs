mod databento;
mod rithmic;

use iced::Task;

use super::super::{Kairos, Message};

impl Kairos {
    pub(crate) fn handle_data_feeds(
        &mut self,
        msg: crate::modals::data_feeds::DataFeedsMessage,
    ) -> Task<Message> {
        let mut feed_manager = data::lock_or_recover(&self.connections.connection_manager);

        let actions = self.modals.data_feeds_modal.update(msg, &mut feed_manager);

        if actions.is_empty() {
            // Sync snapshot after any update
            self.modals.data_feeds_modal.sync_snapshot(&feed_manager);
            return Task::none();
        }

        // Drop feed_manager before calling self methods
        drop(feed_manager);

        let mut tasks: Vec<Task<Message>> = Vec::new();
        for action in actions {
            tasks.push(self.dispatch_data_feeds_action(action));
        }

        match tasks.len() {
            0 => Task::none(),
            1 => tasks.remove(0),
            _ => Task::batch(tasks),
        }
    }

    fn dispatch_data_feeds_action(
        &mut self,
        action: crate::modals::data_feeds::Action,
    ) -> Task<Message> {
        match action {
            crate::modals::data_feeds::Action::ConnectFeed(feed_id) => {
                let dm = self.connections.connection_manager.clone();
                let feed_manager = data::lock_or_recover(&dm);
                self.connect_feed(feed_id, feed_manager)
            }
            crate::modals::data_feeds::Action::DisconnectFeed(feed_id) => {
                let dm = self.connections.connection_manager.clone();
                let feed_manager = data::lock_or_recover(&dm);
                self.disconnect_feed(feed_id, feed_manager)
            }
            crate::modals::data_feeds::Action::FeedsUpdated => self.handle_feeds_updated(),
            crate::modals::data_feeds::Action::OpenHistoricalDownload => {
                let has_key = self
                    .secrets
                    .has_api_key(crate::config::secrets::ApiProvider::Databento);
                if has_key {
                    self.modals.historical_download_modal =
                        Some(crate::modals::download::HistoricalDownloadModal::new());
                } else {
                    self.modals.api_key_setup_modal =
                        Some(crate::modals::download::ApiKeySetupModal::new());
                }
                Task::none()
            }
            crate::modals::data_feeds::Action::LoadPreview(feed_id, info) => {
                self.load_feed_preview(feed_id, info)
            }
            crate::modals::data_feeds::Action::Close => {
                self.ui.sidebar.set_menu(None);
                Task::none()
            }
            crate::modals::data_feeds::Action::SaveApiKey { provider, key } => {
                if let Err(e) = self.secrets.set_api_key(provider, &key) {
                    log::warn!("Failed to save API key for {:?}: {}", provider, e);
                }
                self.handle_feeds_updated()
            }
            crate::modals::data_feeds::Action::SaveFeedPassword { feed_id, password } => {
                if let Err(e) = self
                    .secrets
                    .set_feed_password(&feed_id.to_string(), &password)
                {
                    log::warn!("Failed to save password for feed {}: {}", feed_id, e);
                }
                self.handle_feeds_updated()
            }
            crate::modals::data_feeds::Action::ProbeSystemNames(server) => Task::perform(
                async move {
                    let handle = tokio::runtime::Handle::current();
                    tokio::task::spawn_blocking(move || {
                        handle.block_on(data::probe_system_names(server.url()))
                    })
                    .await
                    .map_err(|e| format!("Task join error: {}", e))
                    .and_then(|r| r.map_err(|e| e.to_string()))
                },
                move |result| Message::RithmicSystemNames { server, result },
            ),
        }
    }

    fn connect_feed(
        &mut self,
        feed_id: data::FeedId,
        feed_manager: std::sync::MutexGuard<'_, data::ConnectionManager>,
    ) -> Task<Message> {
        log::info!("Connect feed requested: {}", feed_id);

        let Some(feed) = feed_manager.get(feed_id) else {
            return Task::none();
        };

        match feed.provider {
            data::ConnectionProvider::Databento => {
                self.connect_databento_feed(feed_id, feed_manager)
            }
            data::ConnectionProvider::Rithmic => self.connect_rithmic_feed(feed_id, feed_manager),
        }
    }

    fn disconnect_feed(
        &mut self,
        feed_id: data::FeedId,
        mut feed_manager: std::sync::MutexGuard<'_, data::ConnectionManager>,
    ) -> Task<Message> {
        log::info!("Disconnect feed requested: {}", feed_id);

        let provider = feed_manager.get(feed_id).map(|f| f.provider);

        // Check if this is the active Rithmic feed
        if self.services.rithmic_feed_id == Some(feed_id) {
            return self.disconnect_rithmic_feed(feed_id, feed_manager);
        }

        feed_manager.set_status(feed_id, data::ConnectionStatus::Disconnected);

        if provider == Some(data::ConnectionProvider::Databento) {
            self.disconnect_databento_feed(feed_id, feed_manager)
        } else {
            self.sync_feed_snapshots(&feed_manager);
            Task::none()
        }
    }

    fn load_feed_preview(
        &mut self,
        _feed_id: data::FeedId,
        _info: data::HistoricalDatasetInfo,
    ) -> Task<Message> {
        // Preview loading is not yet supported by the DataEngine.
        // The old market_data_service.get_trades_for_preview() was removed.
        Task::none()
    }

    pub(crate) fn handle_connections_menu(
        &mut self,
        msg: crate::modals::connections::ConnectionsMenuMessage,
    ) -> Task<Message> {
        if let Some(action) = self.modals.connections_menu.update(msg) {
            match action {
                crate::modals::connections::Action::ConnectFeed(feed_id) => {
                    self.ui.sidebar.set_menu(None);
                    return Task::done(Message::DataFeeds(
                        crate::modals::data_feeds::DataFeedsMessage::ConnectFeed(feed_id),
                    ));
                }
                crate::modals::connections::Action::DisconnectFeed(feed_id) => {
                    self.ui.sidebar.set_menu(None);
                    return Task::done(Message::DataFeeds(
                        crate::modals::data_feeds::DataFeedsMessage::DisconnectFeed(feed_id),
                    ));
                }
                crate::modals::connections::Action::Close => {
                    self.ui.sidebar.set_menu(None);
                    return Task::none();
                }
                crate::modals::connections::Action::OpenManageDialog => {
                    self.ui
                        .sidebar
                        .set_menu(Some(crate::config::sidebar::Menu::DataFeeds));
                    let feed_manager = data::lock_or_recover(&self.connections.connection_manager);
                    self.modals.data_feeds_modal.sync_snapshot(&feed_manager);
                }
            }
        }
        Task::none()
    }

    pub(super) fn handle_feeds_updated(&mut self) -> Task<Message> {
        log::info!("Data feeds updated, persisting to disk");
        let dm = self.connections.connection_manager.clone();
        let feed_manager = data::lock_or_recover(&dm);
        self.sync_feed_snapshots(&feed_manager);
        self.collect_and_persist_state()
    }
}
