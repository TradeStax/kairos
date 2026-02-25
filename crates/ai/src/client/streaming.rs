//! SSE stream parser for OpenRouter streaming responses.

use crate::error::AiError;
use futures::Stream;
use serde::Deserialize;

/// A single streamed chunk from the SSE response.
#[derive(Debug, Clone, Deserialize)]
pub struct StreamChunk {
    pub id: String,
    pub choices: Vec<StreamChoice>,
    #[serde(default)]
    pub usage: Option<super::response::Usage>,
}

/// Delta-based choice in a stream chunk.
#[derive(Debug, Clone, Deserialize)]
pub struct StreamChoice {
    pub index: u32,
    pub delta: StreamDelta,
    pub finish_reason: Option<String>,
}

/// Incremental message delta.
#[derive(Debug, Clone, Deserialize)]
pub struct StreamDelta {
    pub role: Option<String>,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<StreamToolCallDelta>>,
}

/// Incremental tool call delta.
#[derive(Debug, Clone, Deserialize)]
pub struct StreamToolCallDelta {
    pub index: u32,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub function: Option<StreamFunctionDelta>,
    #[serde(default, rename = "type")]
    pub call_type: Option<String>,
}

/// Incremental function name/arguments delta.
#[derive(Debug, Clone, Deserialize)]
pub struct StreamFunctionDelta {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

/// Parse an SSE stream from a `reqwest::Response` into a
/// `futures::Stream` of `StreamChunk` items.
///
/// The stream reads the response body, splits on double newlines,
/// strips the `data: ` prefix, skips `[DONE]`, and parses JSON.
pub fn parse_sse_stream(
    response: reqwest::Response,
) -> impl Stream<Item = Result<StreamChunk, AiError>> {
    use futures::StreamExt;

    let byte_stream = response.bytes_stream();

    // Buffer for incomplete SSE lines across byte boundaries.
    let mut buffer = String::new();

    futures::stream::unfold(
        (byte_stream, buffer),
        |(mut stream, mut buf)| async move {
            loop {
                // Try to extract a complete SSE event from buffer.
                if let Some(event_end) =
                    buf.find("\n\n")
                {
                    let event =
                        buf[..event_end].to_string();
                    buf =
                        buf[event_end + 2..].to_string();

                    if let Some(chunk) =
                        parse_sse_event(&event)
                    {
                        return Some((
                            chunk,
                            (stream, buf),
                        ));
                    }
                    // If this event didn't yield a chunk
                    // (e.g. comment, empty), keep looping.
                    continue;
                }

                // Need more data from the byte stream.
                use futures::StreamExt as _;
                match stream.next().await {
                    Some(Ok(bytes)) => {
                        let text =
                            String::from_utf8_lossy(&bytes);
                        buf.push_str(&text);
                    }
                    Some(Err(e)) => {
                        return Some((
                            Err(AiError::Streaming(
                                e.to_string(),
                            )),
                            (stream, buf),
                        ));
                    }
                    None => {
                        // Stream ended. Try to flush any
                        // remaining event in buffer.
                        if !buf.trim().is_empty() {
                            let event =
                                std::mem::take(&mut buf);
                            if let Some(chunk) =
                                parse_sse_event(&event)
                            {
                                return Some((
                                    chunk,
                                    (stream, buf),
                                ));
                            }
                        }
                        return None;
                    }
                }
            }
        },
    )
}

/// Parse a single SSE event block (potentially multi-line).
/// Returns `None` for non-data events, comments, and `[DONE]`.
fn parse_sse_event(
    event: &str,
) -> Option<Result<StreamChunk, AiError>> {
    for line in event.lines() {
        let line = line.trim();
        if let Some(data) = line.strip_prefix("data:") {
            let data = data.trim();
            if data == "[DONE]" {
                return None;
            }
            if data.is_empty() {
                continue;
            }
            return Some(
                serde_json::from_str::<StreamChunk>(data)
                    .map_err(|e| {
                        AiError::Streaming(format!(
                            "JSON parse error: {e} \
                             — data: {prefix}",
                            prefix =
                                &data[..data.len().min(200)]
                        ))
                    }),
            );
        }
    }
    None
}
