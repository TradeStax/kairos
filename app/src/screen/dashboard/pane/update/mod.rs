mod chart;
mod modal;

use super::{Action, Content, Event, State};
use crate::{
    modals::pane::Modal,
    screen::dashboard::panel,
};
use data::ContentKind;

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
                    | Event::AiAssistant(
                        super::types::AiAssistantEvent::CursorMoved(_)
                    )
                    | Event::ChartInteraction(
                        crate::chart::Message::CrosshairMoved(_)
                    )
                    | Event::ChartInteraction(
                        crate::chart::Message::CursorLeft
                    )
                    | Event::ChartInteraction(
                        crate::chart::Message::BoundsChanged(_)
                    )
                    | Event::ChartInteraction(
                        crate::chart::Message::SidePanelCrosshairMoved(_)
                    )
                    | Event::ChartInteraction(
                        crate::chart::Message::SideSplitDragged(_, _)
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
            Event::AiAssistant(ai_event) => {
                return self.handle_ai_assistant_event(ai_event);
            }
            Event::AiContextBubble(event) => {
                return self.handle_ai_context_bubble_event(event);
            }
        }
        None
    }

    fn handle_ai_context_bubble_event(
        &mut self,
        event: super::types::AiContextBubbleEvent,
    ) -> Option<Action> {
        use super::types::AiContextBubbleEvent;

        match event {
            AiContextBubbleEvent::InputChanged(text) => {
                if let Some(ref mut bubble) = self.ai_context_bubble {
                    bubble.input_text = text;
                }
            }
            AiContextBubbleEvent::Submit => {
                let bubble = self.ai_context_bubble.take()?;
                let question = bubble.input_text.trim().to_string();
                if question.is_empty() {
                    self.ai_context_bubble = Some(bubble);
                    return None;
                }

                // Build structured chart context (sent as system context)
                let s = &bubble.range_summary;
                let mut context = format!(
                    "Ticker: {} | Timeframe: {}\n\
                     Range: {} \u{2192} {}\n\
                     Price: {} \u{2013} {} | \
                     Candles: {} | Vol: {} | Delta: {}",
                    s.ticker,
                    s.timeframe,
                    s.time_start_fmt,
                    s.time_end_fmt,
                    s.price_low,
                    s.price_high,
                    s.candle_count,
                    s.total_volume,
                    s.net_delta,
                );

                if !s.candle_ohlcv_lines.is_empty() {
                    context.push_str("\n\n");
                    for line in &s.candle_ohlcv_lines {
                        context.push_str(line);
                        context.push('\n');
                    }
                }

                // Delete the AiContext drawing (ephemeral)
                if let Some(chart) =
                    self.content.drawing_chart_mut()
                {
                    chart.drawings_mut().delete(bubble.drawing_id);
                    chart.invalidate_all_drawing_caches();
                }

                let source_pane_id = self.unique_id();
                return Some(Action::AiContextQuery {
                    source_pane_id,
                    context,
                    question,
                });
            }
            AiContextBubbleEvent::Dismiss => {
                self.ai_context_bubble = None;
            }
        }
        None
    }

    fn handle_ai_assistant_event(
        &mut self,
        event: super::types::AiAssistantEvent,
    ) -> Option<Action> {
        use super::types::AiAssistantEvent;

        let Content::AiAssistant(state) = &mut self.content else {
            return None;
        };

        match event {
            AiAssistantEvent::InputChanged(text) => {
                state.input_text = text;
            }
            AiAssistantEvent::SendMessage => {
                let text = state.input_text.trim().to_string();
                if text.is_empty() || state.is_streaming {
                    return None;
                }
                let pane_id = self.unique_id();
                return Some(Action::AiRequest {
                    pane_id,
                    user_message: text,
                });
            }
            AiAssistantEvent::ToggleSettings => {
                state.show_settings = !state.show_settings;
            }
            AiAssistantEvent::ModelChanged(model) => {
                state.model = model.clone();
                return Some(Action::AiPreferencesChanged {
                    model,
                    temperature: state.temperature,
                    max_tokens: state.max_tokens,
                });
            }
            AiAssistantEvent::TemperatureChanged(t) => {
                state.temperature = t;
                return Some(Action::AiPreferencesChanged {
                    model: state.model.clone(),
                    temperature: t,
                    max_tokens: state.max_tokens,
                });
            }
            AiAssistantEvent::MaxTokensChanged(n) => {
                state.max_tokens = n;
                return Some(Action::AiPreferencesChanged {
                    model: state.model.clone(),
                    temperature: state.temperature,
                    max_tokens: n,
                });
            }
            AiAssistantEvent::StopStreaming => {
                state.stop_streaming();
            }
            AiAssistantEvent::RetryLastMessage => {
                // Pop the error system notice
                if matches!(
                    state.messages.last().map(|m| &m.kind),
                    Some(data::domain::assistant::ChatMessageKind::SystemNotice { .. })
                ) {
                    state.messages.pop();
                }
                // Find the last user message
                let last_user = state
                    .messages
                    .iter()
                    .rev()
                    .find_map(|m| {
                        if let data::domain::assistant::ChatMessageKind::User { text } = &m.kind {
                            Some(text.clone())
                        } else {
                            None
                        }
                    });
                if let Some(user_msg) = last_user {
                    let pane_id = self.unique_id();
                    return Some(Action::AiRequest {
                        pane_id,
                        user_message: user_msg,
                    });
                }
            }
            AiAssistantEvent::ClearHistory => {
                state.clear_history();
            }
            AiAssistantEvent::ApiKeyInputChanged(s) => {
                state.api_key_input = s;
            }
            AiAssistantEvent::DismissApiKeyModal => {
                state.show_api_key_modal = false;
                state.api_key_input.clear();
            }
            AiAssistantEvent::SaveApiKey => {
                let key = state.api_key_input.trim().to_string();
                state.show_api_key_modal = false;
                state.api_key_input.clear();
                if !key.is_empty() {
                    return Some(Action::SaveAiApiKey(key));
                }
            }
            AiAssistantEvent::OpenUrl(url) => {
                let _ = open::that_detached(&url);
            }
            AiAssistantEvent::CursorMoved(p) => {
                state.last_cursor_position = p;
            }
            AiAssistantEvent::MessageRightClicked(idx) => {
                self.context_menu =
                    Some(super::types::ContextMenuKind::AiMessage {
                        position: state.last_cursor_position,
                        message_index: idx,
                    });
            }
            AiAssistantEvent::ToggleThinking(idx) => {
                if !state.expanded_thinking.remove(&idx) {
                    state.expanded_thinking.insert(idx);
                }
            }
        }
        None
    }

    fn handle_content_selected(&mut self, kind: ContentKind) -> Option<Action> {
        self.content = Content::placeholder(kind);

        // AI assistant and backtest panes don't need a ticker selection
        if !matches!(
            kind,
            ContentKind::Starter
                | ContentKind::AiAssistant
        ) {
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
        use crate::modals::pane::indicator::IndicatorManagerModal;

        let content_kind = self.content.kind();
        let active_study_ids = match &self.content {
            Content::Candlestick { study_ids, .. }
            | Content::Profile { study_ids, .. } => study_ids.clone(),
            _ => vec![],
        };
        let studies: Vec<Box<dyn study::Study>> = match &self.content {
            Content::Candlestick { chart: Some(c), .. } => {
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

    /// Open the indicator manager with a specific study pre-selected.
    pub(super) fn open_indicator_manager_for_study(
        &mut self,
        study_index: usize,
    ) {
        use crate::modals::pane::indicator::{
            IndicatorManagerModal, SelectedIndicator,
        };

        // Resolve the study ID from the index
        let study_id = match &self.content {
            Content::Candlestick { chart: Some(c), .. } => {
                c.studies().get(study_index).map(|s| s.id().to_string())
            }
            Content::Profile { chart: Some(c), .. } => {
                c.studies().get(study_index).map(|s| s.id().to_string())
            }
            _ => None,
        };

        let Some(study_id) = study_id else {
            // Index out of bounds — fall back to normal manager
            self.open_indicator_manager();
            return;
        };

        let content_kind = self.content.kind();
        let active_study_ids = match &self.content {
            Content::Candlestick { study_ids, .. }
            | Content::Profile { study_ids, .. } => study_ids.clone(),
            _ => vec![],
        };
        let studies: Vec<Box<dyn study::Study>> = match &self.content {
            Content::Candlestick { chart: Some(c), .. } => {
                c.studies().iter().map(|s| s.clone_study()).collect()
            }
            Content::Profile { chart: Some(c), .. } => {
                c.studies().iter().map(|s| s.clone_study()).collect()
            }
            _ => vec![],
        };

        let mut manager = IndicatorManagerModal::new(
            content_kind,
            active_study_ids,
            studies,
        );
        manager.selected =
            Some(SelectedIndicator::Study(study_id));
        self.modal = Some(Modal::IndicatorManager(manager));
    }

    fn show_modal_with_focus(
        &mut self,
        requested_modal: Modal,
    ) -> Option<Action> {
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
        focus_widget_id.map(Action::FocusWidget)
    }
}
