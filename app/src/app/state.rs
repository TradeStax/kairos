use std::collections::HashMap;

use iced::Task;

use crate::layout::{LayoutId, configuration};
use crate::screen::dashboard::Dashboard;
use crate::window;
use data::state::WindowSpec;

use super::{Kairos, Message};

impl Kairos {
    pub fn active_dashboard(&self) -> Option<&Dashboard> {
        let active_layout = self.layout_manager.active_layout_id()?;
        self.layout_manager
            .get(active_layout.unique)
            .map(|layout| &layout.dashboard)
    }

    pub fn active_dashboard_mut(&mut self) -> Option<&mut Dashboard> {
        let active_layout = self.layout_manager.active_layout_id()?;
        let unique = active_layout.unique;
        self.layout_manager
            .get_mut(unique)
            .map(|layout| &mut layout.dashboard)
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
        if let Some(dashboard) = self.active_dashboard_mut() {
            dashboard
                .popout
                .iter_mut()
                .for_each(|(id, (_, window_spec))| {
                    if let Some(new_window_spec) = windows.get(id) {
                        *window_spec = new_window_spec.clone();
                    }
                });
        }

        let main_window_spec = windows
            .iter()
            .find(|(id, _)| **id == self.main_window.id)
            .map(|(_, spec)| spec.clone());

        // Serialize full pane trees for each layout
        let active_layout_name = self
            .layout_manager
            .active_layout_id()
            .map(|id| id.name.clone());

        let layouts_for_save: Vec<data::state::layout::Layout> = self
            .layout_manager
            .layouts
            .iter()
            .filter_map(|layout| {
                self.layout_manager.get(layout.id.unique).map(|l| {
                    data::state::layout::Layout {
                        name: layout.id.name.clone(),
                        dashboard: data::Dashboard::from(&l.dashboard),
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
            self.theme_editor
                .custom_theme
                .clone()
                .map(crate::style::theme_bridge::iced_theme_to_data),
            main_window_spec,
            self.timezone,
            self.sidebar.state.clone(),
            self.ui_scale_factor,
            data::lock_or_recover(&self.downloaded_tickers).clone(),
            self.data_feed_manager
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
        );

        // Save state using the persistence module
        let state_dir = crate::infra::platform::data_path(None);
        if let Err(e) = data::save_state(&state, state_dir.as_path(), "app-state.json") {
            log::error!("Failed to save application state: {}", e);
        } else {
            log::info!("Application state persisted successfully");
        }
    }

    #[allow(dead_code)]
    pub fn restart(&mut self) -> Task<Message> {
        let mut windows_to_close: Vec<window::Id> = self
            .active_dashboard()
            .map(|d| d.popout.keys().copied().collect())
            .unwrap_or_default();
        windows_to_close.push(self.main_window.id);

        let close_windows = Task::batch(
            windows_to_close
                .into_iter()
                .map(window::close)
                .collect::<Vec<_>>(),
        );

        let (new_state, init_task) = Kairos::new();
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
                self.data_index.clone(),
            );

            manager.insert_layout(new_layout.clone(), dashboard);
        }
    }

    pub fn refresh_edit_menu_panes(&mut self) {
        use crate::components::chrome::menu_bar::PaneInfo;

        let main_id = self.main_window.id;
        let panes: Vec<PaneInfo> = self
            .active_dashboard()
            .map(|dashboard| {
                dashboard
                    .iter_all_panes(main_id)
                    .map(|(window_id, pane, state)| {
                        let kind = state.content.to_string();
                        let label = if let Some(ti) = state.get_ticker() {
                            format!("{} - {}", kind, ti.ticker)
                        } else {
                            kind
                        };
                        PaneInfo {
                            window_id,
                            pane,
                            label,
                            is_main_window: window_id == main_id,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        self.menu_bar.set_panes(panes);
    }

    pub fn handle_layout_select(&mut self, layout: uuid::Uuid) -> Task<Message> {
        use crate::screen::dashboard;

        let active_popout_keys = self
            .active_dashboard()
            .map(|d| d.popout.keys().copied().collect::<Vec<_>>())
            .unwrap_or_default();

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
