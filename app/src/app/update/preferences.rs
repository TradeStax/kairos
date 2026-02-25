use iced::Task;

use crate::components::display::toast::{Notification, Toast};
use crate::modals::preferences;
use crate::screen::dashboard;

use super::super::{DownloadMessage, Kairos, Message};
use super::super::init::services;

impl Kairos {
    pub(crate) fn handle_theme_selected(&mut self, theme: data::Theme) {
        self.ui.theme = theme;
    }

    pub(crate) fn handle_scale_factor_changed(
        &mut self,
        value: data::ScaleFactor,
    ) {
        self.ui.ui_scale_factor = value;
    }

    pub(crate) fn handle_set_timezone(&mut self, tz: data::UserTimezone) {
        self.ui.timezone = tz;
    }

    pub(crate) fn handle_remove_notification(&mut self, index: usize) {
        if index < self.ui.notifications.len() {
            self.ui.notifications.remove(index);
        }
    }

    pub(crate) fn handle_toggle_dialog_modal(
        &mut self,
        dialog: Option<
            crate::components::overlay::confirm_dialog::ConfirmDialog<
                Message,
            >,
        >,
    ) {
        self.ui.confirm_dialog = dialog;
    }

    pub(crate) fn handle_reinitialize_service(
        &mut self,
        provider: data::config::secrets::ApiProvider,
    ) -> Task<Message> {
        match provider {
            data::config::secrets::ApiProvider::Databento => {
                log::info!(
                    "Reinitializing Databento service with new API key..."
                );
                // Service init is now async — delegate to Task::perform
                // and handle result via ServicesReady
                return Task::perform(
                    services::initialize_all_services(),
                    Message::ServicesReady,
                );
            }
            data::config::secrets::ApiProvider::Massive => {
                #[cfg(feature = "options")]
                {
                    log::info!("Reinitializing Massive service with new API key...");
                    // Options init is now async — delegate to
                    // Task::perform and handle via ServicesReady
                    return Task::perform(
                        services::initialize_all_services(),
                        Message::ServicesReady,
                    );
                }
                #[cfg(not(feature = "options"))]
                {
                    log::info!("Options feature not enabled, skipping Massive service init");
                }
            }
            data::config::secrets::ApiProvider::Rithmic => {
                log::info!(
                    "Reinitializing Rithmic service with new password..."
                );
                if let Some(feed_id) = self.connections.rithmic_feed_id {
                    return Task::done(Message::DataFeeds(
                        crate::modals::data_feeds::DataFeedsMessage::ConnectFeed(
                            feed_id,
                        ),
                    ));
                } else {
                    self.ui.notifications.push(Toast::new(
                        Notification::Info(
                            "Rithmic password saved. Configure a Rithmic feed \
                             to connect."
                                .to_string(),
                        ),
                    ));
                }
            }
            data::config::secrets::ApiProvider::OpenRouter => {
                log::info!("OpenRouter API key updated.");
            }
        }
        Task::none()
    }

    pub(crate) fn handle_layouts(
        &mut self,
        message: crate::modals::layout::Message,
    ) -> Task<Message> {
        let action = self.persistence.layout_manager.update(message);

        match action {
            Some(crate::modals::layout::Action::Select(layout)) => {
                return self.handle_layout_select(layout);
            }
            Some(crate::modals::layout::Action::Clone(id)) => {
                self.handle_layout_clone(id);
            }
            None => {}
        }
        Task::none()
    }

    pub(crate) fn handle_theme_editor(
        &mut self,
        msg: crate::modals::theme::Message,
    ) {
        let iced_theme =
            crate::style::theme::theme_to_iced(&self.ui.theme);
        let action = self.modals.theme_editor.update(msg, &iced_theme);

        match action {
            Some(crate::modals::theme::Action::Exit) => {
                self.ui.sidebar.set_menu(None);
            }
            Some(crate::modals::theme::Action::UpdateTheme(
                iced_theme,
            )) => {
                self.ui.theme =
                    crate::style::theme::iced_theme_to_data(
                        iced_theme,
                    );
                let main_window = self.main_window.id;
                if let Some(dashboard) = self.active_dashboard_mut() {
                    dashboard.invalidate_all_panes(main_window);
                }
            }
            None => {}
        }
    }

