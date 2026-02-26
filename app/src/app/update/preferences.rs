use iced::Task;

use crate::components::display::toast::{Notification, Toast};
use crate::modals::cache_management;
use crate::modals::preferences;
use crate::screen::dashboard;

use super::super::init::services;
use super::super::{DownloadMessage, Kairos, Message};

impl Kairos {
    pub(crate) fn handle_remove_notification(&mut self, index: usize) {
        if index < self.ui.notifications.len() {
            self.ui.notifications.remove(index);
        }
    }

    pub(crate) fn handle_toggle_dialog_modal(
        &mut self,
        dialog: Option<crate::components::overlay::confirm_dialog::ConfirmDialog<Message>>,
    ) {
        self.ui.confirm_dialog = dialog;
    }

    pub(crate) fn handle_reinitialize_service(
        &mut self,
        provider: crate::config::secrets::ApiProvider,
    ) -> Task<Message> {
        match provider {
            crate::config::secrets::ApiProvider::Databento => {
                log::info!("Reinitializing Databento service with new API key...");
                // Reinitialize the DataEngine with the new API key.
                return Task::perform(services::initialize_data_engine(), Message::DataEngineReady);
            }
            crate::config::secrets::ApiProvider::Rithmic => {
                log::info!("Reinitializing Rithmic service with new password...");
                if let Some(feed_id) = self.services.rithmic_feed_id {
                    return Task::done(Message::DataFeeds(
                        crate::modals::data_feeds::DataFeedsMessage::ConnectFeed(feed_id),
                    ));
                } else {
                    self.ui.push_notification(Toast::new(Notification::Info(
                        "Rithmic password saved. Configure a Rithmic feed \
                             to connect."
                            .to_string(),
                    )));
                }
            }
            crate::config::secrets::ApiProvider::OpenRouter => {
                log::info!("OpenRouter API key updated.");
            }
        }
        Task::none()
    }

    pub(crate) fn handle_theme_editor(&mut self, msg: crate::modals::theme::Message) {
        let iced_theme = crate::style::theme::theme_to_iced(&self.ui.theme);
        let action = self.modals.theme_editor.update(msg, &iced_theme);

        match action {
            Some(crate::modals::theme::Action::Exit) => {
                self.ui.sidebar.set_menu(None);
            }
            Some(crate::modals::theme::Action::UpdateTheme(iced_theme)) => {
                self.ui.theme = crate::style::theme::iced_theme_to_data(iced_theme);
                let main_window = self.main_window.id;
                if let Some(dashboard) = self.active_dashboard_mut() {
                    dashboard.invalidate_all_panes(main_window);
                }
            }
            None => {}
        }
    }

