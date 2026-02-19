use crate::screen::dashboard::pane::Message;
use crate::style;
use crate::component::display::tooltip::tooltip;
use crate::component::layout::scrollable_content::scrollable_content;

use data::state::pane_config::VisualConfig;

use iced::{
    Element, Length,
    widget::{button, container, pane_grid, tooltip::Position as TooltipPosition},
};

pub fn cfg_view_container<'a, T>(max_width: u32, content: T) -> Element<'a, Message>
where
    T: Into<Element<'a, Message>>,
{
    container(scrollable_content(content))
        .width(Length::Shrink)
        .padding(28)
        .max_width(max_width)
        .style(style::chart_modal)
        .into()
}

pub fn sync_all_button<'a>(pane: pane_grid::Pane, config: VisualConfig) -> Element<'a, Message> {
    tooltip(
        button("Sync all").on_press(Message::VisualConfigChanged(pane, config, true)),
        Some("Apply configuration to similar panes"),
        TooltipPosition::Top,
    )
}
