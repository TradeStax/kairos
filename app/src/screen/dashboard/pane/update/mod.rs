mod chart;
mod modal;

use super::{Content, Effect, Event, State};
use crate::{
    modals::pane::Modal,
    screen::dashboard::panel,
};
use data::ContentKind;

impl State {
    pub fn update(&mut self, msg: Event) -> Option<Effect> {
        // Dismiss context menu on meaningful interactions
        if self.context_menu.is_some()
            && !matches!(
                msg,
                Event::ContextMenuAction(_)
                    | Event::DismissContextMenu
                    | Event::ChartInteraction(
                        crate::chart::Message::CrosshairMoved(_)
                    )
                    | Event::ChartInteraction(
                        crate::chart::Message::CursorLeft
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
            Event::IndicatorManagerInteraction(message) => {
                return self.handle_indicator_manager(message);
            }
        }
        None
    }

    fn handle_content_selected(&mut self, kind: ContentKind) -> Option<Effect> {
        self.content = Content::placeholder(kind);

        if !matches!(kind, ContentKind::Starter) {
            let modal = Modal::MiniTickersList(
                crate::modals::pane::tickers::MiniPanel::new(),
            );

            if let Some(effect) = self.show_modal_with_focus(modal) {
                return Some(effect);
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
