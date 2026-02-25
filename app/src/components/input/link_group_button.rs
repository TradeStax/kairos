use iced::widget::{button, text};
use iced::{Alignment, Element, Theme};

use crate::components::primitives::AZERET_MONO;
use crate::style::{self, tokens};

pub fn link_group_button<'a, Message>(
    display_label: Option<String>,
    is_active: bool,
    on_press: Message,
) -> Element<'a, Message>
where
    Message: Clone + 'static,
{
    let label = display_label.unwrap_or_else(|| "-".to_string());

    let icon = text(label)
        .font(AZERET_MONO)
        .align_x(Alignment::Start)
        .align_y(Alignment::Center);

    button(icon)
        .style(move |theme: &Theme, status| {
            style::button::bordered_toggle(theme, status, is_active)
        })
        .on_press(on_press)
        .width(tokens::component::button::LINK_GROUP_WIDTH)
        .into()
}
