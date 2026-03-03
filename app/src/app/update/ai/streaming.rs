//! SSE Parser + Agentic Loop
//!
//! Streams chat completions from OpenRouter, handles tool call
//! accumulation, executes tools against a ChartSnapshot, and loops
//! for multi-step reasoning (max 10 rounds).

use std::collections::HashMap;

use futures::StreamExt as _;
use serde_json::{Value, json};

use super::tools::{self, ToolContext};
use crate::app::core::globals::{AiStreamEventClone, ToolRoundSync};
use data::domain::assistant::{ApiMessage, ChartSnapshot, ChatRole};

const MAX_TOOL_ROUNDS: usize = 10;

// ── Tool call accumulator ─────────────────────────────────────────

struct PendingToolCall {
    id: String,
    name: String,
    arguments_buf: String,
}

struct ToolCallAccumulator {
    pending: HashMap<usize, PendingToolCall>,
}

impl ToolCallAccumulator {
    fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    /// Process a tool_calls delta chunk. Returns true if a new call
    /// was started (has an `id` field).
    fn process_delta(&mut self, tool_calls: &Value) -> bool {
        let Some(arr) = tool_calls.as_array() else {
            return false;
        };
        let mut new_call = false;
        for tc in arr {
            let idx = tc["index"].as_u64().unwrap_or(0) as usize;

            if let Some(id) = tc["id"].as_str() {
                // New tool call
                let name = tc["function"]["name"].as_str().unwrap_or("").to_string();
                self.pending.insert(
                    idx,
                    PendingToolCall {
                        id: id.to_string(),
                        name,
                        arguments_buf: String::new(),
                    },
                );
                new_call = true;
            }

            if let (Some(args_frag), Some(entry)) = (
                tc["function"]["arguments"].as_str(),
                self.pending.get_mut(&idx),
            ) {
                entry.arguments_buf.push_str(args_frag);
            }
        }
        new_call
    }

    /// Drain all pending tool calls into a finalized list.
    fn finalize(&mut self) -> Vec<(String, String, String)> {
        let mut result: Vec<(String, String, String)> = self
            .pending
            .drain()
            .map(|(_, tc)| (tc.id, tc.name, tc.arguments_buf))
            .collect();
        // Sort by id for deterministic ordering
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }
}

// ── Agentic streaming loop ────────────────────────────────────────

