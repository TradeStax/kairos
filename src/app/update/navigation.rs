use std::collections::HashMap;

use iced::Task;

use crate::screen::dashboard;
use crate::widget::toast::Toast;
use crate::window;

use super::super::{ChartMessage, DownloadMessage, Flowsurface, Message};

impl Flowsurface {
    pub(crate) fn handle_tick(&mut self, now: std::time::Instant) -> Task<Message> {
        let main_window_id = self.main_window.id;

        self.active_dashboard_mut()
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
                let dashboard = self.active_dashboard_mut();

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
        windows: HashMap<window::Id, data::layout::WindowSpec>,
    ) -> Task<Message> {
        self.save_state_to_disk(&windows);
        iced::exit()
    }

    pub(crate) fn handle_restart_requested(
        &mut self,
        windows: HashMap<window::Id, data::layout::WindowSpec>,
    ) -> Task<Message> {
        self.save_state_to_disk(&windows);
        self.restart()
    }

    pub(crate) fn handle_go_back(&mut self) {
        let main_window = self.main_window.id;

        if self.confirm_dialog.is_some() {
            self.confirm_dialog = None;
        } else if self.historical_download_modal.is_some() {
            self.historical_download_modal = None;
            self.historical_download_id = None;
        } else if self.sidebar.active_menu().is_some() {
            self.sidebar.set_menu(None);
        } else {
            let dashboard = self.active_dashboard_mut();

            if dashboard.go_back(main_window) {
                return;
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
        if let Err(err) = data::open_data_folder() {
            self.notifications
                .push(Toast::error(format!("Failed to open data folder: {err}")));
        }
    }
}
