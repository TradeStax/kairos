//! Conversation model and persistence.
//!
//! A `Conversation` holds an ordered list of messages exchanged
//! between the user, the AI assistant, and any tool calls.

pub mod message;
pub mod persistence;
pub mod token;

pub use message::{
    ConversationMessage, ImageContent, Role, ToolCallRecord,
};
pub use persistence::ConversationSummary;
pub use token::TokenUsage;

use serde::{Deserialize, Serialize};

/// A single conversation thread with the AI assistant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    /// Unique identifier.
    pub id: uuid::Uuid,
    /// Human-readable title (auto-generated or user-set).
    pub title: String,
    /// Model used for this conversation.
    pub model: String,
    /// Optionally linked ticker symbol
    /// (e.g. `"ES.c.0"` for chart context).
    pub linked_ticker: Option<String>,
    /// Ordered messages.
    pub messages: Vec<ConversationMessage>,
    /// When the conversation was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// When the conversation was last updated.
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// Accumulated token usage across all rounds.
    pub token_usage: TokenUsage,
}

impl Conversation {
    /// Create a new empty conversation.
    pub fn new(
        model: String,
        linked_ticker: Option<String>,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: uuid::Uuid::new_v4(),
            title: String::new(),
            model,
            linked_ticker,
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
            token_usage: TokenUsage::default(),
        }
    }

    /// Append a message and bump `updated_at`.
    pub fn add_message(&mut self, msg: ConversationMessage) {
        self.messages.push(msg);
        self.updated_at = chrono::Utc::now();
    }

    /// Auto-generate a title from the first user message,
    /// truncated to 80 characters.
    pub fn generate_title(&self) -> String {
        for msg in &self.messages {
            if matches!(msg.role, Role::User) {
                if let Some(content) = &msg.content {
                    let trimmed = content.trim();
                    if trimmed.len() <= 80 {
                        return trimmed.to_string();
                    }
                    let truncated: String = trimmed
                        .chars()
                        .take(77)
                        .collect();
                    return format!("{}...", truncated);
                }
            }
        }
        "New conversation".to_string()
    }
}
