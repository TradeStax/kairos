mod snapshot;
mod streaming;
mod system_prompt;
pub(crate) mod tools;

pub(crate) use tools::drawings::AiDrawingAction;

use system_prompt::SYSTEM_PROMPT;

use iced::{Task, widget::scrollable};

use super::super::{Kairos, Message};
use crate::app::core::globals::{self, AiStreamEventClone};
use data::domain::assistant::{ChatMessageKind, DisplayMessage};

impl Kairos {
    /// Handle an AI stream event routed from the global staging buffer.
    /// Routes to the correct pane and returns a scroll-to-bottom task.
    pub(crate) fn handle_ai_stream_event(
        &mut self,
        event: AiStreamEventClone,
    ) -> Task<Message> {
        // Handle drawing actions at the Kairos level (needs cross-pane access)
        if let AiStreamEventClone::DrawingAction {
            conversation_id,
            action,
        } = &event
        {
            return self.apply_ai_drawing_action(
                *conversation_id,
                action.clone(),
            );
        }

        let conversation_id = match &event {
            AiStreamEventClone::Delta { conversation_id, .. }
            | AiStreamEventClone::ToolCallStarted {
                conversation_id, ..
            }
            | AiStreamEventClone::ToolCallResult {
                conversation_id, ..
            }
            | AiStreamEventClone::TextSegmentComplete {
                conversation_id,
                ..
            }
            | AiStreamEventClone::ApiHistorySync {
                conversation_id, ..
            }
            | AiStreamEventClone::Complete {
                conversation_id, ..
            }
            | AiStreamEventClone::Error {
                conversation_id, ..
            }
            | AiStreamEventClone::ApiKeyMissing {
                conversation_id, ..
            }
            | AiStreamEventClone::DrawingAction {
                conversation_id, ..
            } => *conversation_id,
        };

        // Route to matching pane (main dashboard or popouts)
        if let Some(dash) = self.active_dashboard_mut() {
            for (_, state) in dash.panes.iter_mut() {
                if state.ai_conversation_id() == Some(conversation_id) {
                    state.handle_ai_event(event);
                    if let Some(scroll_id) = state.ai_scroll_id() {
                        return iced::widget::operation::snap_to(
                            scroll_id,
                            scrollable::RelativeOffset::END,
                        );
                    }
                    return Task::none();
                }
            }
            for (_, (popout_panes, _)) in dash.popout.iter_mut() {
                for (_, state) in popout_panes.iter_mut() {
                    if state.ai_conversation_id()
                        == Some(conversation_id)
                    {
                        state.handle_ai_event(event);
                        if let Some(scroll_id) = state.ai_scroll_id() {
                            return iced::widget::operation::snap_to(
                                scroll_id,
                                scrollable::RelativeOffset::END,
                            );
                        }
                        return Task::none();
                    }
                }
            }
        }
        Task::none()
    }

    /// Apply an AI-initiated drawing action to the linked chart pane.
    fn apply_ai_drawing_action(
        &mut self,
        conversation_id: uuid::Uuid,
        action: AiDrawingAction,
    ) -> Task<Message> {
        use crate::chart::drawing::Drawing;

        log::debug!(
            "AI drawing action: {:?} (conv {})",
            action,
            conversation_id,
        );

        let dash = match self.active_dashboard_mut() {
            Some(d) => d,
            None => {
                log::warn!("AI drawing: no active dashboard");
                return Task::none();
            }
        };

        // Find the AI pane's link group
        let link_group = dash
            .panes
            .iter()
            .find(|(_, s)| {
                s.ai_conversation_id() == Some(conversation_id)
            })
            .and_then(|(_, s)| s.link_group);

        let Some(lg) = link_group else {
            log::warn!(
                "AI drawing: no link group for conv {}",
                conversation_id
            );
            return Task::none();
        };

        // Find the chart pane in the same link group
        let chart_pane = dash.panes.iter_mut().find(|(_, s)| {
            s.link_group == Some(lg)
                && !matches!(
                    s.content,
                    crate::screen::dashboard::pane::Content::AiAssistant(
                        _
                    )
                )
        });

        let Some((_, chart_state)) = chart_pane else {
            log::warn!(
                "AI drawing: no chart pane in link group {:?}",
                lg
            );
            return Task::none();
        };

        let Some(chart) = chart_state.content.drawing_chart_mut()
        else {
            log::warn!("AI drawing: pane has no drawing chart");
            return Task::none();
        };

        match action {
            AiDrawingAction::AddDrawing {
                ref drawing,
                ref description,
            } => {
                log::info!(
                    "AI adding drawing: {} (tool={:?}, points={})",
                    description,
                    drawing.tool,
                    drawing.points.len(),
                );
                let d = Drawing::from(drawing);
                chart.drawings_mut().add_drawing(d);
                chart.invalidate_all_drawing_caches();
            }
            AiDrawingAction::RemoveDrawing {
                ref id,
                ref description,
            } => {
                log::info!("AI removing drawing: {}", description);
                if let Ok(uuid) = uuid::Uuid::parse_str(id) {
                    chart
                        .drawings_mut()
                        .delete(data::DrawingId(uuid));
                    chart.invalidate_all_drawing_caches();
                }
            }
        }

        Task::none()
    }

    /// Handle the async completion of an AI stream task.
    pub(crate) fn handle_ai_stream_complete(&mut self) -> Task<Message> {
        Task::none()
    }

