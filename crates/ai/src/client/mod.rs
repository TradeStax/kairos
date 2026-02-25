//! OpenRouter API client.
//!
//! Provides HTTP transport for chat completions (blocking and
//! streaming) against the OpenRouter `/api/v1/chat/completions`
//! endpoint, which is OpenAI-compatible.

mod config;
pub mod request;
pub mod response;
pub mod streaming;

pub use config::{AiModel, ClientConfig};
pub use request::{
    ChatCompletionRequest, FunctionCall, FunctionDefinition,
    RequestMessage, RequestToolCall, ToolDefinition,
};
pub use response::{
    ChatCompletionResponse, Choice, ResponseMessage, Usage,
};
pub use streaming::{StreamChunk, StreamChoice, StreamDelta};

use crate::error::AiError;
use futures::Stream;

/// OpenRouter API client.
#[derive(Debug, Clone)]
pub struct OpenRouterClient {
    http: reqwest::Client,
    config: ClientConfig,
}

impl OpenRouterClient {
    /// Create a new client with the given configuration.
    pub fn new(config: ClientConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(
                config.timeout_secs,
            ))
            .build()
            .expect("failed to build reqwest client");

        Self { http, config }
    }

    /// Non-streaming chat completion.
    pub async fn chat_completion(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, AiError> {
        let url =
            format!("{}/chat/completions", self.config.base_url);

        let resp = self
            .http
            .post(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.api_key),
            )
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://kairos.com")
            .header("X-Title", "Kairos")
            .json(request)
            .send()
            .await
            .map_err(|e| AiError::ApiRequest(e.to_string()))?;

        let status = resp.status();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(30);
            return Err(AiError::RateLimited {
                retry_after_secs: retry,
            });
        }

        if !status.is_success() {
            let body = resp
                .text()
                .await
                .unwrap_or_else(|_| "no body".to_string());
            return Err(AiError::ApiRequest(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let body = resp.text().await.map_err(|e| {
            AiError::ApiRequest(format!(
                "failed to read response: {}",
                e
            ))
        })?;

        serde_json::from_str::<ChatCompletionResponse>(&body)
            .map_err(|e| {
                AiError::Serialization(format!(
                    "response parse: {e} — body: \
                     {body_prefix}",
                    body_prefix =
                        &body[..body.len().min(500)]
                ))
            })
    }

    /// Streaming chat completion. Returns an async stream of
    /// `StreamChunk` items parsed from SSE.
    pub async fn stream_completion(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<
        impl Stream<Item = Result<StreamChunk, AiError>>,
        AiError,
    > {
        // Ensure the request is marked for streaming
        let mut req = request.clone();
        req.stream = Some(true);

        let url =
            format!("{}/chat/completions", self.config.base_url);

        let resp = self
            .http
            .post(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.api_key),
            )
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://kairos.com")
            .header("X-Title", "Kairos")
            .json(&req)
            .send()
            .await
            .map_err(|e| AiError::ApiRequest(e.to_string()))?;

        let status = resp.status();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(30);
            return Err(AiError::RateLimited {
                retry_after_secs: retry,
            });
        }

        if !status.is_success() {
            let body = resp
                .text()
                .await
                .unwrap_or_else(|_| "no body".to_string());
            return Err(AiError::ApiRequest(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        Ok(streaming::parse_sse_stream(resp))
    }

    /// Access the underlying configuration.
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }
}
