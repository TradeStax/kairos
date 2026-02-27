use super::context_menu::ContextMenuAction;
use super::types::{AiAssistantEvent, AiContextBubbleEvent};
use crate::{
    chart,
    modals::{self, pane::Modal},
    screen::dashboard::ladder,
};

use crate::screen::dashboard::pane::config::{ContentKind, LinkGroup, VisualConfig};
use iced::widget::pane_grid;

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
    PaneEvent(pane_grid::Pane, Box<Event>),
}

#[derive(Debug, Clone)]
pub enum Event {
    ShowModal(Modal),
    HideModal,
    ContentSelected(ContentKind),
    ChartInteraction(chart::Message),
    PanelInteraction(ladder::Message),
    ToggleStudy(String),
    DeleteNotification(usize),
    ReorderIndicator(crate::components::layout::reorderable_list::DragEvent),
    DataManagementInteraction(crate::modals::download::DataManagementMessage),
    #[cfg(feature = "heatmap")]
    StudyConfigurator(modals::pane::settings::StudyMessage),
    StreamModifierChanged(modals::stream::Message),
    ComparisonChartInteraction(chart::comparison::Message),
    MiniTickersListInteraction(modals::pane::tickers::Message),
    ContextMenuAction(ContextMenuAction),
    DismissContextMenu,
    DrawingPropertiesChanged(crate::modals::drawing::properties::Message),
    IndicatorManagerInteraction(crate::modals::pane::indicator::Message),
    OpenIndicatorManager,
    LevelDetailInteraction(crate::modals::pane::level_detail::Message),
    AiAssistant(AiAssistantEvent),
    AiContextBubble(AiContextBubbleEvent),
}
