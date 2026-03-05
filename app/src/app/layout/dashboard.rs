//! Layout Dashboard Operations — runtime layout management and persistence.
//!
//! Contains both runtime operations (layout switching, cloning, popout management)
//! AND the single save path for application state via [`Kairos::save_state_to_disk`].
//!
//! ## Counterpart
//! The load path is [`crate::persistence::load_saved_state_without_registry`]
//! in `app/src/persistence/loading.rs`.

use std::collections::HashMap;

use iced::Task;

use crate::persistence::WindowSpec;
use crate::persistence::{LayoutId, configuration};
use crate::screen::dashboard::Dashboard;
use crate::window;

use super::super::{Kairos, Message};

impl Kairos {
    pub fn active_dashboard(&self) -> Option<&Dashboard> {
        let active_layout = self.persistence.layout_manager.active_layout_id()?;
        self.persistence
            .layout_manager
            .get(active_layout.unique)
            .map(|layout| &layout.dashboard)
    }

    pub fn active_dashboard_mut(&mut self) -> Option<&mut Dashboard> {
        let active_layout = self.persistence.layout_manager.active_layout_id()?;
        let unique = active_layout.unique;
        self.persistence
            .layout_manager
            .get_mut(unique)
            .map(|layout| &mut layout.dashboard)
    }

    pub fn load_layout(
        &mut self,
        layout_uid: uuid::Uuid,
        main_window: window::Id,
    ) -> Task<Message> {
        match self
            .persistence
            .layout_manager
            .set_active_layout(layout_uid)
        {
            Ok(layout) => {
                layout
                    .dashboard
                    .load_layout(main_window)
                    .map(move |msg| Message::Dashboard {
                        layout_id: Some(layout_uid),
                        event: Box::new(msg),
                    })
            }
            Err(err) => {
                log::error!("Failed to set active layout: {}", err);
                Task::none()
            }
        }
    }

    /// Collect live window specs asynchronously, then persist state to disk.
    ///
    /// Unlike [`save_state_to_disk`] (which requires the caller to already have
    /// window specs), this returns a [`Task`] that first queries the window
    /// system for current positions/sizes and then triggers a
    /// [`Message::PersistState`] to write to disk.
    pub fn collect_and_persist_state(&self) -> Task<Message> {
        let mut popout_keys: Vec<window::Id> = self
            .active_dashboard()
            .map(|d| d.popout.keys().copied().collect())
            .unwrap_or_default();
        popout_keys.push(self.main_window.id);

        window::collect_window_specs(popout_keys, Message::PersistState)
    }

    /// Build a serializable snapshot of the current application state.
    ///
    /// This updates popout window specs from the provided map, then
    /// clones all relevant state into an [`AppState`] suitable for
    /// serialization. No I/O is performed.
    fn prepare_state_snapshot(
        &mut self,
        windows: &HashMap<window::Id, WindowSpec>,
    ) -> crate::persistence::AppState {
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

        let active_layout_name = self
            .persistence
            .layout_manager
            .active_layout_id()
            .map(|id| id.name.clone());

        let layouts_for_save: Vec<crate::persistence::layout::Layout> = self
            .persistence
            .layout_manager
            .layouts
            .iter()
            .filter_map(|layout| {
                self.persistence
                    .layout_manager
                    .get(layout.id.unique)
                    .map(|l| crate::persistence::layout::Layout {
                        name: layout.id.name.clone(),
                        dashboard: crate::persistence::Dashboard::from(&l.dashboard),
                    })
            })
            .collect();

        let layout_manager_clone = crate::persistence::layout::LayoutManager {
            layouts: layouts_for_save,
            active_layout: active_layout_name,
        };

        let mut state = crate::persistence::AppState::from_parts(
            layout_manager_clone,
            self.ui.theme.clone(),
            self.modals
                .theme_editor
                .custom_theme
                .clone()
                .map(crate::style::theme::iced_theme_to_data),
            main_window_spec,
            self.ui.timezone,
            self.ui.sidebar.state.clone(),
            self.ui.ui_scale_factor,
            data::lock_or_recover(&self.persistence.downloaded_tickers).clone(),
            data::lock_or_recover(&self.connections.connection_manager).clone(),
        );
        state.ai_preferences = self.ui.ai_preferences.clone();
        state.auto_update = self.persistence.auto_update_prefs.clone();
        state
    }

