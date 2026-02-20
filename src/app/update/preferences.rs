use iced::Task;

use crate::components::display::toast::{Notification, Toast};
use crate::screen::dashboard;

use super::super::{DownloadMessage, Kairos, Message, services};

impl Kairos {
    pub(crate) fn handle_theme_selected(&mut self, theme: data::Theme) {
        self.theme = theme;
    }

    pub(crate) fn handle_scale_factor_changed(&mut self, value: data::ScaleFactor) {
        self.ui_scale_factor = value;
    }

    pub(crate) fn handle_set_timezone(&mut self, tz: data::UserTimezone) {
        self.timezone = tz;
    }

    pub(crate) fn handle_remove_notification(&mut self, index: usize) {
        if index < self.notifications.len() {
            self.notifications.remove(index);
        }
    }

    pub(crate) fn handle_toggle_dialog_modal(
        &mut self,
        dialog: Option<crate::components::overlay::confirm_dialog::ConfirmDialog<Message>>,
    ) {
        self.confirm_dialog = dialog;
    }

    pub(crate) fn handle_reinitialize_service(
        &mut self,
        provider: data::config::secrets::ApiProvider,
    ) -> Task<Message> {
        match provider {
            data::config::secrets::ApiProvider::Databento => {
                log::info!("Reinitializing Databento service with new API key...");
                if let Some(result) = services::initialize_market_data_service() {
                    self.market_data_service = Some(result.service.clone());
                    self.replay_engine = services::create_replay_engine(Some(&result));
                    self.notifications.push(Toast::new(Notification::Info(
                        "Databento service initialized".to_string(),
                    )));
                } else {
                    self.notifications.push(Toast::error(
                        "Failed to initialize Databento service".to_string(),
                    ));
                }
            }
            data::config::secrets::ApiProvider::Massive => {
                #[cfg(feature = "options")]
                {
                    log::info!("Reinitializing Massive service with new API key...");
                    let (options_service, _) = services::initialize_options_services();
                    self.options_service = options_service;
                    if self.options_service.is_some() {
                        self.notifications.push(Toast::new(Notification::Info(
                            "Options service initialized".to_string(),
                        )));
                    }
                }
                #[cfg(not(feature = "options"))]
                {
                    log::info!("Options feature not enabled, skipping Massive service init");
                }
            }
            data::config::secrets::ApiProvider::Rithmic => {
                log::info!("Reinitializing Rithmic service with new password...");
                if let Some(feed_id) = self.rithmic_feed_id {
                    return Task::done(Message::DataFeeds(
                        crate::modals::data_feeds::DataFeedsMessage::ConnectFeed(feed_id),
                    ));
                } else {
                    self.notifications.push(Toast::new(Notification::Info(
                        "Rithmic password saved. Configure a Rithmic feed \
                         to connect."
                            .to_string(),
                    )));
                }
            }
        }
        Task::none()
    }

    pub(crate) fn handle_layouts(
        &mut self,
        message: crate::modals::layout::Message,
    ) -> Task<Message> {
        let action = self.layout_manager.update(message);

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

    pub(crate) fn handle_theme_editor(&mut self, msg: crate::modals::theme::Message) {
        let iced_theme = crate::style::theme_bridge::theme_to_iced(&self.theme);
        let action = self.theme_editor.update(msg, &iced_theme);

        match action {
            Some(crate::modals::theme::Action::Exit) => {
                self.sidebar.set_menu(Some(data::sidebar::Menu::Settings));
            }
            Some(crate::modals::theme::Action::UpdateTheme(iced_theme)) => {
                self.theme = crate::style::theme_bridge::iced_theme_to_data(iced_theme);
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

        // Handle date range preset change - update all dashboards
        if let dashboard::sidebar::Message::SetDateRangePreset(preset) = &message {
            self.layout_manager.set_date_range_preset(*preset);
        }

        // Sync feed manager snapshot when opening Connections or
        // DataFeeds menu
        if matches!(
            &message,
            dashboard::sidebar::Message::ToggleSidebarMenu(Some(data::sidebar::Menu::Connections))
        ) {
            let feed_manager = self
                .data_feed_manager
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            self.connections_menu.sync_snapshot(&feed_manager);
        }

        if let dashboard::sidebar::Message::ToggleSidebarMenu(Some(
            data::sidebar::Menu::DataFeeds,
        )) = &message
        {
            let feed_manager = self
                .data_feed_manager
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            self.data_feeds_modal.sync_snapshot(&feed_manager);
        }

        // Trigger initial estimation when opening DataFeeds menu
        if let dashboard::sidebar::Message::ToggleSidebarMenu(Some(data::sidebar::Menu::DataFeeds)) =
            &message
            && let Some(action) = self.data_management_panel.request_initial_estimation()
        {
            match action {
                crate::modals::download::data_management::Action::EstimateRequested {
                    ticker,
                    schema,
                    date_range,
                } => {
                    let (task, _) = self.sidebar.update(message);

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
            dashboard::sidebar::Message::ToggleSidebarMenu(Some(data::sidebar::Menu::Replay))
        ) {
            let feed_manager = self
                .data_feed_manager
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let downloaded = self
                .downloaded_tickers
                .lock()
                .unwrap_or_else(|e| e.into_inner());

            let mut ticker_infos = std::collections::HashMap::new();
            for (ticker, info) in &self.tickers_info {
                ticker_infos.insert(ticker.to_string(), *info);
            }

            self.replay_manager
                .refresh_streams(&feed_manager, &downloaded, &ticker_infos);
        }

        let (task, drawing_action) = self.sidebar.update(message);

        // Handle drawing tool actions from the sidebar
        if let Some(action) = drawing_action {
            match action {
                crate::modals::drawing_tools::Action::SelectTool(tool) => {
                    return task
                        .map(Message::Sidebar)
                        .chain(Task::done(Message::Dashboard {
                            layout_id: None,
                            event: dashboard::Message::DrawingToolSelected(tool),
                        }));
                }
                crate::modals::drawing_tools::Action::ToggleSnap => {
                    return task
                        .map(Message::Sidebar)
                        .chain(Task::done(Message::Dashboard {
                            layout_id: None,
                            event: dashboard::Message::DrawingSnapToggled,
                        }));
                }
            }
        }

        task.map(Message::Sidebar)
    }
}
