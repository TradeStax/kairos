//! Context window pruning for API message history.

use serde_json::Value;

/// Estimate token count from a JSON message array using char-based heuristic.
/// ~3.5 chars per token is a reasonable approximation for mixed text/JSON.
fn estimate_tokens(messages: &[Value]) -> u32 {
    let total_chars: usize = messages.iter().map(|m| m.to_string().len()).sum();
    (total_chars as f64 / 3.5) as u32
}

/// Prune messages to fit within a token budget.
///
/// Strategy:
/// - Always keep system messages (indices where role == "system")
/// - Always keep the last user message
/// - Drop oldest user/assistant/tool message groups from the front
/// - Preserve tool_call/tool_result pairing integrity
///
/// Returns the number of messages removed.
pub fn prune_messages(messages: &mut Vec<Value>, budget_tokens: u32) -> usize {
    if estimate_tokens(messages) <= budget_tokens {
        return 0;
    }

    // Find the index of the last user message — we must keep it.
    let last_user_idx = messages
        .iter()
        .rposition(|m| m["role"].as_str() == Some("user"));

    // Identify which messages are removable (not system, not the last user msg).
    // We remove in "groups": a user message, followed by assistant+tool messages,
    // up to (but not including) the next user message.
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current_group: Vec<usize> = Vec::new();

    for (i, msg) in messages.iter().enumerate() {
        let role = msg["role"].as_str().unwrap_or("");

        // Never include system messages in removable groups.
        if role == "system" {
            continue;
        }

        // Never include the last user message.
        if Some(i) == last_user_idx {
            // Flush any pending group before the last user message.
            if !current_group.is_empty() {
                groups.push(std::mem::take(&mut current_group));
            }
            continue;
        }

        // A user message starts a new group (flush previous).
        if role == "user" && !current_group.is_empty() {
            groups.push(std::mem::take(&mut current_group));
        }

        current_group.push(i);
    }

    // Flush remaining group (messages after last user msg are kept
    // only if they aren't part of an earlier group).
    if !current_group.is_empty() {
        groups.push(current_group);
    }

    // Remove groups from the front (oldest) until under budget.
    let mut removed = 0usize;
    let mut indices_to_remove: Vec<usize> = Vec::new();

    for group in &groups {
        if estimate_tokens(messages) <= budget_tokens {
            break;
        }

        // Mark this group for removal.
        indices_to_remove.extend(group);
        removed += group.len();

        // Temporarily replace with null to update token estimate.
        // We'll compact after the loop.
        for &idx in group {
            messages[idx] = Value::Null;
        }
    }

    if removed > 0 {
        messages.retain(|m| !m.is_null());
    }

    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn msg(role: &str, content: &str) -> Value {
        json!({"role": role, "content": content})
    }

    fn assistant_with_tool_calls(text: &str, call_id: &str) -> Value {
        json!({
            "role": "assistant",
            "content": text,
            "tool_calls": [{"id": call_id, "type": "function",
                "function": {"name": "test", "arguments": "{}"}}]
        })
    }

    fn tool_result(call_id: &str, content: &str) -> Value {
        json!({"role": "tool", "tool_call_id": call_id, "content": content})
    }

    #[test]
    fn system_messages_preserved() {
        let mut messages = vec![
            msg("system", "You are helpful."),
            msg("user", "Hello"),
            msg("assistant", "Hi there!"),
            msg("user", "How are you?"),
        ];
        // Use a tiny budget to force pruning
        let removed = prune_messages(&mut messages, 1);
        // System message must survive
        assert!(messages.iter().any(|m| m["role"] == "system"));
    }

    #[test]
    fn last_user_message_preserved() {
        let mut messages = vec![
            msg("system", "System prompt."),
            msg("user", "First question"),
            msg("assistant", "First answer"),
            msg("user", "Second question"),
        ];
        let removed = prune_messages(&mut messages, 1);
        assert!(removed > 0);
        // The last user message must survive
        assert!(messages.iter().any(|m| m["content"] == "Second question"));
    }

    #[test]
    fn tool_call_result_pairs_stay_together() {
        let mut messages = vec![
            msg("system", "System"),
            msg("user", "Q1"),
            assistant_with_tool_calls("thinking", "call_1"),
            tool_result("call_1", "result data"),
            msg("assistant", "Final answer from tool round"),
            msg("user", "Q2"),
        ];
        let removed = prune_messages(&mut messages, 1);
        // The tool call and result from the first group should both
        // be removed together (not orphaned).
        let has_tool_call = messages.iter().any(|m| m.get("tool_calls").is_some());
        let has_tool_result = messages.iter().any(|m| m["role"] == "tool");
        // Both should be gone, or both present
        assert_eq!(has_tool_call, has_tool_result);
    }

    #[test]
    fn no_pruning_when_under_budget() {
        let mut messages = vec![msg("system", "Hi"), msg("user", "Hello")];
        let removed = prune_messages(&mut messages, 1_000_000);
        assert_eq!(removed, 0);
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn empty_messages() {
        let mut messages: Vec<Value> = Vec::new();
        let removed = prune_messages(&mut messages, 100);
        assert_eq!(removed, 0);
        assert!(messages.is_empty());
    }
}
