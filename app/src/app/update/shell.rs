use std::collections::HashMap;

use iced::Task;

use crate::components::display::toast::Toast;
use crate::screen::dashboard;
use crate::infra::window;

use super::super::{ChartMessage, DownloadMessage, Kairos, Message};

impl Kairos {
    pub(crate) fn handle_tick(
        &mut self,
        now: std::time::Instant,
    ) -> Task<Message> {
        let main_window_id = self.main_window.id;

        let Some(dashboard) = self.active_dashboard_mut() else {
            return Task::none();
        };
        dashboard
            .tick(now, main_window_id)
            .map(move |msg| Message::Dashboard {
                layout_id: None,
                event: msg,
            })
    }

    pub(crate) fn handle_window_event(
        &mut self,
        event: window::Event,
    ) -> Task<Message> {
        match event {
            window::Event::CloseRequested(window) => {
                let main_window = self.main_window.id;
                let Some(dashboard) = self.active_dashboard_mut() else {
                    return window::close(window);
                };

                if window != main_window {
                    dashboard.popout.remove(&window);
                    return window::close(window);
                }

                let mut active_windows = dashboard
                    .popout
                    .keys()
                    .copied()
                    .collect::<Vec<window::Id>>();
                active_windows.push(main_window);

                window::collect_window_specs(
                    active_windows,
                    Message::ExitRequested,
                )
            }
        }
    }

    pub(crate) fn handle_exit_requested(
        &mut self,
        windows: HashMap<window::Id, data::state::WindowSpec>,
    ) -> Task<Message> {
        self.save_state_to_disk(&windows);
        iced::exit()
    }

    /// Dismiss the front-most visible overlay in priority order.
    ///
    /// Escape dismissal priority (highest to lowest):
    /// 1. Confirm dialog (blocks all interaction behind it)
    /// 2. Menu bar save-layout dialog
    /// 3. Menu bar open dropdown
    /// 4. Historical download modal
    /// 5. Sidebar settings active modal (e.g. API key setup)
    /// 6. Sidebar settings flyout (expanded panel)
    /// 7. Sidebar active menu (ticker list, replay, etc.)
    /// 8. Dashboard pane go-back (drawing tool cancel, etc.)
    /// 9. Dashboard pane focus (keyboard focus ring)
    ///
    /// To add a new dismissible overlay: insert it at the correct priority
    /// level in this chain and ensure it does not appear in two places.
    pub(crate) fn handle_go_back(&mut self) {
        let main_window = self.main_window.id;

        if self.ui.confirm_dialog.is_some() {
            // 1. Confirm dialog
            self.ui.confirm_dialog = None;
        } else if self.menu_bar.show_save_dialog {
            // 2. Menu bar save-layout dialog
            self.menu_bar.show_save_dialog = false;
            self.menu_bar.save_layout_name.clear();
        } else if self.menu_bar.open_menu.is_some() {
            // 3. Menu bar dropdown
            self.menu_bar.open_menu = None;
            self.menu_bar.show_submenu = false;
        } else if self.modals.historical_download_modal.is_some() {
            // 4. Historical download modal
            self.modals.historical_download_modal = None;
            self.modals.historical_download_id = None;
        } else if self.ui.sidebar.settings.active_modal.is_some() {
            // 5. Sidebar settings modal
            self.ui.sidebar.settings.active_modal = None;
            self.ui.sidebar.set_menu(None);
        } else if self.ui.sidebar.settings.flyout_expanded {
            // 6. Sidebar settings flyout
            self.ui.sidebar.settings.flyout_expanded = false;
        } else if self.ui.sidebar.active_menu().is_some() {
            // 7. Sidebar active menu
            self.ui.sidebar.set_menu(None);
        } else if let Some(dashboard) = self.active_dashboard_mut() {
            // 8 & 9. Dashboard pane go-back / focus
            if !dashboard.go_back(main_window) && dashboard.focus.is_some() {
                dashboard.focus = None;
            }
        }
    }

