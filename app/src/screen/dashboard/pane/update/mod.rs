mod ai;
mod chart;
mod indicators;
mod modal;

use super::{Action, Content, Event, State};
use crate::modals::pane::Modal;
#[cfg(feature = "heatmap")]
use crate::screen::dashboard::ladder;
use crate::screen::dashboard::pane::config::ContentKind;

impl State {
    pub fn update(&mut self, msg: Event) -> Option<Action> {
        // Dismiss context menu on meaningful interactions.
        // Passive view-update messages (crosshair, bounds, side panel hover)
        // must be whitelisted so the menu stays open while the cursor moves.
        if self.context_menu.is_some()
            && !matches!(
                msg,
                Event::ContextMenuAction(_)
                    | Event::DismissContextMenu
                    | Event::AiAssistant(super::types::AiAssistantEvent::CursorMoved(_))
                    | Event::ChartInteraction(crate::chart::Message::CrosshairMoved(_))
                    | Event::ChartInteraction(crate::chart::Message::CursorLeft)
                    | Event::ChartInteraction(crate::chart::Message::BoundsChanged(_))
                    | Event::ChartInteraction(crate::chart::Message::SidePanelCrosshairMoved(_))
                    | Event::ChartInteraction(crate::chart::Message::SideSplitDragged(_, _))
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
            Event::PanelInteraction(_msg) => match &mut self.content {
                #[cfg(feature = "heatmap")]
                Content::Ladder(Some(p)) => ladder::update(p, _msg),
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
            Event::ReorderIndicator(_e) => {
                #[cfg(feature = "heatmap")]
                self.content.reorder_indicators(&_e);
            }
            #[cfg(feature = "heatmap")]
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
            Event::LevelDetailInteraction(message) => {
                return self.handle_level_detail(message);
            }
            Event::AiAssistant(ai_event) => {
                return self.handle_ai_assistant_event(ai_event);
            }
            Event::AiContextBubble(event) => {
                return self.handle_ai_context_bubble_event(event);
            }
        }
        None
    }

    fn handle_content_selected(&mut self, kind: ContentKind) -> Option<Action> {
        self.content = Content::placeholder(kind);

        // AI assistant and backtest panes don't need a ticker selection
        if !matches!(kind, ContentKind::Starter | ContentKind::AiAssistant) {
            let modal = Modal::MiniTickersList(crate::modals::pane::tickers::MiniPanel::new());

            if let Some(effect) = self.show_modal_with_focus(modal) {
                return Some(effect);
            }
        }
        None
    }

    fn show_modal_with_focus(&mut self, requested_modal: Modal) -> Option<Action> {
        let should_toggle_close = match (&self.modal, &requested_modal) {
            (Some(Modal::StreamModifier(open)), Modal::StreamModifier(req)) => {
                open.view_mode == req.view_mode
            }
            (Some(open), req) => core::mem::discriminant(open) == core::mem::discriminant(req),
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
        focus_widget_id.map(Action::FocusWidget)
    }
}
