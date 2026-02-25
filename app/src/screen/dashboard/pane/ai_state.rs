use crate::app::core::globals::AiStreamEventClone;

use data::{
    ChartConfig,
    domain::assistant::{
        ApiMessage, ChatMessageKind, ChatRole, DisplayMessage, TokenUsage,
    },
};
use exchange::FuturesTickerInfo;
use iced::Point;
use std::collections::HashSet;

/// A model option for the AI settings picker.
pub struct ModelOption {
    pub id: &'static str,
    pub display_name: &'static str,
}

pub const AI_MODELS: &[ModelOption] = &[
    ModelOption {
        id: "google/gemini-3-flash-preview",
        display_name: "Gemini 3 Flash",
    },
    ModelOption {
        id: "google/gemini-2.5-pro-preview",
        display_name: "Gemini 2.5 Pro",
    },
    ModelOption {
        id: "anthropic/claude-sonnet-4",
        display_name: "Claude Sonnet 4",
    },
    ModelOption {
        id: "openai/gpt-4.1",
        display_name: "GPT-4.1",
    },
    ModelOption {
        id: "openai/o4-mini",
        display_name: "o4-mini",
    },
];

/// Resolve model ID to display name.
pub fn model_display_name(id: &str) -> &'static str {
    AI_MODELS
        .iter()
        .find(|m| m.id == id)
        .map(|m| m.display_name)
        .unwrap_or("Unknown")
}

/// Resolve display name to model ID.
pub fn model_id_from_name(name: &str) -> &'static str {
    AI_MODELS
        .iter()
        .find(|m| m.display_name == name)
        .map(|m| m.id)
        .unwrap_or(AI_MODELS[0].id)
}

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

/// State for the AI assistant panel content.
pub struct AiAssistantState {
    /// Display messages for the chat UI.
    pub messages: Vec<DisplayMessage>,
    /// Wire-format message history for the API (user/assistant/tool).
    pub api_history: Vec<ApiMessage>,
    pub input_text: String,
    pub is_streaming: bool,
    pub streaming_buffer: String,
    pub streaming_tool_calls: Vec<StreamingToolCall>,
    pub model: String,
    pub session_usage: TokenUsage,
    pub scroll_id: iced::widget::Id,
    pub show_settings: bool,
    pub show_api_key_modal: bool,
    pub api_key_input: String,
    pub conversation_id: uuid::Uuid,
    pub temperature: f32,
    pub max_tokens: u32,
    /// Accumulates `<think>` content during streaming.
    pub thinking_buffer: String,
    /// State machine flag: currently inside `<think>` tags.
    pub in_think_block: bool,
    /// Which thinking-block message indices are expanded.
    pub expanded_thinking: HashSet<usize>,
    /// Last known cursor position (for context menu placement).
    pub last_cursor_position: Point,
    /// Context chip shown above the input row.
    pub active_context: Option<ActiveContext>,
}

impl AiAssistantState {
    pub fn new() -> Self {
        Self {
            messages: vec![],
            api_history: vec![],
            input_text: String::new(),
            is_streaming: false,
            streaming_buffer: String::new(),
            streaming_tool_calls: vec![],
            model: "google/gemini-3-flash-preview".to_string(),
            session_usage: TokenUsage::default(),
            scroll_id: iced::widget::Id::unique(),
            show_settings: false,
            show_api_key_modal: false,
            api_key_input: String::new(),
            conversation_id: uuid::Uuid::new_v4(),
            temperature: 0.3,
            max_tokens: 4096,
            thinking_buffer: String::new(),
            in_think_block: false,
            expanded_thinking: HashSet::new(),
            last_cursor_position: Point::ORIGIN,
            active_context: None,
        }
    }

    /// Create with saved preferences.
    pub fn with_preferences(
        prefs: &data::AiPreferences,
    ) -> Self {
        let mut state = Self::new();
        state.model = prefs.model.clone();
        state.temperature = prefs.temperature;
        state.max_tokens = prefs.max_tokens;
        state
    }

