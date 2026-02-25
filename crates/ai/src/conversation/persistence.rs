//! Conversation persistence — save / load / list / delete.
//!
//! Conversations are stored as individual JSON files inside a
//! `conversations/` directory under the application data path.

use super::Conversation;
use crate::error::AiError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Directory that holds conversation JSON files.
pub fn conversations_dir(base_path: &Path) -> PathBuf {
    base_path.join("conversations")
}

/// Save a conversation to disk.
pub fn save_conversation(
    base_path: &Path,
    conversation: &Conversation,
) -> Result<(), AiError> {
    let dir = conversations_dir(base_path);
    std::fs::create_dir_all(&dir).map_err(|e| {
        AiError::Conversation(format!(
            "failed to create conversations dir: {e}"
        ))
    })?;

    let path =
        dir.join(format!("{}.json", conversation.id));
    let json =
        serde_json::to_string_pretty(conversation).map_err(
            |e| AiError::Serialization(e.to_string()),
        )?;

    std::fs::write(&path, json).map_err(|e| {
        AiError::Conversation(format!(
            "failed to write {}: {e}",
            path.display()
        ))
    })
}

/// Load a conversation by ID.
pub fn load_conversation(
    base_path: &Path,
    id: uuid::Uuid,
) -> Result<Conversation, AiError> {
    let path = conversations_dir(base_path)
        .join(format!("{id}.json"));

    let json = std::fs::read_to_string(&path).map_err(|e| {
        AiError::Conversation(format!(
            "failed to read {}: {e}",
            path.display()
        ))
    })?;

    serde_json::from_str::<Conversation>(&json)
        .map_err(|e| AiError::Serialization(e.to_string()))
}

/// List all saved conversations (summaries only, sorted newest
/// first).
pub fn list_conversations(
    base_path: &Path,
) -> Result<Vec<ConversationSummary>, AiError> {
    let dir = conversations_dir(base_path);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut summaries = Vec::new();

    let entries =
        std::fs::read_dir(&dir).map_err(|e| {
            AiError::Conversation(format!(
                "failed to read conversations dir: {e}"
            ))
        })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str())
            != Some("json")
        {
            continue;
        }

        if let Ok(json) = std::fs::read_to_string(&path) {
            if let Ok(conv) =
                serde_json::from_str::<Conversation>(&json)
            {
                summaries.push(ConversationSummary {
                    id: conv.id,
                    title: if conv.title.is_empty() {
                        conv.generate_title()
                    } else {
                        conv.title.clone()
                    },
                    model: conv.model.clone(),
                    created_at: conv.created_at,
                    message_count: conv.messages.len(),
                });
            }
        }
    }

    // Newest first
    summaries
        .sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(summaries)
}

/// Delete a conversation by ID.
pub fn delete_conversation(
    base_path: &Path,
    id: uuid::Uuid,
) -> Result<(), AiError> {
    let path = conversations_dir(base_path)
        .join(format!("{id}.json"));

    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| {
            AiError::Conversation(format!(
                "failed to delete {}: {e}",
                path.display()
            ))
        })?;
    }
    Ok(())
}

/// Lightweight conversation summary (no message content).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub id: uuid::Uuid,
    pub title: String,
    pub model: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub message_count: usize,
}