/// Stream a chat completion with agentic tool calling.
///
/// Loops up to `MAX_TOOL_ROUNDS` times. Each round:
/// 1. POST chat/completions with messages + tools
/// 2. Stream SSE, emit Delta events, accumulate tool calls
/// 3. On `stop` → emit Complete, return
/// 4. On `tool_calls` → execute tools, append to history, loop
pub(crate) async fn stream_openrouter_agentic(
    api_key: String,
    model: String,
    mut api_messages: Vec<Value>,
    tools_json: Value,
    conversation_id: uuid::Uuid,
    sender: &'static tokio::sync::mpsc::UnboundedSender<AiStreamEventClone>,
    snapshot: Option<ChartSnapshot>,
    temperature: f32,
    max_tokens: u32,
    timezone: crate::config::UserTimezone,
) {
    use AiStreamEventClone as Ev;

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = sender.send(Ev::Error {
                conversation_id,
                error: format!("HTTP client error: {}", e),
            });
            return;
        }
    };

    let has_tools = tools_json.as_array().is_some_and(|a| !a.is_empty());
    let mut total_prompt_tokens: u32 = 0;
    let mut total_completion_tokens: u32 = 0;
    let mut all_tool_rounds: Vec<ToolRoundSync> = Vec::new();

    for _round in 0..MAX_TOOL_ROUNDS {
        let mut body = json!({
            "model": model,
            "messages": api_messages,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "stream": true,
            "stream_options": { "include_usage": true }
        });

        if has_tools {
            body["tools"] = tools_json.clone();
        }

        let response = match client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://kairos.app")
            .header("X-Title", "Kairos")
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let _ = sender.send(Ev::Error {
                    conversation_id,
                    error: format!("Network error: {}", e),
                });
                return;
            }
        };

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let msg = match response.json::<Value>().await {
                Ok(json) => json["error"]["message"]
                    .as_str()
                    .unwrap_or("Unknown API error")
                    .to_string(),
                Err(_) => "Unknown API error".to_string(),
            };
            let _ = sender.send(Ev::Error {
                conversation_id,
                error: format!("API error {}: {}", status, msg),
            });
            return;
        }

        // Parse SSE stream for this round
        let mut byte_stream = response.bytes_stream();
        let mut line_buf = String::new();
        let mut text_buf = String::new();
        let mut accumulator = ToolCallAccumulator::new();
        let mut finish_reason: Option<String> = None;

        'stream: loop {
            match byte_stream.next().await {
                None => break 'stream,
                Some(Err(e)) => {
                    let _ = sender.send(Ev::Error {
                        conversation_id,
                        error: format!("Stream error: {}", e),
                    });
                    return;
                }
                Some(Ok(chunk)) => {
                    line_buf.push_str(&String::from_utf8_lossy(&chunk));

                    while let Some(nl) = line_buf.find('\n') {
                        let line = line_buf[..nl].trim_end_matches('\r').to_owned();
                        line_buf = line_buf[nl + 1..].to_owned();

                        let Some(data) = line.strip_prefix("data: ") else {
                            continue;
                        };

                        if data.trim() == "[DONE]" {
                            break 'stream;
                        }

                        let Ok(json) = serde_json::from_str::<Value>(data) else {
                            continue;
                        };

                        // Text delta
                        if let Some(text) = json["choices"][0]["delta"]["content"]
                            .as_str()
                            .filter(|t| !t.is_empty())
                        {
                            text_buf.push_str(text);
                            let _ = sender.send(Ev::Delta {
                                conversation_id,
                                text: text.to_owned(),
                            });
                        }

                        // Tool call deltas
                        if let Some(tcs) = json["choices"][0]["delta"].get("tool_calls") {
                            accumulator.process_delta(tcs);
                        }

                        // Finish reason
                        if let Some(fr) = json["choices"][0]["finish_reason"].as_str() {
                            finish_reason = Some(fr.to_string());
                        }

                        // Usage
                        if let Some(usage) = json.get("usage").filter(|u| !u.is_null()) {
                            total_prompt_tokens =
                                usage["prompt_tokens"].as_u64().unwrap_or(0) as u32;
                            total_completion_tokens =
                                usage["completion_tokens"].as_u64().unwrap_or(0) as u32;
                        }
                    }
                }
            }
        }

        // Handle finish_reason
        match finish_reason.as_deref() {
            Some("tool_calls") => {
                // Commit text segment if non-empty
                if !text_buf.is_empty() {
                    let _ = sender.send(Ev::TextSegmentComplete { conversation_id });
                }

                let tool_calls = accumulator.finalize();
                if tool_calls.is_empty() {
                    // Unexpected: finish_reason=tool_calls but no calls
                    let _ = sender.send(Ev::Complete {
                        conversation_id,
                        prompt_tokens: total_prompt_tokens,
                        completion_tokens: total_completion_tokens,
                    });
                    return;
                }

                // Build the assistant message with tool_calls for
                // the API history
                let api_tool_calls: Vec<Value> = tool_calls
                    .iter()
                    .map(|(id, name, args)| {
                        json!({
                            "id": id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": args,
                            }
                        })
                    })
                    .collect();

                let mut assistant_msg = json!({
                    "role": "assistant",
                });
                if !text_buf.is_empty() {
                    assistant_msg["content"] = json!(text_buf);
                }
                assistant_msg["tool_calls"] = json!(api_tool_calls);
                api_messages.push(assistant_msg);

                // Execute each tool call
                for (call_id, name, args) in &tool_calls {
                    let summary = tool_call_summary(name, args);
                    let _ = sender.send(Ev::ToolCallStarted {
                        conversation_id,
                        call_id: call_id.clone(),
                        name: name.clone(),
                        arguments_json: args.clone(),
                        display_summary: summary,
                    });

                    let tool_ctx = ToolContext {
                        snapshot: &snapshot,
                        sender,
                        conversation_id,
                        timezone,
                    };
                    let result = tools::execute_tool(name, args, &tool_ctx);

                    let _ = sender.send(Ev::ToolCallResult {
                        conversation_id,
                        call_id: call_id.clone(),
                        name: name.clone(),
                        content_json: result.content_json.clone(),
                        display_summary: result.display_summary.clone(),
                        is_error: result.is_error,
                    });

                    // Track for api_history sync
                    all_tool_rounds.push(ToolRoundSync {
                        call_id: call_id.clone(),
                        name: name.clone(),
                        arguments: args.clone(),
                        result_json: result.content_json.clone(),
                    });

                    // Append tool result to API messages
                    api_messages.push(json!({
                        "role": "tool",
                        "tool_call_id": call_id,
                        "content": result.content_json,
                    }));
                }

                // Reset text buffer for next round
                text_buf.clear();
                continue; // Next round
            }
            _ => {
                // "stop" or end of stream — sync history then complete
                if !all_tool_rounds.is_empty() {
                    let final_text = if text_buf.is_empty() {
                        None
                    } else {
                        Some(text_buf.clone())
                    };
                    let _ = sender.send(Ev::ApiHistorySync {
                        conversation_id,
                        rounds: std::mem::take(&mut all_tool_rounds),
                        final_text,
                    });
                }
                let _ = sender.send(Ev::Complete {
                    conversation_id,
                    prompt_tokens: total_prompt_tokens,
                    completion_tokens: total_completion_tokens,
                });
                return;
            }
        }
    }

    // Max rounds exceeded
    let _ = sender.send(Ev::Error {
        conversation_id,
        error: "Max tool calling rounds exceeded".to_string(),
    });
}