    /// Commit thinking and streaming buffers as display messages.
    /// Returns true if anything was committed.
    fn commit_streaming_buffer(&mut self) -> bool {
        let mut committed = false;

        let think_text = std::mem::take(&mut self.thinking_buffer);
        if !think_text.is_empty() {
            self.messages.push(DisplayMessage::new(
                ChatMessageKind::Thinking { text: think_text },
            ));
            committed = true;
        }

        let text = std::mem::take(&mut self.streaming_buffer);
        if !text.is_empty() {
            self.messages.push(DisplayMessage::new(
                ChatMessageKind::AssistantText { text },
            ));
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
            ChatMessageKind::SystemNotice { text, .. } => {
                Some(text.clone())
            }
            ChatMessageKind::ToolResult {
                display_summary, ..
            } => Some(display_summary.clone()),
            _ => None,
        })
    }

    /// Handle an AI stream event, mutating state in place.
    pub fn handle_event(&mut self, event: AiStreamEventClone) {
        match event {
            AiStreamEventClone::Delta { text, .. } => {
                // Route delta through think-tag state machine
                let mut remaining = text.as_str();
                while !remaining.is_empty() {
                    if self.in_think_block {
                        if let Some(pos) =
                            remaining.find("</think>")
                        {
                            self.thinking_buffer
                                .push_str(&remaining[..pos]);
                            self.in_think_block = false;
                            remaining =
                                &remaining[pos + "</think>".len()..];
                        } else {
                            self.thinking_buffer.push_str(remaining);
                            break;
                        }
                    } else if let Some(pos) =
                        remaining.find("<think>")
                    {
                        self.streaming_buffer
                            .push_str(&remaining[..pos]);
                        self.in_think_block = true;
                        remaining =
                            &remaining[pos + "<think>".len()..];
                    } else {
                        self.streaming_buffer.push_str(remaining);
                        break;
                    }
                }
            }
            AiStreamEventClone::ToolCallStarted {
                call_id,
                name,
                arguments_json,
                display_summary,
                ..
            } => {
                // Commit any text before the tool call
                self.commit_streaming_buffer();
                // Push ToolCall display message
                self.messages.push(DisplayMessage::new(
                    ChatMessageKind::ToolCall {
                        call_id: call_id.clone(),
                        name: name.clone(),
                        arguments_json,
                        display_summary: display_summary.clone(),
                    },
                ));
                self.streaming_tool_calls.push(StreamingToolCall {
                    call_id,
                    name,
                    display_summary,
                    result_summary: None,
                    is_error: false,
                    is_complete: false,
                });
            }
            AiStreamEventClone::ToolCallResult {
                call_id,
                name,
                content_json,
                display_summary,
                is_error,
                ..
            } => {
                // Update the matching streaming tool call
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
                // Push ToolResult display message
                self.messages.push(DisplayMessage::new(
                    ChatMessageKind::ToolResult {
                        call_id,
                        name,
                        content_json,
                        display_summary,
                        is_error,
                    },
                ));
            }
            AiStreamEventClone::ApiHistorySync {
                rounds,
                final_text,
                ..
            } => {
                // Sync tool call rounds back to api_history for
                // multi-turn context.
                for round in &rounds {
                    // Assistant message with tool_calls
                    self.api_history.push(ApiMessage {
                        role: ChatRole::Assistant,
                        content: None,
                        tool_calls: Some(vec![
                            data::domain::assistant::ApiToolCall {
                                id: round.call_id.clone(),
                                call_type: "function".to_string(),
                                function:
                                    data::domain::assistant::ApiToolCallFunction {
                                        name: round.name.clone(),
                                        arguments: round
                                            .arguments
                                            .clone(),
                                    },
                            },
                        ]),
                        tool_call_id: None,
                    });
                    // Tool result message
                    self.api_history.push(ApiMessage {
                        role: ChatRole::Tool,
                        content: Some(round.result_json.clone()),
                        tool_calls: None,
                        tool_call_id: Some(round.call_id.clone()),
                    });
                }
                // Final assistant text
                if let Some(text) = final_text {
                    if !text.is_empty() {
                        self.api_history.push(ApiMessage {
                            role: ChatRole::Assistant,
                            content: Some(text),
                            tool_calls: None,
                            tool_call_id: None,
                        });
                    }
                }
            }
            AiStreamEventClone::TextSegmentComplete { .. } => {
                self.commit_streaming_buffer();
            }
            AiStreamEventClone::Complete {
                prompt_tokens,
                completion_tokens,
                ..
            } => {
                // Commit remaining buffer
                self.commit_streaming_buffer();
                self.streaming_tool_calls.clear();
                self.is_streaming = false;
                self.session_usage
                    .add(prompt_tokens, completion_tokens);
            }
            AiStreamEventClone::Error { error, .. } => {
                self.is_streaming = false;
                // Commit any partial content
                self.commit_streaming_buffer();
                self.streaming_tool_calls.clear();
                // Add error as system notice
                self.messages.push(DisplayMessage::new(
                    ChatMessageKind::SystemNotice {
                        text: format!("Error: {}", error),
                        is_error: true,
                    },
                ));
            }
            AiStreamEventClone::ApiKeyMissing { .. } => {
                self.show_api_key_modal = true;
            }
            // Drawing actions are handled at the Kairos level,
            // not at the pane level (needs cross-pane access).
            AiStreamEventClone::DrawingAction { .. } => {}
        }
    }

    /// Start streaming: add user message, set streaming flag.
    /// Returns (model, conversation_id, api_history clone).
    pub fn start_streaming(
        &mut self,
        user_message: &str,
    ) -> (String, uuid::Uuid, Vec<ApiMessage>) {
        // Push user display message
        self.messages.push(DisplayMessage::new(
            ChatMessageKind::User {
                text: user_message.to_string(),
            },
        ));
        // Push to API history
        self.api_history.push(ApiMessage {
            role: ChatRole::User,
            content: Some(user_message.to_string()),
            tool_calls: None,
            tool_call_id: None,
        });
        self.input_text.clear();
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
        // Add system notice
        self.messages.push(DisplayMessage::new(
            ChatMessageKind::SystemNotice {
                text: "Response stopped by user".to_string(),
                is_error: false,
            },
        ));
        // New UUID orphans the running task
        self.conversation_id = uuid::Uuid::new_v4();
    }

    /// Clear conversation history.
    pub fn clear_history(&mut self) {
        self.messages.clear();
        self.api_history.clear();
        self.streaming_buffer.clear();
        self.thinking_buffer.clear();
        self.in_think_block = false;
        self.expanded_thinking.clear();
        self.streaming_tool_calls.clear();
        self.session_usage = Default::default();
        self.conversation_id = uuid::Uuid::new_v4();
        self.is_streaming = false;
        self.active_context = None;
    }
}