    /// Handle an AI context query from a chart drawing selection.
    pub(crate) fn handle_ai_context_query(
        &mut self,
        _context: String,
        question: String,
    ) -> Task<Message> {
        let dash = match self.active_dashboard_mut() {
            Some(d) => d,
            None => return Task::none(),
        };

        // Find existing AI pane or create one by splitting
        let ai_pane_id = {
            let existing = dash
                .panes
                .iter()
                .find(|(_, s)| {
                    matches!(
                        s.content,
                        crate::screen::dashboard::pane::Content::AiAssistant(
                            _
                        )
                    )
                })
                .map(|(_, s)| s.unique_id());

            match existing {
                Some(id) => id,
                None => {
                    use crate::screen::dashboard::pane;
                    use iced::widget::pane_grid;

                    let target = dash
                        .focus
                        .map(|(_, p)| p)
                        .or_else(|| {
                            dash.panes
                                .iter()
                                .next()
                                .map(|(p, _)| p)
                                .copied()
                        });
                    let Some(target) = target else {
                        return Task::none();
                    };

                    let mut new_state = pane::State::new();
                    new_state.content = pane::Content::AiAssistant(
                        pane::types::AiAssistantState::new(),
                    );
                    let id = new_state.unique_id();

                    if dash
                        .panes
                        .split(
                            pane_grid::Axis::Vertical,
                            target,
                            new_state,
                        )
                        .is_none()
                    {
                        return Task::none();
                    }
                    id
                }
            }
        };

        // Clear history and send the question
        if let Some(crate::screen::dashboard::pane::Content::AiAssistant(
            ai,
        )) = dash
            .panes
            .iter_mut()
            .find(|(_, s)| s.unique_id() == ai_pane_id)
            .map(|(_, s)| &mut s.content)
        {
            ai.clear_history();
        }

        self.handle_ai_request(ai_pane_id, question)
    }

    /// Kick off an AI completion request for a pane.
    pub(crate) fn handle_ai_request(
        &mut self,
        pane_id: uuid::Uuid,
        user_message: String,
    ) -> Task<Message> {
        // Retrieve API key
        let api_key = match self
            .secrets
            .get_api_key(data::config::secrets::ApiProvider::OpenRouter)
            .key()
        {
            Some(key) => key.to_string(),
            None => {
                let conversation_id = self
                    .active_dashboard_mut()
                    .and_then(|dash| {
                        dash.panes
                            .iter()
                            .find(|(_, s)| s.unique_id() == pane_id)
                            .and_then(|(_, s)| s.ai_conversation_id())
                    });
                if let Some(conv_id) = conversation_id {
                    let _ = globals::get_ai_sender().send(AiStreamEventClone::ApiKeyMissing {
                        conversation_id: conv_id,
                    });
                }
                return Task::none();
            }
        };

        let user_tz = self.ui.timezone;

        let dash = match self.active_dashboard_mut() {
            Some(d) => d,
            None => return Task::none(),
        };

        // Scoped mutable borrow: start streaming + extract settings
        let (model, conversation_id, api_history, link_group, temperature, max_tokens) = {
            let pane_state = dash
                .panes
                .iter_mut()
                .find(|(_, s)| s.unique_id() == pane_id)
                .map(|(_, s)| s);

            let Some(state) = pane_state else {
                return Task::none();
            };

            let (model, conversation_id, api_history) =
                match state.ai_start_streaming(&user_message) {
                    Some(info) => info,
                    None => return Task::none(),
                };

            let link_group = state.link_group;

            let (temperature, max_tokens) =
                if let crate::screen::dashboard::pane::Content::AiAssistant(
                    ref ai,
                ) = state.content
                {
                    (ai.temperature, ai.max_tokens)
                } else {
                    (0.3, 4096)
                };

            (model, conversation_id, api_history, link_group, temperature, max_tokens)
        };
        // Mutable borrow on dash.panes is now released

        // Build chart snapshot from first chart pane in same link group
        let chart_snapshot = link_group.and_then(|lg| {
            dash.panes
                .iter()
                .find(|(_, s)| {
                    s.link_group == Some(lg)
                        && !matches!(
                            s.content,
                            crate::screen::dashboard::pane::Content::AiAssistant(_)
                        )
                })
                .and_then(|(_, s)| {
                    snapshot::build_chart_snapshot(s, user_tz)
                })
        });

        // Push context attachment display message if we have a snapshot
        if let Some(snap) = &chart_snapshot
            && let Some(
                crate::screen::dashboard::pane::Content::AiAssistant(
                    ai,
                ),
            ) = dash
                .panes
                .iter_mut()
                .find(|(_, s)| s.unique_id() == pane_id)
                .map(|(_, s)| &mut s.content)
        {
            ai.messages.push(DisplayMessage::new(
                ChatMessageKind::ContextAttachment {
                    ticker: snap.ticker.clone(),
                    timeframe: snap.timeframe.clone(),
                    chart_type: snap.chart_type.clone(),
                    candle_count: snap.candles.len(),
                    is_live: snap.is_live,
                },
            ));
            ai.active_context = Some(
                crate::screen::dashboard::pane::types::ActiveContext {
                    ticker: snap.ticker.clone(),
                    timeframe: snap.timeframe.clone(),
                    chart_type: snap.chart_type.clone(),
                    candle_count: snap.candles.len(),
                    is_live: snap.is_live,
                },
            );
        }

        // Build tools JSON (only if snapshot present)
        let tools_json = if chart_snapshot.is_some() {
            tools::build_tools_json()
        } else {
            serde_json::json!([])
        };

        // Build initial API messages
        let initial_messages = streaming::build_api_messages(
            SYSTEM_PROMPT,
            &api_history,
            &chart_snapshot,
        );

        let ai_sender = globals::get_ai_sender();

        Task::perform(
            async move {
                streaming::stream_openrouter_agentic(
                    api_key,
                    model,
                    initial_messages,
                    tools_json,
                    conversation_id,
                    ai_sender,
                    chart_snapshot,
                    temperature,
                    max_tokens,
                )
                .await;
            },
            |_| Message::AiStreamComplete,
        )
    }
}
