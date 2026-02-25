//! Kairos AI Assistant Backend
//!
//! Provides the AI assistant infrastructure for Kairos:
//!
//! - **client**: OpenRouter API client (chat completions, streaming)
//! - **conversation**: Conversation model, persistence, token tracking
//! - **context**: System prompt and chart context formatting
//! - **tools**: Tool definitions and execution (candles, trades,
//!   studies, annotations, chart actions)
//! - **service**: Streaming orchestrator with tool-call loop
//! - **error**: Unified error types

pub mod client;
pub mod context;
pub mod conversation;
pub mod error;
pub mod service;
pub mod tools;

pub use client::{ClientConfig, OpenRouterClient};
pub use conversation::{Conversation, ConversationMessage, Role};
pub use error::AiError;
pub use service::{AiService, AiStreamEvent};
pub use tools::{AiChartAction, ToolContext, ToolExecutor};
