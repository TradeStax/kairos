//! AI Assistant Domain Types
//!
//! Chat message types, tool call records, and token usage for the
//! AI assistant panel. Pure domain types with no I/O or framework deps.

use serde::{Deserialize, Serialize};

use super::entities::{Candle, Trade};

/// Role in a chat conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatRole {
    User,
    Assistant,
    System,
    Tool,
}

/// Typed message union for the UI — each variant maps to a distinct
/// visual bubble in the chat view.
#[derive(Debug, Clone)]
pub enum ChatMessageKind {
    User {
        text: String,
    },
    AssistantText {
        text: String,
    },
    ToolCall {
        call_id: String,
        name: String,
        arguments_json: String,
        display_summary: String,
    },
    ToolResult {
        call_id: String,
        name: String,
        content_json: String,
        display_summary: String,
        is_error: bool,
    },
    Thinking {
        text: String,
    },
    ContextAttachment {
        ticker: String,
        timeframe: String,
        chart_type: String,
        candle_count: usize,
        is_live: bool,
    },
    SystemNotice {
        text: String,
        is_error: bool,
    },
}

/// A single display message in the chat UI.
#[derive(Debug, Clone)]
pub struct DisplayMessage {
    pub kind: ChatMessageKind,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl DisplayMessage {
    pub fn new(kind: ChatMessageKind) -> Self {
        Self {
            kind,
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Wire-format message for the OpenAI/OpenRouter API.
#[derive(Debug, Clone)]
pub struct ApiMessage {
    pub role: ChatRole,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ApiToolCall>>,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ApiToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiToolCallFunction {
    pub name: String,
    pub arguments: String,
}

/// Snapshot of a single study's output for AI tool access.
#[derive(Debug, Clone)]
pub struct StudyOutputSnapshot {
    pub study_id: String,
    pub study_name: String,
    /// Line series: (label, Vec<(time_secs, value)>)
    pub line_values: Vec<(String, Vec<(u64, f32)>)>,
    /// Bar series: (label, Vec<(time_secs, value)>)
    pub bar_values: Vec<(String, Vec<(u64, f32)>)>,
    /// Horizontal levels: (label, price)
    pub levels: Vec<(String, f64)>,
}

/// Per-price-level trade data within a footprint candle snapshot.
#[derive(Debug, Clone)]
pub struct FootprintLevelSnapshot {
    pub price: f64,
    pub buy_volume: f32,
    pub sell_volume: f32,
}

/// Per-candle footprint data snapshot for AI tool access.
#[derive(Debug, Clone)]
pub struct FootprintCandleSnapshot {
    pub time_secs: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub poc_price: Option<f64>,
    pub levels: Vec<FootprintLevelSnapshot>,
}

/// A single level within a profile snapshot.
#[derive(Debug, Clone)]
pub struct ProfileLevelSnapshot {
    pub price: f64,
    pub buy_volume: f32,
    pub sell_volume: f32,
}

/// Volume profile snapshot for AI tool access.
#[derive(Debug, Clone)]
pub struct ProfileSnapshot {
    pub levels: Vec<ProfileLevelSnapshot>,
    pub poc_price: Option<f64>,
    pub value_area_high: Option<f64>,
    pub value_area_low: Option<f64>,
    pub total_volume: f64,
    pub hvn_prices: Vec<f64>,
    pub lvn_prices: Vec<f64>,
    pub time_range: Option<(u64, u64)>,
}

/// A single point in a drawing snapshot.
#[derive(Debug, Clone)]
pub struct DrawingPointSnapshot {
    pub price: f64,
    pub time_secs: u64,
}

/// Snapshot of a chart drawing for AI tool access.
#[derive(Debug, Clone)]
pub struct DrawingSnapshot {
    pub id: String,
    pub tool_type: String,
    pub points: Vec<DrawingPointSnapshot>,
    pub label: Option<String>,
    pub visible: bool,
    pub locked: bool,
}

/// Snapshot of a big trade marker for AI tool access.
#[derive(Debug, Clone)]
pub struct BigTradeSnapshot {
    pub time_secs: u64,
    pub price: f64,
    pub quantity: f64,
    pub is_buy: bool,
}

/// Immutable snapshot of chart data captured at request time so the
/// async streaming function needs no mutable access to pane state.
#[derive(Debug, Clone)]
pub struct ChartSnapshot {
    pub ticker: String,
    pub tick_size: f32,
    pub contract_size: f32,
    pub timeframe: String,
    pub chart_type: String,
    pub is_live: bool,
    pub candles: Vec<Candle>,
    pub trades: Vec<Trade>,
    pub trades_truncated: bool,
    pub active_studies: Vec<String>,
    pub date_range_display: Option<(String, String)>,
    pub study_snapshots: Vec<StudyOutputSnapshot>,
    pub big_trade_markers: Vec<BigTradeSnapshot>,
    pub timezone: String,
    // Extended snapshot data
    pub footprint_candles: Vec<FootprintCandleSnapshot>,
    pub profile_snapshots: Vec<ProfileSnapshot>,
    pub drawing_snapshots: Vec<DrawingSnapshot>,
    pub visible_price_high: Option<f64>,
    pub visible_price_low: Option<f64>,
    pub visible_time_start: Option<u64>,
    pub visible_time_end: Option<u64>,
}

/// A single message in the chat — used for backward compat in the
/// AI context query flow. Prefer `DisplayMessage` for new code.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ChatMessage {
    pub fn user(content: String) -> Self {
        Self {
            role: ChatRole::User,
            content,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn assistant(content: String) -> Self {
        Self {
            role: ChatRole::Assistant,
            content,
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Session-total token usage for status bar display.
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl TokenUsage {
    pub fn add(&mut self, prompt: u32, completion: u32) {
        self.prompt_tokens += prompt;
        self.completion_tokens += completion;
        self.total_tokens += prompt + completion;
    }

    pub fn format_display(&self) -> String {
        if self.total_tokens == 0 {
            return String::new();
        }
        format_token_count(self.total_tokens)
    }
}

fn format_token_count(count: u32) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M tokens", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K tokens", count as f64 / 1_000.0)
    } else {
        format!("{} tokens", count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_usage_display() {
        let mut usage = TokenUsage::default();
        assert_eq!(usage.format_display(), "");

        usage.add(1000, 500);
        assert_eq!(usage.total_tokens, 1500);
        assert_eq!(usage.format_display(), "1.5K tokens");
    }

    #[test]
    fn test_chat_message_constructors() {
        let msg = ChatMessage::user("hello".to_string());
        assert_eq!(msg.role, ChatRole::User);
        assert_eq!(msg.content, "hello");
    }

    #[test]
    fn test_display_message() {
        let msg = DisplayMessage::new(ChatMessageKind::User {
            text: "hello".to_string(),
        });
        assert!(matches!(msg.kind, ChatMessageKind::User { .. }));
    }
}
