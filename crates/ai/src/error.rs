use thiserror::Error;

/// Unified error type for the AI crate.
#[derive(Error, Debug, Clone)]
pub enum AiError {
    #[error("API request failed: {0}")]
    ApiRequest(String),

    #[error("Streaming error: {0}")]
    Streaming(String),

    #[error("Tool execution failed: {tool}: {message}")]
    ToolExecution { tool: String, message: String },

    #[error("Context building failed: {0}")]
    ContextBuild(String),

    #[error("API key not configured")]
    ApiKeyMissing,

    #[error("Rate limited, retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },

    #[error("Model not available: {0}")]
    ModelUnavailable(String),

    #[error("Conversation error: {0}")]
    Conversation(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Max tool call rounds exceeded ({max_rounds})")]
    MaxToolRounds { max_rounds: u32 },
}