    pub(crate) fn handle_sidebar(&mut self, message: dashboard::sidebar::Message) -> Task<Message> {
        self.menu_bar.open_menu = None;

        // Sync feed manager snapshot when opening Connections or
        // DataFeeds menu
        if matches!(
            &message,
            dashboard::sidebar::Message::ToggleSidebarMenu(Some(
                crate::config::sidebar::Menu::Connections
            ))
        ) {
            let connection_manager = data::lock_or_recover(&self.connections.connection_manager);
            self.modals
                .connections_menu
                .sync_snapshot(&connection_manager);
        }

        if let dashboard::sidebar::Message::ToggleSidebarMenu(Some(
            crate::config::sidebar::Menu::DataFeeds,
        )) = &message
        {
            let connection_manager = data::lock_or_recover(&self.connections.connection_manager);
            self.modals
                .data_feeds_modal
                .sync_snapshot(&connection_manager);
        }

        // Trigger initial estimation when opening DataFeeds menu
        if let dashboard::sidebar::Message::ToggleSidebarMenu(Some(
            crate::config::sidebar::Menu::DataFeeds,
        )) = &message
            && let Some(action) = self
                .modals
                .data_management_panel
                .request_initial_estimation()
        {
            match action {
                crate::modals::download::data_management::Action::EstimateRequested {
                    ticker,
                    schema,
                    date_range,
                } => {
                    let (task, _) = self.ui.sidebar.update(message);

                    return task
                        .map(Message::Sidebar)
                        .chain(Task::done(Message::Download(
                            DownloadMessage::EstimateDataCost {
                                pane_id: super::download::GLOBAL_PANE_ID,
                                ticker,
                                schema,
                                date_range,
                            },
                        )));
                }
                crate::modals::download::data_management::Action::DownloadRequested { .. } => {
                    // Shouldn't happen on initial open
                }
            }
        }

        // Refresh available streams when opening Replay menu
        if matches!(
            &message,
            dashboard::sidebar::Message::ToggleSidebarMenu(Some(
                crate::config::sidebar::Menu::Replay
            ))
        ) {
            let connection_manager = data::lock_or_recover(&self.connections.connection_manager);
            let downloaded = data::lock_or_recover(&self.persistence.downloaded_tickers);

            let mut ticker_infos = std::collections::HashMap::new();
            for (ticker, info) in &self.persistence.tickers_info {
                ticker_infos.insert(ticker.to_string(), *info);
            }

            self.modals.replay_manager.refresh_streams(
                &connection_manager,
                &downloaded,
                &ticker_infos,
            );
        }

        let (task, action) = self.ui.sidebar.update(message);

        // Handle sidebar actions
        if let Some(action) = action {
            match action {
                dashboard::sidebar::SidebarAction::Drawing(drawing_action) => {
                    match drawing_action {
                        crate::modals::drawing::tools::Action::SelectTool(tool) => {
                            return task.map(Message::Sidebar).chain(Task::done(
                                Message::Dashboard {
                                    layout_id: None,
                                    event: Box::new(dashboard::Message::DrawingToolSelected(tool)),
                                },
                            ));
                        }
                        crate::modals::drawing::tools::Action::ToggleSnap => {
                            return task.map(Message::Sidebar).chain(Task::done(
                                Message::Dashboard {
                                    layout_id: None,
                                    event: Box::new(dashboard::Message::DrawingSnapToggled),
                                },
                            ));
                        }
                    }
                }
                dashboard::sidebar::SidebarAction::Settings(settings_action) => {
                    match settings_action {
                        preferences::Action::FlyoutToggled => {}
                        preferences::Action::OpenModal(page) => {
                            let draft = preferences::SettingsPanel::create_draft(
                                self.ui.timezone,
                                self.ui.theme.clone(),
                                self.ui.ui_scale_factor,
                                self.modals.theme_editor.custom_theme.clone(),
                            );
                            self.ui.sidebar.settings.active_modal = Some((page, draft));
                            self.ui
                                .sidebar
                                .set_menu(Some(crate::config::sidebar::Menu::Settings));
                        }
                        preferences::Action::CloseModal => {
                            self.ui.sidebar.set_menu(None);
                        }
                        preferences::Action::SaveSettings(draft) => {
                            self.ui.timezone = draft.timezone;
                            self.ui.theme = draft.theme;
                            self.ui.ui_scale_factor = draft.scale_factor;
                            let main_window = self.main_window.id;
                            if let Some(dashboard) = self.active_dashboard_mut() {
                                dashboard.invalidate_all_panes(main_window);
                            }
                            self.ui.sidebar.set_menu(None);
                        }
                        preferences::Action::OpenThemeEditor => {
                            self.ui.sidebar.settings.active_modal = None;
                            self.ui
                                .sidebar
                                .set_menu(Some(crate::config::sidebar::Menu::ThemeEditor));
                        }
                        preferences::Action::OpenDataFolder => {
                            return task
                                .map(Message::Sidebar)
                                .chain(Task::done(Message::DataFolderRequested));
                        }
                        preferences::Action::OpenDataManagement => {
                            self.modals.cache_management.reset();
                            self.modals.cache_management.loading = true;
                            self.ui
                                .sidebar
                                .set_menu(Some(crate::config::sidebar::Menu::CacheManagement));
                            return task.map(Message::Sidebar).chain(Task::perform(
                                cache_management::scan_databento_cache(),
                                |result| {
                                    Message::CacheManagement(
                                        cache_management::CacheManagementMessage::CacheScanned(
                                            result,
                                        ),
                                    )
                                },
                            ));
                        }
                    }
                }
            }
        }

        task.map(Message::Sidebar)
    }

    pub(crate) fn handle_cache_management(
        &mut self,
        msg: cache_management::CacheManagementMessage,
    ) -> Task<Message> {
        let action = self.modals.cache_management.update(msg);

        match action {
            Some(cache_management::Action::ScanCache) => {
                return Task::perform(cache_management::scan_databento_cache(), |result| {
                    Message::CacheManagement(
                        cache_management::CacheManagementMessage::CacheScanned(result),
                    )
                });
            }
            Some(cache_management::Action::DeleteEntries(paths)) => {
                return Task::perform(cache_management::delete_cache_entries(paths), |result| {
                    Message::CacheManagement(
                        cache_management::CacheManagementMessage::DeleteComplete(result),
                    )
                });
            }
            Some(cache_management::Action::ClearAll) => {
                return Task::perform(cache_management::clear_all_cache(), |result| {
                    Message::CacheManagement(
                        cache_management::CacheManagementMessage::DeleteComplete(result),
                    )
                });
            }
            Some(cache_management::Action::Close) => {
                self.modals.cache_management.reset();
                self.ui.sidebar.set_menu(None);
            }
            None => {}
        }

        Task::none()
    }
}