    pub(crate) fn handle_dashboard(
        &mut self,
        id: Option<uuid::Uuid>,
        msg: dashboard::Message,
    ) -> Task<Message> {
        let Some(active_layout) = self.persistence.layout_manager.active_layout_id()
        else {
            log::error!("No active layout to handle dashboard message");
            return Task::none();
        };

        let main_window = self.main_window;
        let layout_id = id.unwrap_or(active_layout.unique);

        if let Some(dashboard) =
            self.persistence.layout_manager.mut_dashboard(layout_id)
        {
            let (main_task, event) =
                dashboard.update(msg, &main_window);

            let additional_task = match event {
                Some(dashboard::Event::LoadChart {
                    pane_id,
                    config,
                    ticker_info,
                }) => Task::done(Message::Chart(
                    ChartMessage::LoadChartData {
                        layout_id,
                        pane_id,
                        config,
                        ticker_info,
                    },
                )),
                Some(dashboard::Event::Notification(toast)) => {
                    self.ui.notifications.push(toast);
                    Task::none()
                }
                Some(dashboard::Event::EstimateDataCost {
                    pane_id,
                    ticker,
                    schema,
                    date_range,
                }) => Task::done(Message::Download(
                    DownloadMessage::EstimateDataCost {
                        pane_id,
                        ticker,
                        schema,
                        date_range,
                    },
                )),
                Some(dashboard::Event::DownloadData {
                    pane_id,
                    ticker,
                    schema,
                    date_range,
                }) => Task::done(Message::Download(
                    DownloadMessage::DownloadData {
                        pane_id,
                        ticker,
                        schema,
                        date_range,
                    },
                )),
                Some(dashboard::Event::PaneClosed { .. }) => Task::none(),
                Some(dashboard::Event::DrawingToolChanged(tool)) => {
                    self.ui.sidebar.drawing_tools.set_active_tool(tool);
                    Task::none()
                }
                Some(dashboard::Event::AiRequest {
                    pane_id,
                    user_message,
                }) => self.handle_ai_request(pane_id, user_message),
                Some(dashboard::Event::SaveAiApiKey(key)) => {
                    match self.secrets.set_api_key(
                        data::config::secrets::ApiProvider::OpenRouter,
                        &key,
                    ) {
                        Ok(()) => self.ui.notifications.push(
                            Toast::success("OpenRouter API key saved."),
                        ),
                        Err(e) => self.ui.notifications.push(
                            Toast::error(format!("Failed to save key: {}", e)),
                        ),
                    }
                    Task::none()
                }
                Some(dashboard::Event::AiContextQuery {
                    source_pane_id: _,
                    context,
                    question,
                }) => self.handle_ai_context_query(context, question),
                Some(dashboard::Event::AiPreferencesChanged {
                    model,
                    temperature,
                    max_tokens,
                }) => {
                    self.ui.ai_preferences =
                        data::AiPreferences {
                            model,
                            temperature,
                            max_tokens,
                        };
                    Task::none()
                }
                None => Task::none(),
            };

            return main_task
                .map(move |msg| Message::Dashboard {
                    layout_id: Some(layout_id),
                    event: msg,
                })
                .chain(additional_task);
        }
        Task::none()
    }

    pub(crate) fn handle_data_folder_requested(&mut self) {
        if let Err(err) = crate::infra::platform::open_data_folder() {
            self.ui.notifications.push(Toast::error(format!(
                "Failed to open data folder: {err}"
            )));
        }
    }

    // ── Window control handlers (custom title bar) ─────────────────────

    pub(crate) fn handle_window_drag(&self, id: window::Id) -> Task<Message> {
        iced::window::drag(id)
    }

    pub(crate) fn handle_window_minimize(
        &self,
        id: window::Id,
    ) -> Task<Message> {
        iced::window::minimize(id, true)
    }

    pub(crate) fn handle_window_toggle_maximize(
        &mut self,
        id: window::Id,
    ) -> Task<Message> {
        if id == self.main_window.id {
            self.main_window.is_maximized = !self.main_window.is_maximized;
        }
        iced::window::toggle_maximize(id)
    }

    pub(crate) fn handle_window_close(
        &mut self,
        id: window::Id,
    ) -> Task<Message> {
        self.handle_window_event(window::Event::CloseRequested(id))
    }
}
