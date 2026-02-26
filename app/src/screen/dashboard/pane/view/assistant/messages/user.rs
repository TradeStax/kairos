//! User message bubble rendering.

use crate::components::primitives::{body, separator::flex_space};
use crate::screen::dashboard::pane::types::{AiAssistantEvent, Event, Message};
use crate::style::{self, tokens};
use iced::widget::pane_grid;
use iced::{Element, widget::{button, container, mouse_area}};

pub fn view_user_bubble<'a>(
    content: &'a str,
    index: usize,
    id: pane_grid::Pane,
) -> Element<'a, Message> {
    let inner = button(body(content).wrapping(iced::widget::text::Wrapping::WordOrGlyph))
        .style(style::button::chat_bubble_user)
        .padding([tokens::spacing::SM, tokens::spacing::MD])
        .on_press(Message::PaneEvent(id, Box::new(Event::DismissContextMenu)))
        .clip(false);

    let with_ctx = mouse_area(inner).on_right_press(Message::PaneEvent(
        id,
        Box::new(Event::AiAssistant(AiAssistantEvent::MessageRightClicked(
            index,
        ))),
    ));

    iced::widget::row![flex_space(), container(with_ctx).max_width(500)].into()
}