    pub(crate) fn handle_sidebar(
        &mut self,
        message: dashboard::sidebar::Message,
    ) -> Task<Message> {
        self.menu_bar.open_menu = None;

        // Sync feed manager snapshot when opening Connections or
        // DataFeeds menu
        if matches!(
            &message,
            dashboard::sidebar::Message::ToggleSidebarMenu(Some(
                data::sidebar::Menu::Connections
            ))
        ) {
            let feed_manager =
                data::lock_or_recover(&self.connections.data_feed_manager);
            self.modals.connections_menu.sync_snapshot(&feed_manager);
        }

        if let dashboard::sidebar::Message::ToggleSidebarMenu(
            Some(data::sidebar::Menu::DataFeeds),
        ) = &message
        {
            let feed_manager =
                data::lock_or_recover(&self.connections.data_feed_manager);
            self.modals.data_feeds_modal.sync_snapshot(&feed_manager);
        }

        // Trigger initial estimation when opening DataFeeds menu
        if let dashboard::sidebar::Message::ToggleSidebarMenu(
            Some(data::sidebar::Menu::DataFeeds),
        ) = &message
            && let Some(action) =
                self.modals.data_management_panel.request_initial_estimation()
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
                                pane_id: uuid::Uuid::nil(),
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
                data::sidebar::Menu::Replay
            ))
        ) {
            let feed_manager =
                data::lock_or_recover(&self.connections.data_feed_manager);
            let downloaded =
                data::lock_or_recover(&self.persistence.downloaded_tickers);

            let mut ticker_infos =
                std::collections::HashMap::new();
            for (ticker, info) in &self.persistence.tickers_info {
                ticker_infos.insert(ticker.to_string(), *info);
            }

            self.modals.replay_manager.refresh_streams(
                &feed_manager,
                &downloaded,
                &ticker_infos,
            );
        }

        let (task, action) = self.ui.sidebar.update(message);

        // Handle sidebar actions
        if let Some(action) = action {
            match action {
                dashboard::sidebar::SidebarAction::Drawing(
                    drawing_action,
                ) => match drawing_action {
                    crate::modals::drawing::tools::Action::SelectTool(
                        tool,
                    ) => {
                        return task
                            .map(Message::Sidebar)
                            .chain(Task::done(Message::Dashboard {
                                layout_id: None,
                                event:
                                    dashboard::Message::DrawingToolSelected(
                                        tool,
                                    ),
                            }));
                    }
                    crate::modals::drawing::tools::Action::ToggleSnap => {
                        return task
                            .map(Message::Sidebar)
                            .chain(Task::done(Message::Dashboard {
                                layout_id: None,
                                event:
                                    dashboard::Message::DrawingSnapToggled,
                            }));
                    }
                },
                dashboard::sidebar::SidebarAction::Settings(
                    settings_action,
                ) => {
                    match settings_action {
                        preferences::Action::FlyoutToggled(_) => {}
                        preferences::Action::OpenModal(page) => {
                            let draft =
                                preferences::SettingsPanel::create_draft(
                                    self.ui.timezone,
                                    self.ui.sidebar.date_range_preset(),
                                    self.ui.theme.clone(),
                                    self.ui.ui_scale_factor,
                                    self.modals.theme_editor
                                        .custom_theme
                                        .clone(),
                                );
                            self.ui.sidebar.settings.active_modal =
                                Some((page, draft));
                            self.ui.sidebar.set_menu(Some(
                                data::sidebar::Menu::Settings,
                            ));
                        }
                        preferences::Action::CloseModal => {
                            self.ui.sidebar.set_menu(None);
                        }
                        preferences::Action::SaveSettings(draft) => {
                            self.ui.timezone = draft.timezone;
                            self.ui.theme = draft.theme;
                            self.ui.ui_scale_factor = draft.scale_factor;
                            self.ui.sidebar.state.date_range_preset =
                                draft.date_range_preset;
                            let main_window = self.main_window.id;
                            if let Some(dashboard) =
                                self.active_dashboard_mut()
                            {
                                dashboard.invalidate_all_panes(
                                    main_window,
                                );
                            }
                            self.ui.sidebar.set_menu(None);
                        }
                        preferences::Action::OpenThemeEditor => {
                            self.ui.sidebar.settings.active_modal = None;
                            self.ui.sidebar.set_menu(Some(
                                data::sidebar::Menu::ThemeEditor,
                            ));
                        }
                        preferences::Action::OpenDataFolder => {
                            return task
                                .map(Message::Sidebar)
                                .chain(Task::done(
                                    Message::DataFolderRequested,
                                ));
                        }
                    }
                }
            }
        }

        task.map(Message::Sidebar)
    }
}
