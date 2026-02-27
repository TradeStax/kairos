//! AI assistant domain types.
//!
//! Chat message types, tool call records, chart snapshots, and token usage
//! for the AI assistant panel. Pure domain types with no I/O or framework deps.

use serde::{Deserialize, Serialize};

use crate::domain::market::entities::{Candle, Trade};

// ── Chat Roles and Messages ─────────────────────────────────────────────

/// Role in a chat conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatRole {
    /// User input
    User,
    /// Assistant response
    Assistant,
    /// System prompt
    System,
    /// Tool call result
    Tool,
}

/// Typed message kind for the UI — each variant maps to a distinct
/// visual bubble in the chat view.
#[derive(Debug, Clone)]
pub enum ChatMessageKind {
    /// User-authored text
    User {
        /// The user's message text
        text: String,
    },
    /// Assistant-authored text response
    AssistantText {
        /// The assistant's response text
        text: String,
    },
    /// A tool call made by the assistant
    ToolCall {
        /// Unique identifier for this tool call
        call_id: String,
        /// Tool function name
        name: String,
        /// JSON-encoded arguments
        arguments_json: String,
        /// Human-readable summary for the UI
        display_summary: String,
    },
    /// Result returned from a tool call
    ToolResult {
        /// Matching call identifier
        call_id: String,
        /// Tool function name
        name: String,
        /// JSON-encoded result content
        content_json: String,
        /// Human-readable summary for the UI
        display_summary: String,
        /// Whether the tool call resulted in an error
        is_error: bool,
    },
    /// Assistant thinking / chain-of-thought
    Thinking {
        /// The thinking text
        text: String,
    },
    /// Attached chart context sent alongside a user message
    ContextAttachment {
        /// Ticker symbol
        ticker: String,
        /// Timeframe label
        timeframe: String,
        /// Chart type label
        chart_type: String,
        /// Number of candles included
        candle_count: usize,
        /// Whether the chart has live data
        is_live: bool,
    },
    /// System notice or error banner
    SystemNotice {
        /// Notice text
        text: String,
        /// Whether this is an error notice
        is_error: bool,
    },
}

/// A single display message in the chat UI.
#[derive(Debug, Clone)]
pub struct DisplayMessage {
    /// Message content and type
    pub kind: ChatMessageKind,
    /// When the message was created
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl DisplayMessage {
    /// Create a new display message with the current timestamp
    #[must_use]
    pub fn new(kind: ChatMessageKind) -> Self {
        Self {
            kind,
            timestamp: chrono::Utc::now(),
        }
    }
}

// ── API Wire Format ─────────────────────────────────────────────────────

/// Wire-format message for the OpenAI/OpenRouter API.
#[derive(Debug, Clone)]
pub struct ApiMessage {
    /// Message role
    pub role: ChatRole,
    /// Text content (may be absent for tool-call messages)
    pub content: Option<String>,
    /// Tool calls requested by the assistant
    pub tool_calls: Option<Vec<ApiToolCall>>,
    /// Tool call ID (present when role is `Tool`)
    pub tool_call_id: Option<String>,
}

/// A single tool call in the API wire format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiToolCall {
    /// Unique call identifier
    pub id: String,
    /// Call type (always `"function"`)
    #[serde(rename = "type")]
    pub call_type: String,
    /// Function name and arguments
    pub function: ApiToolCallFunction,
}

/// Function name and JSON arguments within an [`ApiToolCall`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiToolCallFunction {
    /// Function name
    pub name: String,
    /// JSON-encoded arguments string
    pub arguments: String,
}

// ── Chart Snapshot Types ────────────────────────────────────────────────

/// Snapshot of a single study's output for AI tool access.
#[derive(Debug, Clone)]
pub struct StudyOutputSnapshot {
    /// Study instance identifier
    pub study_id: String,
    /// Human-readable study name
    pub study_name: String,
    /// Line series: `(label, Vec<(time_secs, value)>)`
    pub line_values: Vec<(String, Vec<(u64, f32)>)>,
    /// Bar series: `(label, Vec<(time_secs, value)>)`
    pub bar_values: Vec<(String, Vec<(u64, f32)>)>,
    /// Horizontal levels: `(label, price)`
    pub levels: Vec<(String, f64)>,
}

/// Per-price-level trade data within a footprint candle snapshot.
#[derive(Debug, Clone)]
pub struct FootprintLevelSnapshot {
    /// Price level (display units)
    pub price: f64,
    /// Buy-side volume at this level
    pub buy_volume: f32,
    /// Sell-side volume at this level
    pub sell_volume: f32,
}

/// Per-candle footprint data snapshot for AI tool access.
#[derive(Debug, Clone)]
pub struct FootprintCandleSnapshot {
    /// Candle timestamp in seconds since epoch
    pub time_secs: u64,
    /// Open price
    pub open: f64,
    /// High price
    pub high: f64,
    /// Low price
    pub low: f64,
    /// Close price
    pub close: f64,
    /// Point-of-control price, if computed
    pub poc_price: Option<f64>,
    /// Per-level footprint data
    pub levels: Vec<FootprintLevelSnapshot>,
}

/// A single level within a volume profile snapshot.
#[derive(Debug, Clone)]
pub struct ProfileLevelSnapshot {
    /// Price level (display units)
    pub price: f64,
    /// Buy-side volume
    pub buy_volume: f32,
    /// Sell-side volume
    pub sell_volume: f32,
}

