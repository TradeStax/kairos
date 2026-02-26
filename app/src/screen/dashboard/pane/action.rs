use crate::drawing::DrawingTool;
use data::{ChartConfig, DateRange, FuturesTicker, FuturesTickerInfo};

#[derive(Debug, Clone)]
pub enum Action {
    LoadChart {
        config: ChartConfig,
        ticker_info: FuturesTickerInfo,
    },
    SwitchTickersInGroup(FuturesTickerInfo),
    FocusWidget(iced::widget::Id),
    EstimateDataCost {
        ticker: FuturesTicker,
        schema: data::DownloadSchema,
        date_range: DateRange,
    },
    DownloadData {
        ticker: FuturesTicker,
        schema: data::DownloadSchema,
        date_range: DateRange,
    },
    /// Drawing tool was auto-changed (e.g. after completing a drawing)
    DrawingToolChanged(DrawingTool),
    /// Crosshair position changed in a linked pane
    CrosshairSync {
        timestamp: Option<u64>,
    },
    /// AI assistant wants to send a message
    AiRequest {
        pane_id: uuid::Uuid,
        user_message: String,
    },
    /// Save an OpenRouter API key from the in-pane credential modal
    SaveAiApiKey(String),
    /// AI context query from a chart drawing selection
    AiContextQuery {
        source_pane_id: uuid::Uuid,
        /// Structured chart context (system-level, shown as context card)
        context: String,
        /// User's question (shown as user message bubble)
        question: String,
    },
    /// AI preferences changed (persist to saved state)
    AiPreferencesChanged {
        model: String,
        temperature: f32,
        max_tokens: u32,
    },
    /// Copy text to system clipboard
    CopyToClipboard(String),
}
