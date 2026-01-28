use crate::chart::comparison::ComparisonChart;
use crate::screen::dashboard::pane::{Event, Message};

use iced::{Element, widget::{column, pane_grid}};

use super::common::cfg_view_container;

pub fn comparison_cfg_view<'a>(
    pane: pane_grid::Pane,
    chart: &'a ComparisonChart,
) -> Element<'a, Message> {
    let series = chart.series();
    let series_editor = chart.series_editor();

    let content = column![series_editor.view(series).map(move |msg| {
        Message::PaneEvent(
            pane,
            Event::ComparisonChartInteraction(crate::chart::comparison::Message::Editor(msg)),
        )
    })];

    cfg_view_container(320, content)
}
