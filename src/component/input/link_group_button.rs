use iced::widget::{button, text};
use iced::{Alignment, Element, Theme};

use crate::component::primitives::AZERET_MONO;
use crate::style;

pub fn link_group_button<'a, Message, F>(
    id: iced::widget::pane_grid::Pane,
    link_group: Option<data::layout::pane::LinkGroup>,
    on_press: F,
) -> Element<'a, Message>
where
    Message: Clone + 'static,
    F: Fn(iced::widget::pane_grid::Pane) -> Message + 'static,
{
    let is_active = link_group.is_some();

    let icon = if let Some(group) = link_group {
        text(group.to_string())
            .font(AZERET_MONO)
            .align_x(Alignment::Start)
            .align_y(Alignment::Center)
    } else {
        text("-")
            .font(AZERET_MONO)
            .align_x(Alignment::Start)
            .align_y(Alignment::Center)
    };

    button(icon)
        .style(move |theme: &Theme, status| {
            style::button::bordered_toggle(theme, status, is_active)
        })
        .on_press(on_press(id))
        .width(28)
        .into()
}
