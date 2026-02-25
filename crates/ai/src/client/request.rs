//! OpenRouter / OpenAI-compatible request types.

use serde::{Deserialize, Serialize};

/// Top-level chat completion request body.
#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<RequestMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

/// A single message in the request.
#[derive(Debug, Clone, Serialize)]
pub struct RequestMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<RequestToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl RequestMessage {
    /// Create a system message.
    pub fn system(content: &str) -> Self {
        Self {
            role: "system".to_string(),
            content: Some(serde_json::Value::String(
                content.to_string(),
            )),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a user message (text only).
    pub fn user(content: &str) -> Self {
        Self {
            role: "user".to_string(),
            content: Some(serde_json::Value::String(
                content.to_string(),
            )),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create an assistant message (text only).
    pub fn assistant(content: &str) -> Self {
        Self {
            role: "assistant".to_string(),
            content: Some(serde_json::Value::String(
                content.to_string(),
            )),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create an assistant message with tool calls.
    pub fn assistant_tool_calls(
        calls: Vec<RequestToolCall>,
    ) -> Self {
        Self {
            role: "assistant".to_string(),
            content: None,
            tool_calls: Some(calls),
            tool_call_id: None,
        }
    }

    /// Create a tool result message.
    pub fn tool_result(
        tool_call_id: &str,
        content: &str,
    ) -> Self {
        Self {
            role: "tool".to_string(),
            content: Some(serde_json::Value::String(
                content.to_string(),
            )),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.to_string()),
        }
    }
}

/// A tool call included in an assistant response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

/// The function name + arguments string for a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    /// JSON-encoded arguments string.
    pub arguments: String,
}

/// Tool definition for function calling.
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

/// Schema of a callable function.
#[derive(Debug, Clone, Serialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    /// JSON Schema describing the parameters object.
    pub parameters: serde_json::Value,
}
