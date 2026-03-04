use super::Content;
use super::context_menu::ContextMenuKind;
use crate::components::display::toast::Toast;
use crate::config::UserTimezone;
use crate::modals::pane::Modal;
use crate::screen::dashboard::pane::config::{LinkGroup, Settings};
use ai::ApiMessage;
use data::FuturesTickerInfo;
use data::{ChartData, LoadingStatus};

// Re-export AI types from ai module so external code can use either path.
pub use super::ai::{
    AI_MODELS, ActiveContext, AiAssistantEvent, AiAssistantState, AiContextBubble,
    AiContextBubbleEvent, AiContextSummary, TickAction, model_display_name, model_id_from_name,
};

// Re-export Event and Message from messages module so existing code
// that imports `pane::types::Event` / `pane::types::Message` continues to work.
pub use super::messages::{Event, Message};

pub struct State {
    id: uuid::Uuid,
    pub(crate) modal: Option<Modal>,
    pub(crate) content: Content,
    pub(crate) settings: Settings,
    pub(crate) notifications: Vec<Toast>,
    pub(crate) loading_status: LoadingStatus,
    pub(crate) ticker_info: Option<FuturesTickerInfo>,
    pub(crate) chart_data: Option<ChartData>,
    pub(crate) link_group: Option<LinkGroup>,
    /// Tracks which feed provided this pane's data (set when chart data loads)
    pub(crate) feed_id: Option<data::FeedId>,
    /// Backup of chart data before replay (restored on stop)
    pub(crate) replay_backup: Option<ChartData>,
    /// Active right-click context menu
    pub(crate) context_menu: Option<ContextMenuKind>,
    /// The date range that was used to load this pane's data
    pub(crate) loaded_date_range: Option<data::DateRange>,
    /// Floating AI context bubble (shown after AiContext drawing completes)
    pub(crate) ai_context_bubble: Option<AiContextBubble>,
    /// Live trades received while chart was still loading (drained on chart init)
    pub(crate) pending_live_trades: Vec<data::Trade>,
    /// User timezone (synced from Kairos before update dispatch)
    pub(crate) timezone: UserTimezone,
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_config(
        content: Content,
        settings: Settings,
        link_group: Option<LinkGroup>,
        ticker_info: Option<FuturesTickerInfo>,
    ) -> Self {
        Self {
            content,
            settings,
            ticker_info,
            link_group,
            ..Default::default()
        }
    }

    pub fn get_ticker(&self) -> Option<FuturesTickerInfo> {
        self.ticker_info
    }

    pub fn unique_id(&self) -> uuid::Uuid {
        self.id
    }

    /// Set the remote crosshair interval on the chart's ViewState.
    /// Only clears the crosshair cache if the value actually changed.
    pub fn set_remote_crosshair(&mut self, interval: Option<u64>) {
        use crate::chart::Chart;
        let state = match &mut self.content {
            Content::Candlestick { chart, .. } => {
                if let Some(c) = (**chart).as_mut() {
                    c.mut_state()
                } else {
                    return;
                }
            }
            #[cfg(feature = "heatmap")]
            Content::Heatmap { chart: Some(c), .. } => c.mut_state(),
            Content::Profile { chart, .. } => {
                if let Some(c) = (**chart).as_mut() {
                    c.mut_state()
                } else {
                    return;
                }
            }
            _ => return,
        };
        if state.crosshair.remote != interval {
            state.crosshair.remote = interval;
            state.cache.clear_crosshair();
        }
    }

    /// Get the AI conversation ID if this pane is an AI assistant.
    pub fn ai_conversation_id(&self) -> Option<uuid::Uuid> {
        match &self.content {
            Content::AiAssistant(state) => Some(state.conversation_id),
            _ => None,
        }
    }

    /// Handle an AI stream event, routing to the AI state.
    pub fn handle_ai_event(&mut self, event: crate::app::core::globals::AiStreamEvent) {
        if let Content::AiAssistant(state) = &mut self.content {
            state.handle_event(event);
        }
    }

    /// Start AI streaming: returns (model, conversation_id,
    /// api_history) or None.
    ///
    /// When `display_text` is `Some`, only it is shown in the chat
    /// bubble while the full `user_message` goes to the API.
    pub fn ai_start_streaming(
        &mut self,
        user_message: &str,
        display_text: Option<&str>,
    ) -> Option<(String, uuid::Uuid, Vec<ApiMessage>)> {
        if let Content::AiAssistant(state) = &mut self.content {
            Some(state.start_streaming(user_message, display_text))
        } else {
            None
        }
    }

    /// Returns the scrollable widget Id for the AI chat list, if any.
    pub fn ai_scroll_id(&self) -> Option<iced::widget::Id> {
        if let Content::AiAssistant(state) = &self.content {
            Some(state.scroll_id.clone())
        } else {
            None
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            modal: None,
            content: Content::Starter,
            settings: Settings::default(),
            notifications: vec![],
            loading_status: LoadingStatus::Idle,
            ticker_info: None,
            chart_data: None,
            link_group: None,
            feed_id: None,
            replay_backup: None,
            context_menu: None,
            loaded_date_range: None,
            ai_context_bubble: None,
            pending_live_trades: Vec::new(),
            timezone: UserTimezone::Utc,
        }
    }
}
