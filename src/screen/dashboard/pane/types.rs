use super::Content;
use crate::{
    chart,
    component::display::toast::Toast,
    modal::{self, pane::Modal},
    screen::dashboard::panel,
};
use data::{
    ChartConfig, ChartData, ContentKind, LinkGroup, LoadingStatus, Settings, UiIndicator,
    VisualConfig,
};
use exchange::FuturesTickerInfo;
use iced::widget::pane_grid;

pub enum Action {
    Chart(chart::Action),
    Panel(panel::Action),
    LoadChart {
        config: ChartConfig,
        ticker_info: FuturesTickerInfo,
    },
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
    ToggleIndicator(UiIndicator),
    DeleteNotification(usize),
    ReorderIndicator(crate::component::layout::reorderable_list::DragEvent),
    DataManagementInteraction(crate::modal::pane::download::DataManagementMessage),
    ClusterKindSelected(data::ClusterKind),
    ClusterScalingSelected(data::ClusterScaling),
    StudyConfigurator(modal::pane::settings::study::StudyMessage),
    StreamModifierChanged(modal::stream::Message),
    ComparisonChartInteraction(chart::comparison::Message),
    MiniTickersListInteraction(modal::pane::tickers::Message),
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
        }
    }
}
