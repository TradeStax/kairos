use super::super::{Action, Content, State};

impl State {
    pub(in super::super) fn handle_ai_context_bubble_event(
        &mut self,
        event: super::super::types::AiContextBubbleEvent,
    ) -> Option<Action> {
        use super::super::types::AiContextBubbleEvent;

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
                if let Some(chart) = self.content.drawing_chart_mut() {
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

    pub(in super::super) fn handle_ai_assistant_event(
        &mut self,
        event: super::super::types::AiAssistantEvent,
    ) -> Option<Action> {
        use super::super::types::AiAssistantEvent;

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
                    Some(ai::ChatMessageKind::SystemNotice { .. })
                ) {
                    state.messages.pop();
                }
                // Find the last user message
                let last_user = state.messages.iter().rev().find_map(|m| {
                    if let ai::ChatMessageKind::User { text } = &m.kind {
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
                self.context_menu = Some(super::super::context_menu::ContextMenuKind::AiMessage {
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
}
