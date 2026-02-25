use super::Content;
use crate::{
    chart,
    components::display::toast::Toast,
    modals::{self, pane::Modal},
    screen::dashboard::panel,
};

use data::{
    ChartData, ContentKind, DrawingId, LinkGroup,
    LoadingStatus, Settings, VisualConfig,
    domain::assistant::ApiMessage,
};
use exchange::FuturesTickerInfo;
use iced::{Point, widget::pane_grid};

// Re-export AI types from ai_state so external code can use either path.
pub use super::ai_state::{
    AI_MODELS, ActiveContext, AiAssistantEvent, AiAssistantState,
    AiContextBubble, AiContextBubbleEvent, AiContextSummary,
    TickAction, model_display_name, model_id_from_name,
};

/// What was right-clicked on the chart
#[derive(Debug, Clone)]
pub enum ContextMenuKind {
    /// Right-clicked empty chart area
    Chart { position: Point },
    /// Right-clicked a specific drawing
    Drawing {
        position: Point,
        id: DrawingId,
        locked: bool,
    },
    /// Right-clicked a study overlay label
    StudyOverlay {
        position: Point,
        study_index: usize,
    },
    /// Right-clicked an AI assistant message
    AiMessage {
        position: Point,
        message_index: usize,
    },
}

impl ContextMenuKind {
    pub fn position(&self) -> Point {
        match self {
            ContextMenuKind::Chart { position }
            | ContextMenuKind::Drawing { position, .. }
            | ContextMenuKind::StudyOverlay { position, .. }
            | ContextMenuKind::AiMessage { position, .. } => *position,
        }
    }
}

/// Actions available from chart context menu
#[derive(Debug, Clone)]
pub enum ContextMenuAction {
    RebuildChart,
    CenterLastPrice,
    OpenIndicators,
    DeleteDrawing(DrawingId),
    ToggleLockDrawing(DrawingId),
    CloneDrawing(DrawingId),
    OpenDrawingProperties(DrawingId),
    OpenStudyProperties(usize),
    CopyAiMessageText(usize),
}

#[derive(Debug, Clone)]
pub enum Message {
    PaneClicked(pane_grid::Pane),
    PaneResized(pane_grid::ResizeEvent),
    PaneDragged(pane_grid::DragEvent),
    ClosePane(pane_grid::Pane),
    SplitPane(pane_grid::Axis, pane_grid::Pane),
    MaximizePane(pane_grid::Pane),
    Restore,
    ReplacePane(pane_grid::Pane),
    Popout,
    Merge,
    SwitchLinkGroup(pane_grid::Pane, Option<LinkGroup>),
    VisualConfigChanged(pane_grid::Pane, VisualConfig, bool),
    PaneEvent(pane_grid::Pane, Event),
}

#[derive(Debug, Clone)]
pub enum Event {
    ShowModal(Modal),
    HideModal,
    ContentSelected(ContentKind),
    ChartInteraction(chart::Message),
    PanelInteraction(panel::Message),
    ToggleStudy(String),
    DeleteNotification(usize),
    ReorderIndicator(crate::components::layout::reorderable_list::DragEvent),
    DataManagementInteraction(crate::modals::download::DataManagementMessage),
    StudyConfigurator(modals::pane::settings::StudyMessage),
    StreamModifierChanged(modals::stream::Message),
    ComparisonChartInteraction(chart::comparison::Message),
    MiniTickersListInteraction(modals::pane::tickers::Message),
    ContextMenuAction(ContextMenuAction),
    DismissContextMenu,
    DrawingPropertiesChanged(crate::modals::drawing::properties::Message),
    IndicatorManagerInteraction(crate::modals::pane::indicator::Message),
    OpenIndicatorManager,
    AiAssistant(AiAssistantEvent),
    AiContextBubble(AiContextBubbleEvent),
}

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
            Content::Candlestick { chart: Some(c), .. } => c.mut_state(),
            Content::Heatmap { chart: Some(c), .. } => c.mut_state(),
            Content::Profile { chart: Some(c), .. } => c.mut_state(),
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
    pub fn handle_ai_event(
        &mut self,
        event: crate::app::core::globals::AiStreamEventClone,
    ) {
        if let Content::AiAssistant(state) = &mut self.content {
            state.handle_event(event);
        }
    }

    /// Start AI streaming: returns (model, conversation_id,
    /// api_history) or None.
    pub fn ai_start_streaming(
        &mut self,
        user_message: &str,
    ) -> Option<(String, uuid::Uuid, Vec<ApiMessage>)> {
        if let Content::AiAssistant(state) = &mut self.content {
            Some(state.start_streaming(user_message))
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
        }
    }
}
