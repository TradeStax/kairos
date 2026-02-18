use iced::Task;

use crate::component::display::toast::{Notification, Toast};
use crate::screen::dashboard;
use crate::screen::dashboard::tickers_table;

use super::super::{DownloadMessage, Flowsurface, Message, services};

impl Flowsurface {
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
        dialog: Option<crate::screen::ConfirmDialog<Message>>,
    ) {
        self.confirm_dialog = dialog;
    }

    pub(crate) fn handle_reinitialize_service(
        &mut self,
        provider: data::ApiProvider,
    ) -> Task<Message> {
        match provider {
            data::ApiProvider::Databento => {
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
            data::ApiProvider::Massive => {
                log::info!("Reinitializing Massive service with new API key...");
                let (options_service, _) = services::initialize_options_services();
                self.options_service = options_service;
                if self.options_service.is_some() {
                    self.notifications.push(Toast::new(Notification::Info(
                        "Options service initialized".to_string(),
                    )));
                }
            }
            data::ApiProvider::Rithmic => {
                log::info!("Reinitializing Rithmic service with new password...");
                if let Some(feed_id) = self.rithmic_feed_id {
                    return Task::done(Message::DataFeeds(
                        crate::modal::pane::data_feeds::DataFeedsMessage::ConnectFeed(feed_id),
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

    pub(crate) fn handle_audio_stream(&mut self, message: crate::modal::audio::Message) {
        self.audio_stream.update(message);
    }

    pub(crate) fn handle_layouts(
        &mut self,
        message: crate::modal::layout_manager::Message,
    ) -> Task<Message> {
        let action = self.layout_manager.update(message);

        match action {
            Some(crate::modal::layout_manager::Action::Select(layout)) => {
                return self.handle_layout_select(layout);
            }
            Some(crate::modal::layout_manager::Action::Clone(id)) => {
                self.handle_layout_clone(id);
            }
            None => {}
        }
        Task::none()
    }

    pub(crate) fn handle_theme_editor(&mut self, msg: crate::modal::theme_editor::Message) {
        let action = self.theme_editor.update(msg, &self.theme.clone().into());

        match action {
            Some(crate::modal::theme_editor::Action::Exit) => {
                self.sidebar.set_menu(Some(data::sidebar::Menu::Settings));
            }
            Some(crate::modal::theme_editor::Action::UpdateTheme(theme)) => {
                self.theme = data::Theme(theme);
                let main_window = self.main_window.id;
                self.active_dashboard_mut()
                    .invalidate_all_panes(main_window);
            }
            None => {}
        }
    }

    pub(crate) fn handle_sidebar(&mut self, message: dashboard::sidebar::Message) -> Task<Message> {
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
                crate::modal::pane::download::data_management::Action::EstimateRequested {
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
                crate::modal::pane::download::data_management::Action::DownloadRequested {
                    ..
                } => {
                    // Shouldn't happen on initial open
                }
            }
        }

        let (task, drawing_action) = self.sidebar.update(message);

        // Handle drawing tool actions from the sidebar
        if let Some(action) = drawing_action {
            match action {
                crate::modal::drawing_tools::Action::SelectTool(tool) => {
                    return task
                        .map(Message::Sidebar)
                        .chain(Task::done(Message::Dashboard {
                            layout_id: None,
                            event: dashboard::Message::DrawingToolSelected(tool),
                        }));
                }
                crate::modal::drawing_tools::Action::ToggleSnap => {
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

    pub(crate) fn handle_tickers_table(&mut self, msg: tickers_table::Message) -> Task<Message> {
        let action = self.tickers_table.update(msg);

        match action {
            Some(tickers_table::Action::ErrorOccurred(err)) => {
                self.notifications.push(Toast::error(err.to_string()));
            }
            // TickerSelected is handled by pane modals directly
            Some(tickers_table::Action::TickerSelected(_, _)) => {}
            None => {}
        }
        Task::none()
    }
}
