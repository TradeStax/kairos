mod chart;
mod modal;

use super::{
    Content, Effect, Event, State,
    content::{build_script_list, generate_unique_name, script_template},
};
use crate::{
    components::display::toast::{Notification, Toast},
    modals::pane::Modal,
    screen::dashboard::panel,
};
use data::{ContentKind, LoadingStatus, VisualConfig};
use std::path::PathBuf;

impl State {
    pub fn update(&mut self, msg: Event) -> Option<Effect> {
        // Dismiss context menu on meaningful interactions
        if self.context_menu.is_some()
            && !matches!(
                msg,
                Event::ContextMenuAction(_)
                    | Event::DismissContextMenu
                    | Event::ChartInteraction(
                        crate::chart::Message::CrosshairMoved
                    )
                    | Event::ChartInteraction(
                        crate::chart::Message::BoundsChanged(_)
                    )
            )
        {
            self.context_menu = None;
        }

        match msg {
            Event::ShowModal(requested_modal) => {
                return self.show_modal_with_focus(requested_modal);
            }
            Event::HideModal => {
                self.modal = None;
            }
            Event::ContentSelected(kind) => {
                return self.handle_content_selected(kind);
            }
            Event::ChartInteraction(msg) => {
                return self.handle_chart_interaction(msg);
            }
            Event::PanelInteraction(msg) => match &mut self.content {
                Content::Ladder(Some(p)) => panel::update(p, msg),
                Content::TimeAndSales(Some(p)) => panel::update(p, msg),
                _ => {}
            },
            Event::ToggleStudy(study_id) => {
                self.content.toggle_study(&study_id);
            }
            Event::DeleteNotification(idx) => {
                if idx < self.notifications.len() {
                    self.notifications.remove(idx);
                }
            }
            Event::ReorderIndicator(e) => {
                self.content.reorder_indicators(&e);
            }
            Event::StudyConfigurator(study_msg) => {
                self.handle_study_configurator(study_msg);
            }
            Event::StreamModifierChanged(message) => {
                return self.handle_stream_modifier(message);
            }
            Event::ComparisonChartInteraction(message) => {
                return self.handle_comparison_chart(message);
            }
            Event::MiniTickersListInteraction(message) => {
                return self.handle_mini_tickers_list(message);
            }
            Event::DataManagementInteraction(message) => {
                return self.handle_data_management(message);
            }
            Event::DismissContextMenu => {
                self.context_menu = None;
            }
            Event::ContextMenuAction(action) => {
                return self.handle_context_menu_action(action);
            }
            Event::DrawingPropertiesChanged(message) => {
                return self.handle_drawing_properties_modal(message);
            }
            Event::OpenIndicatorManager => {
                self.open_indicator_manager();
            }
            Event::EditorInteraction(msg) => {
                if let Content::ScriptEditor { editor, .. } = &mut self.content
                {
                    let _ = editor.update(&msg);
                }
            }
            Event::ScriptSelected(name) => {
                self.handle_script_selected(name);
            }
            Event::NewScript => {
                self.handle_new_script();
            }
            Event::SaveScript => {
                return self.handle_save_script();
            }
            Event::IndicatorManagerInteraction(message) => {
                return self.handle_indicator_manager(message);
            }
        }
        None
    }

    fn handle_content_selected(&mut self, kind: ContentKind) -> Option<Effect> {
        if matches!(kind, ContentKind::ScriptEditor) {
            let loader = script::ScriptLoader::new();
            let script_list = build_script_list(&loader);

            // Restore last-edited script from settings
            let script_path = self
                .settings
                .visual_config
                .as_ref()
                .and_then(|vc| vc.clone().script_editor())
                .and_then(|cfg| cfg.script_path.map(PathBuf::from));

            let editor = if let Some(ref path) = script_path {
                if let Ok(content) = std::fs::read_to_string(path) {
                    iced_code_editor::CodeEditor::new(&content, "javascript")
                        .with_line_numbers_enabled(true)
                } else {
                    iced_code_editor::CodeEditor::new("", "javascript")
                        .with_line_numbers_enabled(true)
                }
            } else {
                iced_code_editor::CodeEditor::new("", "javascript")
                    .with_line_numbers_enabled(true)
            };

            self.content = Content::ScriptEditor {
                editor,
                script_path,
                script_list,
            };
            self.loading_status = LoadingStatus::Ready;
        } else {
            self.content = Content::placeholder(kind);

            if !matches!(kind, ContentKind::Starter) {
                let modal = Modal::MiniTickersList(
                    crate::modals::pane::tickers::MiniPanel::new(),
                );

                if let Some(effect) = self.show_modal_with_focus(modal) {
                    return Some(effect);
                }
            }
        }
        None
    }

