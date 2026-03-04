//! AI stream events and drawing actions.

/// A single tool call + result pair, used to sync back to api_history.
#[derive(Debug, Clone)]
pub struct ToolRoundSync {
    pub call_id: String,
    pub name: String,
    pub arguments: String,
    pub result_json: String,
}

/// Clone-safe AI stream event for passing through channels.
#[derive(Debug, Clone)]
pub enum AiStreamEvent {
    Delta {
        conversation_id: uuid::Uuid,
        text: String,
    },
    ToolCallStarted {
        conversation_id: uuid::Uuid,
        call_id: String,
        name: String,
        arguments_json: String,
        display_summary: String,
    },
    ToolCallResult {
        conversation_id: uuid::Uuid,
        call_id: String,
        name: String,
        content_json: String,
        display_summary: String,
        is_error: bool,
    },
    /// Marks the end of a text segment (before tool calls start).
    TextSegmentComplete {
        conversation_id: uuid::Uuid,
    },
    Complete {
        conversation_id: uuid::Uuid,
        prompt_tokens: u32,
        completion_tokens: u32,
    },
    Error {
        conversation_id: uuid::Uuid,
        error: String,
    },
    /// Sync tool call rounds back to the pane's api_history so
    /// follow-up messages include prior tool context.
    ApiHistorySync {
        conversation_id: uuid::Uuid,
        rounds: Vec<ToolRoundSync>,
        /// Final assistant text (if any) produced after all tool rounds.
        final_text: Option<String>,
    },
    ApiKeyMissing {
        conversation_id: uuid::Uuid,
    },
    /// AI-initiated drawing action to be applied on the main thread.
    DrawingAction {
        conversation_id: uuid::Uuid,
        action: Box<DrawingAction>,
    },
}

/// AI-initiated drawing action — decoupled from app drawing types.
#[derive(Debug, Clone)]
pub enum DrawingAction {
    Add {
        spec: Box<DrawingSpec>,
        description: String,
    },
    Remove {
        id: String,
        description: String,
    },
    RemoveAll {
        description: String,
    },
}

/// Flat specification for a drawing to be created.
/// The app layer converts this into its native `SerializableDrawing`.
#[derive(Debug, Clone)]
pub struct DrawingSpec {
    pub tool_name: String,
    pub price: Option<f64>,
    pub price_high: Option<f64>,
    pub price_low: Option<f64>,
    pub time_millis: Option<u64>,
    pub from_price: Option<f64>,
    pub from_time_millis: Option<u64>,
    pub to_price: Option<f64>,
    pub to_time_millis: Option<u64>,
    pub label: Option<String>,
    pub text: Option<String>,
    pub color: Option<data::SerializableColor>,
    pub line_style: Option<String>,
    pub opacity: Option<f32>,
    pub drawing_id: Option<String>,
    /// Tick size from snapshot, for price-to-units conversion
    pub tick_size: f32,
    /// Fibonacci config (for fib retracement)
    pub fibonacci: bool,
}

impl Default for DrawingSpec {
    fn default() -> Self {
        Self {
            tool_name: String::new(),
            price: None,
            price_high: None,
            price_low: None,
            time_millis: None,
            from_price: None,
            from_time_millis: None,
            to_price: None,
            to_time_millis: None,
            label: None,
            text: None,
            color: None,
            line_style: None,
            opacity: None,
            drawing_id: None,
            tick_size: 0.0,
            fibonacci: false,
        }
    }
}
