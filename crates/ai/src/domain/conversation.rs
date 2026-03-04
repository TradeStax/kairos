//! AI conversation state — pure domain logic without GUI dependencies.

use super::messages::{
    ApiMessage, ApiToolCall, ApiToolCallFunction, ChatMessageKind, ChatRole, DisplayMessage,
    TokenUsage,
};
use crate::event::AiStreamEvent;

/// In-progress tool call during streaming.
pub struct StreamingToolCall {
    pub call_id: String,
    pub name: String,
    pub display_summary: String,
    pub result_summary: Option<String>,
    pub is_error: bool,
    pub is_complete: bool,
}

/// Context snapshot shown as a chip above the input when a chart is linked.
pub struct ActiveContext {
    pub ticker: String,
    pub timeframe: String,
    pub chart_type: String,
    pub candle_count: usize,
    pub is_live: bool,
}

/// Pure domain state for an AI conversation. No GUI framework deps.
pub struct AiConversation {
    /// Display messages for the chat UI.
    pub messages: Vec<DisplayMessage>,
    /// Wire-format message history for the API (user/assistant/tool).
    pub api_history: Vec<ApiMessage>,
    pub is_streaming: bool,
    pub streaming_buffer: String,
    pub streaming_tool_calls: Vec<StreamingToolCall>,
    pub model: String,
    pub session_usage: TokenUsage,
    pub conversation_id: uuid::Uuid,
    pub temperature: f32,
    pub max_tokens: u32,
    /// Accumulates `<think>` content during streaming.
    pub thinking_buffer: String,
    /// State machine flag: currently inside `<think>` tags.
    pub in_think_block: bool,
    /// Context chip shown above the input row.
    pub active_context: Option<ActiveContext>,
}

impl AiConversation {
    pub fn new() -> Self {
        Self {
            messages: vec![],
            api_history: vec![],
            is_streaming: false,
            streaming_buffer: String::new(),
            streaming_tool_calls: vec![],
            model: "google/gemini-3-flash-preview".to_string(),
            session_usage: TokenUsage::default(),
            conversation_id: uuid::Uuid::new_v4(),
            temperature: 0.3,
            max_tokens: 4096,
            thinking_buffer: String::new(),
            in_think_block: false,
            active_context: None,
        }
    }

    /// Commit thinking and streaming buffers as display messages.
    /// Returns true if anything was committed.
    pub fn commit_streaming_buffer(&mut self) -> bool {
        let mut committed = false;

        let think_text = std::mem::take(&mut self.thinking_buffer);
        if !think_text.is_empty() {
            self.messages
                .push(DisplayMessage::new(ChatMessageKind::Thinking {
                    text: think_text,
                }));
            committed = true;
        }

        let text = std::mem::take(&mut self.streaming_buffer);
        if !text.is_empty() {
            self.messages
                .push(DisplayMessage::new(ChatMessageKind::AssistantText { text }));
            committed = true;
        }
        committed
    }

    /// Extract text content from a display message by index.
    pub fn message_text(&self, index: usize) -> Option<String> {
        self.messages.get(index).and_then(|m| match &m.kind {
            ChatMessageKind::User { text }
            | ChatMessageKind::AssistantText { text }
            | ChatMessageKind::Thinking { text } => Some(text.clone()),
            ChatMessageKind::SystemNotice { text, .. } => Some(text.clone()),
            ChatMessageKind::ToolResult {
                display_summary, ..
            } => Some(display_summary.clone()),
            _ => None,
        })
    }

