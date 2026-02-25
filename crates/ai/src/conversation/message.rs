//! Conversation message types.

use serde::{Deserialize, Serialize};

/// Message role in a conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

impl Role {
    /// API-level role string (`"system"`, `"user"`, etc.).
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        }
    }
}

/// A single message within a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub role: Role,
    /// Text content (may be `None` for tool-call-only messages).
    pub content: Option<String>,
    /// Tool calls made by the assistant in this turn.
    pub tool_calls: Option<Vec<ToolCallRecord>>,
    /// For tool-result messages, the ID of the call this answers.
    pub tool_call_id: Option<String>,
    /// Attached images (base64-encoded).
    pub images: Option<Vec<ImageContent>>,
    /// When this message was created.
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Record of a single tool call and its result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// Unique tool call ID (from the API).
    pub id: String,
    /// Tool/function name.
    pub name: String,
    /// JSON-encoded arguments string.
    pub arguments: String,
    /// Result content (filled in after execution).
    pub result: Option<String>,
    /// Whether the tool call resulted in an error.
    pub is_error: bool,
}

/// Base64-encoded image content for multimodal messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageContent {
    /// MIME type (e.g. `"image/png"`).
    pub mime_type: String,
    /// Base64-encoded image data.
    pub data: String,
}

impl ConversationMessage {
    /// Create a user message.
    pub fn user(content: String) -> Self {
        Self {
            role: Role::User,
            content: Some(content),
            tool_calls: None,
            tool_call_id: None,
            images: None,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create an assistant message (text).
    pub fn assistant(content: String) -> Self {
        Self {
            role: Role::Assistant,
            content: Some(content),
            tool_calls: None,
            tool_call_id: None,
            images: None,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create an assistant message with tool calls.
    pub fn assistant_with_tool_calls(
        content: Option<String>,
        tool_calls: Vec<ToolCallRecord>,
    ) -> Self {
        Self {
            role: Role::Assistant,
            content,
            tool_calls: Some(tool_calls),
            tool_call_id: None,
            images: None,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create a system message.
    pub fn system(content: String) -> Self {
        Self {
            role: Role::System,
            content: Some(content),
            tool_calls: None,
            tool_call_id: None,
            images: None,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create a tool result message.
    pub fn tool_result(
        tool_call_id: String,
        content: String,
        is_error: bool,
    ) -> Self {
        Self {
            role: Role::Tool,
            content: Some(content),
            tool_calls: None,
            tool_call_id: Some(tool_call_id),
            images: None,
            timestamp: chrono::Utc::now(),
        }
    }
}