/// Build a brief display summary for a tool call.
fn tool_call_summary(name: &str, args: &str) -> String {
    let parsed: Value = serde_json::from_str(args).unwrap_or(json!({}));

    match name {
        // Query tools
        "get_chart_info" => "Querying chart info".to_string(),
        "get_candles" => {
            let count = parsed["count"].as_u64().unwrap_or(50);
            format!("Fetching {} candles", count)
        }
        "get_market_state" => "Checking market state".to_string(),
        "get_trades" => {
            if let (Some(lo), Some(hi)) =
                (parsed["price_min"].as_f64(), parsed["price_max"].as_f64())
            {
                format!("Trades at {:.2}-{:.2}", lo, hi)
            } else {
                "Analyzing trade data".to_string()
            }
        }
        "get_volume_profile" => "Building volume profile".to_string(),
        "get_study_values" => {
            let count = parsed["count"].as_u64().unwrap_or(10);
            format!("Reading {} study values", count)
        }
        "get_delta_profile" => "Analyzing delta profile".to_string(),
        "get_big_trades" => {
            let count = parsed["count"].as_u64().unwrap_or(50);
            format!("Fetching {} big trades", count)
        }
        "get_footprint" => {
            let count = parsed["count"].as_u64().unwrap_or(20);
            format!("Reading {} footprint candles", count)
        }
        "get_profile_data" => "Loading VBP profile data".to_string(),
        "get_aggregated_trades" => {
            let bucket = parsed["bucket_seconds"].as_u64().unwrap_or(60);
            format!("Aggregating trades ({}s buckets)", bucket)
        }
        "get_drawings" => "Listing chart drawings".to_string(),
        "get_session_stats" => {
            let session = parsed["session"].as_str().unwrap_or("rth").to_uppercase();
            format!("{} session stats", session)
        }
        "identify_levels" => "Identifying support/resistance".to_string(),
        // Drawing tools
        "add_horizontal_line" => {
            if let Some(p) = parsed["price"].as_f64() {
                format!("Drawing line at {:.2}", p)
            } else {
                "Drawing horizontal line".to_string()
            }
        }
        "add_vertical_line" => parsed["label"]
            .as_str()
            .map(|l| format!("V-Line: {}", l))
            .unwrap_or_else(|| "Drawing vertical line".to_string()),
        "add_text_annotation" => "Adding text annotation".to_string(),
        "add_price_level" => {
            if let Some(p) = parsed["price"].as_f64() {
                format!("Marking level at {:.2}", p)
            } else {
                "Adding price level".to_string()
            }
        }
        "add_price_label" => {
            if let Some(p) = parsed["price"].as_f64() {
                format!("Price label at {:.2}", p)
            } else {
                "Adding price label".to_string()
            }
        }
        "add_line" => "Drawing line segment".to_string(),
        "add_extended_line" => "Drawing extended line".to_string(),
        "add_rectangle" => "Drawing rectangle zone".to_string(),
        "add_ellipse" => "Drawing ellipse".to_string(),
        "add_arrow" => "Drawing arrow".to_string(),
        "add_fib_retracement" => {
            if let (Some(hi), Some(lo)) =
                (parsed["high_price"].as_f64(), parsed["low_price"].as_f64())
            {
                format!("Fib {:.2}-{:.2}", lo, hi)
            } else {
                "Drawing fib retracement".to_string()
            }
        }
        "remove_drawing" => "Removing drawing".to_string(),
        "remove_all_drawings" => "Clearing all drawings".to_string(),
        other => format!("Calling {}", other),
    }
}

