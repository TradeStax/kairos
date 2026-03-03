//! Chat message types and token usage.
//!
//! These are the pure domain types for AI conversations — no I/O,
//! no framework dependencies.

use serde::{Deserialize, Serialize};

// ── Chat Roles and Messages ─────────────────────────────────────────────

/// Role in a chat conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatRole {
    /// User input
    User,
    /// Assistant response
    Assistant,
    /// System prompt
    System,
    /// Tool call result
    Tool,
}

/// Typed message kind for the UI — each variant maps to a distinct
/// visual bubble in the chat view.
#[derive(Debug, Clone)]
pub enum ChatMessageKind {
    /// User-authored text
    User {
        /// The user's message text
        text: String,
    },
    /// Assistant-authored text response
    AssistantText {
        /// The assistant's response text
        text: String,
    },
    /// A tool call made by the assistant
    ToolCall {
        /// Unique identifier for this tool call
        call_id: String,
        /// Tool function name
        name: String,
        /// JSON-encoded arguments
        arguments_json: String,
        /// Human-readable summary for the UI
        display_summary: String,
    },
    /// Result returned from a tool call
    ToolResult {
        /// Matching call identifier
        call_id: String,
        /// Tool function name
        name: String,
        /// JSON-encoded result content
        content_json: String,
        /// Human-readable summary for the UI
        display_summary: String,
        /// Whether the tool call resulted in an error
        is_error: bool,
    },
    /// Assistant thinking / chain-of-thought
    Thinking {
        /// The thinking text
        text: String,
    },
    /// Attached chart context sent alongside a user message
    ContextAttachment {
        /// Ticker symbol
        ticker: String,
        /// Timeframe label
        timeframe: String,
        /// Chart type label
        chart_type: String,
        /// Number of candles included
        candle_count: usize,
        /// Whether the chart has live data
        is_live: bool,
    },
    /// System notice or error banner
    SystemNotice {
        /// Notice text
        text: String,
        /// Whether this is an error notice
        is_error: bool,
    },
}

/// A single display message in the chat UI.
#[derive(Debug, Clone)]
pub struct DisplayMessage {
    /// Message content and type
    pub kind: ChatMessageKind,
    /// When the message was created
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl DisplayMessage {
    /// Create a new display message with the current timestamp
    #[must_use]
    pub fn new(kind: ChatMessageKind) -> Self {
        Self {
            kind,
            timestamp: chrono::Utc::now(),
        }
    }
}

// ── API Wire Format ─────────────────────────────────────────────────────

/// Wire-format message for the OpenAI/OpenRouter API.
#[derive(Debug, Clone)]
pub struct ApiMessage {
    /// Message role
    pub role: ChatRole,
    /// Text content (may be absent for tool-call messages)
    pub content: Option<String>,
    /// Tool calls requested by the assistant
    pub tool_calls: Option<Vec<ApiToolCall>>,
    /// Tool call ID (present when role is `Tool`)
    pub tool_call_id: Option<String>,
}

/// A single tool call in the API wire format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiToolCall {
    /// Unique call identifier
    pub id: String,
    /// Call type (always `"function"`)
    #[serde(rename = "type")]
    pub call_type: String,
    /// Function name and arguments
    pub function: ApiToolCallFunction,
}

/// Function name and JSON arguments within an [`ApiToolCall`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiToolCallFunction {
    /// Function name
    pub name: String,
    /// JSON-encoded arguments string
    pub arguments: String,
}

// ── Token Usage ─────────────────────────────────────────────────────────

/// Session-total token usage for status bar display.
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    /// Total prompt (input) tokens
    pub prompt_tokens: u32,
    /// Total completion (output) tokens
    pub completion_tokens: u32,
    /// Total tokens (prompt + completion)
    pub total_tokens: u32,
}

impl TokenUsage {
    /// Accumulate tokens from a single API response
    pub fn add(&mut self, prompt: u32, completion: u32) {
        self.prompt_tokens += prompt;
        self.completion_tokens += completion;
        self.total_tokens += prompt + completion;
    }

    /// Format for display (e.g. `"1.5K tokens"`), or empty string if zero
    #[must_use]
    pub fn format_display(&self) -> String {
        if self.total_tokens == 0 {
            return String::new();
        }
        format_token_count(self.total_tokens)
    }
}

/// Format a token count with K/M suffixes.
fn format_token_count(count: u32) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M tokens", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K tokens", count as f64 / 1_000.0)
    } else {
        format!("{count} tokens")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_usage_display() {
        let mut usage = TokenUsage::default();
        assert_eq!(usage.format_display(), "");

        usage.add(1000, 500);
        assert_eq!(usage.total_tokens, 1500);
        assert_eq!(usage.format_display(), "1.5K tokens");
    }

    #[test]
    fn test_display_message() {
        let msg = DisplayMessage::new(ChatMessageKind::User {
            text: "hello".to_string(),
        });
        assert!(matches!(msg.kind, ChatMessageKind::User { .. }));
    }
}
