use std::collections::HashMap;

use iced::Task;

use crate::components::display::toast::Toast;
use crate::screen::dashboard;
use crate::window;

use crate::components::chrome::menu_bar;

use super::super::{ChartMessage, DownloadMessage, Kairos, Message};

impl Kairos {
    pub(crate) fn handle_menu_bar(&mut self, msg: menu_bar::Message) -> Task<Message> {
        // Pre-fill save dialog name when opening
        if matches!(msg, menu_bar::Message::SaveLayout) {
            self.menu_bar.save_layout_name = self.layout_manager.generate_unique_layout_name();
        }

        let action = self.menu_bar.update(msg);

        match action {
            menu_bar::Action::CloseWindow => {
                return self.handle_window_close(self.main_window.id);
            }
            menu_bar::Action::SaveLayout(name) => {
                if let Some(active_id) = self.layout_manager.active_layout_id().map(|l| l.unique) {
                    self.handle_layout_clone(active_id);
                    // Rename the newly created layout (last in the list)
                    if let Some(new_layout) = self.layout_manager.layouts.last() {
                        let new_id = new_layout.id.unique;
                        let unique_name = self.layout_manager.ensure_unique_name(&name, new_id);
                        self.layout_manager.layouts.last_mut().unwrap().id.name = unique_name;
                    }
                }
            }
            menu_bar::Action::LoadLayout(id) => {
                return self.handle_layout_select(id);
            }
            menu_bar::Action::None => {}
        }
        Task::none()
    }

    pub(crate) fn handle_tick(&mut self, now: std::time::Instant) -> Task<Message> {
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

    pub(crate) fn handle_window_event(&mut self, event: window::Event) -> Task<Message> {
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

                window::collect_window_specs(active_windows, Message::ExitRequested)
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

    pub(crate) fn handle_go_back(&mut self) {
        let main_window = self.main_window.id;

        if self.confirm_dialog.is_some() {
            self.confirm_dialog = None;
        } else if self.menu_bar.show_save_dialog {
            self.menu_bar.show_save_dialog = false;
            self.menu_bar.save_layout_name.clear();
        } else if self.menu_bar.open_menu.is_some() {
            self.menu_bar.open_menu = None;
            self.menu_bar.show_submenu = false;
        } else if self.historical_download_modal.is_some() {
            self.historical_download_modal = None;
            self.historical_download_id = None;
        } else if self.sidebar.active_menu().is_some() {
            self.sidebar.set_menu(None);
        } else if let Some(dashboard) = self.active_dashboard_mut() {
            if dashboard.go_back(main_window) {
            } else if dashboard.focus.is_some() {
                dashboard.focus = None;
            }
        }
    }

    pub(crate) fn handle_dashboard(
        &mut self,
        id: Option<uuid::Uuid>,
        msg: dashboard::Message,
    ) -> Task<Message> {
        let Some(active_layout) = self.layout_manager.active_layout_id() else {
            log::error!("No active layout to handle dashboard message");
            return Task::none();
        };

        let main_window = self.main_window;
        let layout_id = id.unwrap_or(active_layout.unique);

        if let Some(dashboard) = self.layout_manager.mut_dashboard(layout_id) {
            let (main_task, event) = dashboard.update(msg, &main_window, &layout_id);

            let additional_task = match event {
                Some(dashboard::Event::LoadChart {
                    pane_id,
                    config,
                    ticker_info,
                }) => Task::done(Message::Chart(ChartMessage::LoadChartData {
                    layout_id,
                    pane_id,
                    config,
                    ticker_info,
                })),
                Some(dashboard::Event::Notification(toast)) => {
                    self.notifications.push(toast);
                    Task::none()
                }
                Some(dashboard::Event::EstimateDataCost {
                    pane_id,
                    ticker,
                    schema,
                    date_range,
                }) => Task::done(Message::Download(DownloadMessage::EstimateDataCost {
                    pane_id,
                    ticker,
                    schema,
                    date_range,
                })),
                Some(dashboard::Event::DownloadData {
                    pane_id,
                    ticker,
                    schema,
                    date_range,
                }) => Task::done(Message::Download(DownloadMessage::DownloadData {
                    pane_id,
                    ticker,
                    schema,
                    date_range,
                })),
                Some(dashboard::Event::PaneClosed { .. }) => Task::none(),
                Some(dashboard::Event::DrawingToolChanged(tool)) => {
                    self.sidebar.drawing_tools.set_active_tool(tool);
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
            self.notifications
                .push(Toast::error(format!("Failed to open data folder: {err}")));
        }
    }

    // ── Window control handlers (custom title bar) ─────────────────────

    pub(crate) fn handle_window_drag(&self, id: window::Id) -> Task<Message> {
        iced::window::drag(id)
    }

    pub(crate) fn handle_window_minimize(&self, id: window::Id) -> Task<Message> {
        iced::window::minimize(id, true)
    }

    pub(crate) fn handle_window_toggle_maximize(&mut self, id: window::Id) -> Task<Message> {
        if id == self.main_window.id {
            self.main_window.is_maximized = !self.main_window.is_maximized;
        }
        iced::window::toggle_maximize(id)
    }

    pub(crate) fn handle_window_close(&mut self, id: window::Id) -> Task<Message> {
        self.handle_window_event(window::Event::CloseRequested(id))
    }
}
