use std::collections::HashMap;

use iced::Task;

use crate::layout::{LayoutId, configuration};
use crate::screen::dashboard::Dashboard;
use crate::window;
use data::state::WindowSpec;

use super::{Flowsurface, Message};

impl Flowsurface {
    pub fn active_dashboard(&self) -> &Dashboard {
        let active_layout = self
            .layout_manager
            .active_layout_id()
            .expect("No active layout");
        self.layout_manager
            .get(active_layout.unique)
            .map(|layout| &layout.dashboard)
            .expect("No active dashboard")
    }

    pub fn active_dashboard_mut(&mut self) -> &mut Dashboard {
        let active_layout = self
            .layout_manager
            .active_layout_id()
            .expect("No active layout");
        self.layout_manager
            .get_mut(active_layout.unique)
            .map(|layout| &mut layout.dashboard)
            .expect("No active dashboard")
    }

    pub fn load_layout(
        &mut self,
        layout_uid: uuid::Uuid,
        main_window: window::Id,
    ) -> Task<Message> {
        match self.layout_manager.set_active_layout(layout_uid) {
            Ok(layout) => {
                layout
                    .dashboard
                    .load_layout(main_window)
                    .map(move |msg| Message::Dashboard {
                        layout_id: Some(layout_uid),
                        event: msg,
                    })
            }
            Err(err) => {
                log::error!("Failed to set active layout: {}", err);
                Task::none()
            }
        }
    }

    pub fn save_state_to_disk(&mut self, windows: &HashMap<window::Id, WindowSpec>) {
        self.active_dashboard_mut()
            .popout
            .iter_mut()
            .for_each(|(id, (_, window_spec))| {
                if let Some(new_window_spec) = windows.get(id) {
                    *window_spec = new_window_spec.clone();
                }
            });

        let main_window_spec = windows
            .iter()
            .find(|(id, _)| **id == self.main_window.id)
            .map(|(_, spec)| spec.clone());

        // Clone the layout manager data for serialization
        let active_layout_name = self
            .layout_manager
            .active_layout_id()
            .map(|id| id.name.clone());

        let layouts_for_save: Vec<data::state::app::Layout> = self
            .layout_manager
            .layouts
            .iter()
            .filter_map(|layout| {
                self.layout_manager.get(layout.id.unique).map(|_l| {
                    data::state::app::Layout {
                        name: Some(layout.id.name.clone()),
                        // Intentionally empty: pane layout is managed by the
                        // dashboard's PaneGrid state and restored separately.
                        panes: vec![],
                    }
                })
            })
            .collect();

        let layout_manager_clone = data::state::app::LayoutManager {
            layouts: layouts_for_save,
            active_layout: active_layout_name,
        };

        let state = data::AppState::from_parts(
            layout_manager_clone,
            self.theme.clone(),
            self.theme_editor.custom_theme.clone().map(data::Theme),
            main_window_spec,
            self.timezone,
            self.sidebar.state.clone(),
            self.ui_scale_factor,
            self.downloaded_tickers.lock().unwrap().clone(),
            self.data_feed_manager
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
        );

        // Save state using the persistence module
        if let Err(e) = data::save_state(&state, "app-state.json") {
            log::error!("Failed to save application state: {}", e);
        } else {
            log::info!("Application state persisted successfully");
        }
    }

    #[allow(dead_code)]
    pub fn restart(&mut self) -> Task<Message> {
        let mut windows_to_close: Vec<window::Id> =
            self.active_dashboard().popout.keys().copied().collect();
        windows_to_close.push(self.main_window.id);

        let close_windows = Task::batch(
            windows_to_close
                .into_iter()
                .map(window::close)
                .collect::<Vec<_>>(),
        );

        let (new_state, init_task) = Flowsurface::new();
        *self = new_state;

        close_windows.chain(init_task)
    }

    pub fn handle_layout_clone(&mut self, id: uuid::Uuid) {
        let manager = &mut self.layout_manager;

        let source_data = manager.get(id).map(|layout| {
            (
                layout.id.name.clone(),
                layout.id.unique,
                data::Dashboard::from(&layout.dashboard),
            )
        });

        if let Some((name, old_id, ser_dashboard)) = source_data {
            let new_uid = uuid::Uuid::new_v4();
            let new_layout = LayoutId {
                unique: new_uid,
                name: manager.ensure_unique_name(&name, new_uid),
            };

            let mut popout_windows = Vec::new();

            for (pane, window_spec) in &ser_dashboard.popout {
                let configuration = configuration(pane.clone());
                popout_windows.push((configuration, window_spec.clone()));
            }

            let dashboard = Dashboard::from_config(
                configuration(ser_dashboard.pane.clone()),
                popout_windows,
                old_id,
                self.market_data_service.clone(),
                self.downloaded_tickers.clone(),
                self.sidebar.date_range_preset(),
            );

            manager.insert_layout(new_layout.clone(), dashboard);
        }
    }

    pub fn handle_layout_select(&mut self, layout: uuid::Uuid) -> Task<Message> {
        use crate::screen::dashboard;

        let active_popout_keys = self
            .active_dashboard()
            .popout
            .keys()
            .copied()
            .collect::<Vec<_>>();

        let window_tasks = Task::batch(
            active_popout_keys
                .iter()
                .map(|&popout_id| window::close::<window::Id>(popout_id))
                .collect::<Vec<_>>(),
        )
        .discard();

        let old_layout_id = self
            .layout_manager
            .active_layout_id()
            .as_ref()
            .map(|layout| layout.unique);

        window::collect_window_specs(active_popout_keys, dashboard::Message::SavePopoutSpecs)
            .map(move |msg| Message::Dashboard {
                layout_id: old_layout_id,
                event: msg,
            })
            .chain(window_tasks)
            .chain(self.load_layout(layout, self.main_window.id))
    }
}