// ── AI Context Bubble ──────────────────────────────────────────────

/// State for the floating AI context bubble shown after drawing an
/// AiContext rectangle on a chart.
#[derive(Debug, Clone)]
pub struct AiContextBubble {
    pub drawing_id: data::DrawingId,
    pub input_text: String,
    pub range_summary: AiContextSummary,
}

/// Summary of the selected chart region for the AI context bubble.
#[derive(Debug, Clone)]
pub struct AiContextSummary {
    pub ticker: String,
    pub timeframe: String,
    pub time_start_fmt: String,
    pub time_end_fmt: String,
    pub price_high: String,
    pub price_low: String,
    pub candle_count: usize,
    pub total_volume: String,
    pub net_delta: String,
    /// Pre-formatted OHLCV lines (capped at 50 candles)
    pub candle_ohlcv_lines: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum AiContextBubbleEvent {
    InputChanged(String),
    Submit,
    Dismiss,
}

#[derive(Debug, Clone)]
pub enum AiAssistantEvent {
    InputChanged(String),
    SendMessage,
    StopStreaming,
    ToggleSettings,
    ModelChanged(String),
    TemperatureChanged(f32),
    MaxTokensChanged(u32),
    RetryLastMessage,
    ClearHistory,
    ApiKeyInputChanged(String),
    SaveApiKey,
    DismissApiKeyModal,
    OpenUrl(String),
    CursorMoved(Point),
    MessageRightClicked(usize),
    ToggleThinking(usize),
}

// Re-export FuturesTickerInfo and ChartConfig so the LoadChart
// variant in TickAction is resolved correctly.
use crate::{chart, screen::dashboard::panel};

pub enum TickAction {
    Chart(chart::Action),
    Panel(panel::Action),
    LoadChart {
        config: ChartConfig,
        ticker_info: FuturesTickerInfo,
    },
}