/// Volume profile snapshot for AI tool access.
#[derive(Debug, Clone)]
pub struct ProfileSnapshot {
    /// Per-level profile data
    pub levels: Vec<ProfileLevelSnapshot>,
    /// Point-of-control price
    pub poc_price: Option<f64>,
    /// Value area high price
    pub value_area_high: Option<f64>,
    /// Value area low price
    pub value_area_low: Option<f64>,
    /// Total volume across the profile
    pub total_volume: f64,
    /// High volume node prices
    pub hvn_prices: Vec<f64>,
    /// Low volume node prices
    pub lvn_prices: Vec<f64>,
    /// Time range `(start_secs, end_secs)` of the profile
    pub time_range: Option<(u64, u64)>,
}

/// A single anchor point in a drawing snapshot.
#[derive(Debug, Clone)]
pub struct DrawingPointSnapshot {
    /// Price coordinate (display units)
    pub price: f64,
    /// Time coordinate in seconds since epoch
    pub time_secs: u64,
}

/// Snapshot of a chart drawing for AI tool access.
#[derive(Debug, Clone)]
pub struct DrawingSnapshot {
    /// Drawing identifier
    pub id: String,
    /// Drawing tool type name
    pub tool_type: String,
    /// Anchor points
    pub points: Vec<DrawingPointSnapshot>,
    /// Optional text label
    pub label: Option<String>,
    /// Whether the drawing is visible
    pub visible: bool,
    /// Whether the drawing is locked from editing
    pub locked: bool,
}

/// Snapshot of a big trade marker for AI tool access.
///
/// `time` is the marker's raw X coordinate: millisecond timestamp for
/// time-based charts, or reverse candle index for tick-based charts.
#[derive(Debug, Clone)]
pub struct BigTradeSnapshot {
    /// Raw X coordinate (see type docs for meaning)
    pub time: u64,
    /// Trade price (display units)
    pub price: f64,
    /// Trade quantity
    pub quantity: f64,
    /// `true` if the aggressor was a buyer
    pub is_buy: bool,
}

/// Immutable snapshot of chart data captured at request time so the
/// async streaming function needs no mutable access to pane state.
#[derive(Debug, Clone)]
pub struct ChartSnapshot {
    /// Ticker symbol
    pub ticker: String,
    /// Tick size for price formatting
    pub tick_size: f32,
    /// Contract multiplier
    pub contract_size: f32,
    /// Timeframe label
    pub timeframe: String,
    /// Chart type label
    pub chart_type: String,
    /// Whether the chart is receiving live data
    pub is_live: bool,
    /// Candle data
    pub candles: Vec<Candle>,
    /// Raw trade data
    pub trades: Vec<Trade>,
    /// Whether the trade vec was truncated for size
    pub trades_truncated: bool,
    /// Active study names
    pub active_studies: Vec<String>,
    /// Date range as `(start_display, end_display)`
    pub date_range_display: Option<(String, String)>,
    /// Study output snapshots
    pub study_snapshots: Vec<StudyOutputSnapshot>,
    /// Big trade markers
    pub big_trade_markers: Vec<BigTradeSnapshot>,
    /// Timezone label
    pub timezone: String,
    /// Footprint candle data
    pub footprint_candles: Vec<FootprintCandleSnapshot>,
    /// Volume profile snapshots
    pub profile_snapshots: Vec<ProfileSnapshot>,
    /// Drawing snapshots
    pub drawing_snapshots: Vec<DrawingSnapshot>,
    /// Visible price range high
    pub visible_price_high: Option<f64>,
    /// Visible price range low
    pub visible_price_low: Option<f64>,
    /// Visible time range start (seconds since epoch)
    pub visible_time_start: Option<u64>,
    /// Visible time range end (seconds since epoch)
    pub visible_time_end: Option<u64>,
    /// Whether the chart uses tick-based aggregation
    pub is_tick_basis: bool,
}

// ── Legacy ChatMessage ──────────────────────────────────────────────────

/// A simple chat message used for backward compatibility in the AI
/// context query flow. Prefer [`DisplayMessage`] for new code.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// Message role
    pub role: ChatRole,
    /// Message text content
    pub content: String,
    /// When the message was created
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ChatMessage {
    /// Create a user message with the current timestamp
    #[must_use]
    pub fn user(content: String) -> Self {
        Self {
            role: ChatRole::User,
            content,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create an assistant message with the current timestamp
    #[must_use]
    pub fn assistant(content: String) -> Self {
        Self {
            role: ChatRole::Assistant,
            content,
            timestamp: chrono::Utc::now(),
        }
    }
}

// ── Token Usage ─────────────────────────────────────────────────────────

/// Session-total token usage for status bar display.
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    /// Total prompt (input) tokens
    pub prompt_tokens: u32,
    /// Total completion (output) tokens
    pub completion_tokens: u32,
    /// Total tokens (prompt + completion)
    pub total_tokens: u32,
}

impl TokenUsage {
    /// Accumulate tokens from a single API response
    pub fn add(&mut self, prompt: u32, completion: u32) {
        self.prompt_tokens += prompt;
        self.completion_tokens += completion;
        self.total_tokens += prompt + completion;
    }

    /// Format for display (e.g. `"1.5K tokens"`), or empty string if zero
    #[must_use]
    pub fn format_display(&self) -> String {
        if self.total_tokens == 0 {
            return String::new();
        }
        format_token_count(self.total_tokens)
    }
}

/// Format a token count with K/M suffixes.
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