    /// Handle an AI stream event, mutating state in place.
    ///
    /// Returns `true` for `ApiKeyMissing` so the caller (app layer)
    /// can show the API key modal.
    pub fn handle_event(&mut self, event: AiStreamEvent) -> bool {
        match event {
            AiStreamEvent::Delta { text, .. } => {
                // Route delta through think-tag state machine
                let mut remaining = text.as_str();
                while !remaining.is_empty() {
                    if self.in_think_block {
                        if let Some(pos) = remaining.find("</think>") {
                            self.thinking_buffer.push_str(&remaining[..pos]);
                            self.in_think_block = false;
                            remaining = &remaining[pos + "</think>".len()..];
                        } else {
                            self.thinking_buffer.push_str(remaining);
                            break;
                        }
                    } else if let Some(pos) = remaining.find("<think>") {
                        self.streaming_buffer.push_str(&remaining[..pos]);
                        self.in_think_block = true;
                        remaining = &remaining[pos + "<think>".len()..];
                    } else {
                        self.streaming_buffer.push_str(remaining);
                        break;
                    }
                }
                false
            }
            AiStreamEvent::ToolCallStarted {
                call_id,
                name,
                arguments_json,
                display_summary,
                ..
            } => {
                self.commit_streaming_buffer();
                self.messages
                    .push(DisplayMessage::new(ChatMessageKind::ToolCall {
                        call_id: call_id.clone(),
                        name: name.clone(),
                        arguments_json,
                        display_summary: display_summary.clone(),
                    }));
                self.streaming_tool_calls.push(StreamingToolCall {
                    call_id,
                    name,
                    display_summary,
                    result_summary: None,
                    is_error: false,
                    is_complete: false,
                });
                false
            }
            AiStreamEvent::ToolCallResult {
                call_id,
                name,
                content_json,
                display_summary,
                is_error,
                ..
            } => {
                if let Some(tc) = self
                    .streaming_tool_calls
                    .iter_mut()
                    .rev()
                    .find(|tc| tc.call_id == call_id)
                {
                    tc.result_summary = Some(display_summary.clone());
                    tc.is_error = is_error;
                    tc.is_complete = true;
                }
                self.messages
                    .push(DisplayMessage::new(ChatMessageKind::ToolResult {
                        call_id,
                        name,
                        content_json,
                        display_summary,
                        is_error,
                    }));
                false
            }
            AiStreamEvent::ApiHistorySync {
                rounds, final_text, ..
            } => {
                for round in &rounds {
                    self.api_history.push(ApiMessage {
                        role: ChatRole::Assistant,
                        content: None,
                        tool_calls: Some(vec![ApiToolCall {
                            id: round.call_id.clone(),
                            call_type: "function".to_string(),
                            function: ApiToolCallFunction {
                                name: round.name.clone(),
                                arguments: round.arguments.clone(),
                            },
                        }]),
                        tool_call_id: None,
                    });
                    self.api_history.push(ApiMessage {
                        role: ChatRole::Tool,
                        content: Some(round.result_json.clone()),
                        tool_calls: None,
                        tool_call_id: Some(round.call_id.clone()),
                    });
                }
                if let Some(text) = final_text
                    && !text.is_empty()
                {
                    self.api_history.push(ApiMessage {
                        role: ChatRole::Assistant,
                        content: Some(text),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
                false
            }
            AiStreamEvent::TextSegmentComplete { .. } => {
                self.commit_streaming_buffer();
                false
            }
            AiStreamEvent::Complete {
                prompt_tokens,
                completion_tokens,
                ..
            } => {
                self.commit_streaming_buffer();
                self.streaming_tool_calls.clear();
                self.is_streaming = false;
                self.session_usage.add(prompt_tokens, completion_tokens);
                false
            }
            AiStreamEvent::Error { error, .. } => {
                self.is_streaming = false;
                self.commit_streaming_buffer();
                self.streaming_tool_calls.clear();
                self.messages
                    .push(DisplayMessage::new(ChatMessageKind::SystemNotice {
                        text: format!("Error: {error}"),
                        is_error: true,
                    }));
                false
            }
            AiStreamEvent::ApiKeyMissing { .. } => true,
            // Drawing actions are handled at the app level
            AiStreamEvent::DrawingAction { .. } => false,
        }
    }

    /// Start streaming: add user message, set streaming flag.
    /// Returns (model, conversation_id, api_history clone).
    ///
    /// When `display_text` is `Some`, it is shown in the chat bubble
    /// while `user_message` (which may contain extra context) is sent
    /// to the API.
    pub fn start_streaming(
        &mut self,
        user_message: &str,
        display_text: Option<&str>,
    ) -> (String, uuid::Uuid, Vec<ApiMessage>) {
        let shown = display_text.unwrap_or(user_message);
        self.messages
            .push(DisplayMessage::new(ChatMessageKind::User {
                text: shown.to_string(),
            }));
        self.api_history.push(ApiMessage {
            role: ChatRole::User,
            content: Some(user_message.to_string()),
            tool_calls: None,
            tool_call_id: None,
        });
        self.is_streaming = true;
        self.streaming_buffer.clear();
        self.streaming_tool_calls.clear();
        (
            self.model.clone(),
            self.conversation_id,
            self.api_history.clone(),
        )
    }

    /// Stop streaming mid-response. Commits any partial content as a
    /// message, then rotates conversation_id so in-flight events
    /// are ignored.
    pub fn stop_streaming(&mut self) {
        self.is_streaming = false;
        self.in_think_block = false;
        self.commit_streaming_buffer();
        self.streaming_tool_calls.clear();
        self.messages
            .push(DisplayMessage::new(ChatMessageKind::SystemNotice {
                text: "Response stopped by user".to_string(),
                is_error: false,
            }));
        self.conversation_id = uuid::Uuid::new_v4();
    }

    /// Clear conversation history.
    pub fn clear_history(&mut self) {
        self.messages.clear();
        self.api_history.clear();
        self.streaming_buffer.clear();
        self.thinking_buffer.clear();
        self.in_think_block = false;
        self.streaming_tool_calls.clear();
        self.session_usage = Default::default();
        self.conversation_id = uuid::Uuid::new_v4();
        self.is_streaming = false;
        self.active_context = None;
    }
}

impl Default for AiConversation {
    fn default() -> Self {
        Self::new()
    }
}
