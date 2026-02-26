use iced::Point;

/// In-progress tool call during streaming.
pub struct StreamingToolCall {
    pub call_id: String,
    pub name: String,
    pub display_summary: String,
    pub result_summary: Option<String>,
    pub is_error: bool,
    pub is_complete: bool,
}

/// Context snapshot shown as a chip above the input when a chart is linked.
pub struct ActiveContext {
    pub ticker: String,
    pub timeframe: String,
    pub chart_type: String,
    pub candle_count: usize,
    pub is_live: bool,
}

/// State for the floating AI context bubble shown after drawing an
/// AiContext rectangle on a chart.
#[derive(Debug, Clone)]
pub struct AiContextBubble {
    pub drawing_id: crate::drawing::DrawingId,
    pub input_text: String,
    pub range_summary: AiContextSummary,
    /// Screen position for the bubble (bottom-center of the drawing rect)
    pub anchor: Point,
}

/// Summary of the selected chart region for the AI context bubble.
#[derive(Debug, Clone)]
pub struct AiContextSummary {
    pub ticker: String,
    pub timeframe: String,
    pub time_start_fmt: String,
    pub time_end_fmt: String,
    pub price_high: String,
    pub price_low: String,
    pub candle_count: usize,
    pub total_volume: String,
    pub net_delta: String,
    /// Pre-formatted OHLCV lines (capped at 50 candles)
    pub candle_ohlcv_lines: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum AiContextBubbleEvent {
    InputChanged(String),
    Submit,
    Dismiss,
}

#[derive(Debug, Clone)]
pub enum AiAssistantEvent {
    InputChanged(String),
    SendMessage,
    StopStreaming,
    ToggleSettings,
    ModelChanged(String),
    TemperatureChanged(f32),
    MaxTokensChanged(u32),
    RetryLastMessage,
    ClearHistory,
    ApiKeyInputChanged(String),
    SaveApiKey,
    DismissApiKeyModal,
    OpenUrl(String),
    CursorMoved(Point),
    MessageRightClicked(usize),
    ToggleThinking(usize),
}