    /// Persist state to disk asynchronously via [`Task::perform`].
    ///
    /// Use this for periodic saves so the UI thread is not blocked by
    /// filesystem I/O.
    pub fn save_state_to_disk_async(
        &mut self,
        windows: &HashMap<window::Id, WindowSpec>,
    ) -> Task<Message> {
        let state = self.prepare_state_snapshot(windows);
        let state_dir = crate::infra::platform::data_path(None);
        Task::perform(
            async move {
                if let Err(e) =
                    crate::persistence::save_state(&state, state_dir.as_path(), "app-state.json")
                {
                    log::error!("Failed to save application state: {}", e);
                } else {
                    log::info!("Application state persisted successfully");
                }
            },
            |()| Message::Noop,
        )
    }

    /// Persist state to disk synchronously (blocking).
    ///
    /// Only use this during application shutdown where the event loop
    /// is about to exit and async tasks cannot complete.
    pub fn save_state_to_disk(&mut self, windows: &HashMap<window::Id, WindowSpec>) {
        let state = self.prepare_state_snapshot(windows);
        let state_dir = crate::infra::platform::data_path(None);
        if let Err(e) =
            crate::persistence::save_state(&state, state_dir.as_path(), "app-state.json")
        {
            log::error!("Failed to save application state: {}", e);
        } else {
            log::info!("Application state persisted successfully");
        }
    }

    pub fn handle_layout_clone(&mut self, id: uuid::Uuid) {
        let manager = &mut self.persistence.layout_manager;

        let source_data = manager.get(id).map(|layout| {
            (
                layout.id.name.clone(),
                layout.id.unique,
                crate::persistence::Dashboard::from(&layout.dashboard),
            )
        });

        if let Some((name, _old_id, ser_dashboard)) = source_data {
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
                None,
                self.persistence.data_index.clone(),
            );

            self.persistence
                .layout_manager
                .insert_layout(new_layout.clone(), dashboard);
        }
    }

    pub fn refresh_edit_menu_panes(&mut self) {
        use crate::app::update::menu_bar::PaneInfo;

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

    /// Clear all transient modal and UI overlay state.
    ///
    /// Called during layout switches to prevent stale modals, menus,
    /// and dialogs from persisting across layouts.
    fn clear_transient_state(&mut self) {
        // Sidebar menu and flyouts
        self.ui.sidebar.set_menu(None);
        self.ui.sidebar.drawing_tools.expanded_group = None;
        self.ui.sidebar.settings.flyout_expanded = false;
        self.ui.sidebar.settings.active_modal = None;

        // Menu bar
        self.menu_bar.open_menu = None;
        self.menu_bar.show_save_dialog = false;
        self.menu_bar.show_submenu = false;
        self.menu_bar.hovered_pane_index = None;

        // Confirm dialog
        self.ui.confirm_dialog = None;

        // Download and API key modals
        self.modals.historical_download_modal = None;
        self.modals.historical_download_id = None;
        self.modals.api_key_setup_modal = None;

        // Backtest modals
        self.modals.backtest.show_backtest_modal = false;
        self.modals.backtest.show_backtest_manager = false;
    }

    pub fn handle_layout_select(&mut self, layout: uuid::Uuid) -> Task<Message> {
        use crate::screen::dashboard;

        // Clear transient modal/UI state so it doesn't leak across layouts
        self.clear_transient_state();

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
            .persistence
            .layout_manager
            .active_layout_id()
            .as_ref()
            .map(|layout| layout.unique);

        window::collect_window_specs(active_popout_keys, dashboard::Message::SavePopoutSpecs)
            .map(move |msg| Message::Dashboard {
                layout_id: old_layout_id,
                event: Box::new(msg),
            })
            .chain(window_tasks)
            .chain(self.load_layout(layout, self.main_window.id))
    }
}
