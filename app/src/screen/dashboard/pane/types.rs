use super::Content;
use crate::{
    chart,
    components::display::toast::Toast,
    modals::{self, pane::Modal},
    screen::dashboard::panel,
};
use data::{
    ChartConfig, ChartData, ContentKind, DrawingId, LinkGroup, LoadingStatus, Settings, VisualConfig,
};
use exchange::FuturesTickerInfo;
use iced::{Point, widget::pane_grid};

pub enum Action {
    Chart(chart::Action),
    Panel(panel::Action),
    LoadChart {
        config: ChartConfig,
        ticker_info: FuturesTickerInfo,
    },
}

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
}

impl ContextMenuKind {
    pub fn position(&self) -> Point {
        match self {
            ContextMenuKind::Chart { position }
            | ContextMenuKind::Drawing { position, .. } => *position,
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
    StudyConfigurator(modals::pane::settings::study::StudyMessage),
    StreamModifierChanged(modals::stream::Message),
    ComparisonChartInteraction(chart::comparison::Message),
    MiniTickersListInteraction(modals::pane::tickers::Message),
    ContextMenuAction(ContextMenuAction),
    DismissContextMenu,
    DrawingPropertiesChanged(crate::modals::drawing_properties::Message),
    IndicatorManagerInteraction(crate::modals::pane::indicator_manager::Message),
    OpenIndicatorManager,
}

pub struct State {
    id: uuid::Uuid,
    pub modal: Option<Modal>,
    pub content: Content,
    pub settings: Settings,
    pub notifications: Vec<Toast>,
    pub loading_status: LoadingStatus,
    pub ticker_info: Option<FuturesTickerInfo>,
    pub chart_data: Option<ChartData>,
    pub link_group: Option<LinkGroup>,
    /// Tracks which feed provided this pane's data (set when chart data loads)
    pub feed_id: Option<data::FeedId>,
    /// Backup of chart data before replay (restored on stop)
    pub replay_backup: Option<ChartData>,
    /// Active right-click context menu
    pub context_menu: Option<ContextMenuKind>,
    /// The date range that was used to load this pane's data
    pub loaded_date_range: Option<data::DateRange>,
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
            Content::Kline { chart: Some(c), .. } => c.mut_state(),
            Content::Heatmap { chart: Some(c), .. } => c.mut_state(),
            Content::Profile { chart: Some(c), .. } => c.mut_state(),
            _ => return,
        };
        if state.remote_crosshair != interval {
            state.remote_crosshair = interval;
            state.cache.clear_crosshair();
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
        }
    }
}