    fn handle_script_selected(&mut self, name: String) {
        if let Content::ScriptEditor {
            editor,
            script_path,
            script_list,
            ..
        } = &mut self.content
        {
            if let Some(entry) = script_list.iter().find(|e| e.name == name) {
                let entry_path = entry.path.clone();
                if let Ok(content) = std::fs::read_to_string(&entry_path) {
                    *editor =
                        iced_code_editor::CodeEditor::new(&content, "javascript")
                            .with_line_numbers_enabled(true);
                    *script_path = Some(entry_path.clone());
                    // Persist selection
                    self.settings.visual_config =
                        Some(VisualConfig::ScriptEditor(
                            data::ScriptEditorConfig {
                                script_path: Some(
                                    entry_path
                                        .to_string_lossy()
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        ));
                }
            }
        }
    }

    fn handle_new_script(&mut self) {
        let loader = script::ScriptLoader::new();
        let _ = loader.ensure_user_dir();
        let name = generate_unique_name(loader.user_dir());
        let path = loader.user_dir().join(format!("{}.js", name));
        let template = script_template(&name);
        if std::fs::write(&path, &template).is_ok() {
            if let Content::ScriptEditor {
                editor,
                script_path,
                script_list,
                ..
            } = &mut self.content
            {
                *editor =
                    iced_code_editor::CodeEditor::new(&template, "javascript")
                        .with_line_numbers_enabled(true);
                *script_path = Some(path.clone());
                *script_list = build_script_list(&loader);
                self.settings.visual_config =
                    Some(VisualConfig::ScriptEditor(
                        data::ScriptEditorConfig {
                            script_path: Some(
                                path.to_string_lossy().to_string(),
                            ),
                            ..Default::default()
                        },
                    ));
            }
        }
    }

    fn handle_save_script(&mut self) -> Option<Effect> {
        if let Content::ScriptEditor {
            editor,
            script_path: Some(path),
            ..
        } = &mut self.content
        {
            let content = editor.content();
            if std::fs::write(path, &content).is_ok() {
                editor.mark_saved();
                self.notifications
                    .push(Toast::new(Notification::Info("Script saved".into())));
                return Some(Effect::ReloadScripts);
            }
        }
        None
    }

    pub(super) fn open_indicator_manager(&mut self) {
        use crate::modals::pane::indicator_manager::IndicatorManagerModal;

        let content_kind = self.content.kind();
        let active_study_ids = match &self.content {
            Content::Kline { study_ids, .. }
            | Content::Profile { study_ids, .. } => study_ids.clone(),
            _ => vec![],
        };
        let studies: Vec<Box<dyn study::Study>> = match &self.content {
            Content::Kline { chart: Some(c), .. } => {
                c.studies().iter().map(|s| s.clone_study()).collect()
            }
            Content::Profile { chart: Some(c), .. } => {
                c.studies().iter().map(|s| s.clone_study()).collect()
            }
            _ => vec![],
        };

        let manager = IndicatorManagerModal::new(
            content_kind,
            active_study_ids,
            studies,
        );
        self.modal = Some(Modal::IndicatorManager(manager));
    }

    fn show_modal_with_focus(
        &mut self,
        requested_modal: Modal,
    ) -> Option<Effect> {
        let should_toggle_close = match (&self.modal, &requested_modal) {
            (Some(Modal::StreamModifier(open)), Modal::StreamModifier(req)) => {
                open.view_mode == req.view_mode
            }
            (Some(open), req) => {
                core::mem::discriminant(open) == core::mem::discriminant(req)
            }
            _ => false,
        };

        if should_toggle_close {
            self.modal = None;
            return None;
        }

        let focus_widget_id = match &requested_modal {
            Modal::MiniTickersList(m) => Some(m.search_box_id.clone()),
            _ => None,
        };

        self.modal = Some(requested_modal);
        focus_widget_id.map(Effect::FocusWidget)
    }
}
