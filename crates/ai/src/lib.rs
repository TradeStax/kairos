//! AI assistant engine for Kairos.
//!
//! This crate owns all GUI-independent AI logic:
//! - **domain** — chat messages, chart snapshots, conversation state
//! - **event** — stream events and drawing actions
//! - **client** — OpenRouter streaming client and model config
//! - **tools** — tool definitions, execution, and timezone abstraction
//! - **prompt** — system prompt construction

pub mod client;
pub mod domain;
pub mod event;
pub mod prompt;
pub mod tools;

// Re-export key types at crate root for ergonomic access
pub use client::config::{AI_MODELS, ModelOption, model_display_name, model_id_from_name};
pub use client::streaming::{build_api_messages, stream_openrouter_agentic};
pub use domain::conversation::{ActiveContext, AiConversation, StreamingToolCall};
pub use domain::messages::{
    ApiMessage, ApiToolCall, ApiToolCallFunction, ChatMessageKind, ChatRole, DisplayMessage,
    TokenUsage,
};
pub use domain::snapshot::*;
pub use event::{AiStreamEvent, DrawingAction, DrawingSpec, ToolRoundSync};
pub use prompt::build_system_prompt;
pub use tools::{TimezoneResolver, ToolContext, ToolExecResult, build_tools_json, execute_tool};