// ── API message building ──────────────────────────────────────────

/// Convert stored API history into JSON messages for the API request.
/// Prepends system prompt and optional chart context.
pub(crate) fn build_api_messages(
    system_prompt: &str,
    api_history: &[ApiMessage],
    snapshot: &Option<ChartSnapshot>,
) -> Vec<Value> {
    let mut out = vec![json!({
        "role": "system",
        "content": system_prompt,
    })];

    // Brief chart context as a second system message
    if let Some(snap) = snapshot {
        let studies = if snap.active_studies.is_empty() {
            "none".to_string()
        } else {
            snap.active_studies.join(", ")
        };
        let live = if snap.is_live { "LIVE" } else { "Historical" };
        let date_range = snap
            .date_range_display
            .as_ref()
            .map(|(s, e)| format!("{} to {}", s, e))
            .unwrap_or_default();
        let ctx = format!(
            "Chart context: {} {} {} | {} candles | {} trades{} | \
             studies: {} | {} | range: {} | tz: {}",
            snap.ticker,
            snap.timeframe,
            snap.chart_type,
            snap.candles.len(),
            snap.trades.len(),
            if snap.trades_truncated {
                " (truncated)"
            } else {
                ""
            },
            studies,
            live,
            date_range,
            snap.timezone,
        );
        out.push(json!({
            "role": "system",
            "content": ctx,
        }));
    }

    // Append user/assistant/tool messages
    for msg in api_history {
        let role = match msg.role {
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
            ChatRole::Tool => "tool",
            ChatRole::System => "system",
        };

        let mut obj = json!({ "role": role });

        if let Some(ref content) = msg.content {
            obj["content"] = json!(content);
        }

        if let Some(ref tool_calls) = msg.tool_calls {
            let tcs: Vec<Value> = tool_calls
                .iter()
                .map(|tc| {
                    json!({
                        "id": tc.id,
                        "type": tc.call_type,
                        "function": {
                            "name": tc.function.name,
                            "arguments": tc.function.arguments,
                        }
                    })
                })
                .collect();
            obj["tool_calls"] = json!(tcs);
        }

        if let Some(ref tool_call_id) = msg.tool_call_id {
            obj["tool_call_id"] = json!(tool_call_id);
        }

        out.push(obj);
    }

    out
}
