//! Assistant text + streaming text rendering.

use crate::components::primitives::body;
use crate::screen::dashboard::pane::types::{AiAssistantEvent, Event, Message};
use crate::style::{self, tokens};
use iced::widget::pane_grid;
use iced::{
    Element, Length, Theme,
    widget::{button, container, mouse_area},
};

/// Plain text assistant message bubble with hover + right-click.
pub fn view_text_bubble_interactive<'a>(
    content: &'a str,
    index: usize,
    id: pane_grid::Pane,
) -> Element<'a, Message> {
    let inner = button(body(content).wrapping(iced::widget::text::Wrapping::WordOrGlyph))
        .style(style::button::chat_bubble)
        .padding([tokens::spacing::SM, tokens::spacing::MD])
        .on_press(Message::PaneEvent(id, Box::new(Event::DismissContextMenu)))
        .width(Length::Fill)
        .clip(false);

    mouse_area(inner)
        .on_right_press(Message::PaneEvent(
            id,
            Box::new(Event::AiAssistant(AiAssistantEvent::MessageRightClicked(
                index,
            ))),
        ))
        .into()
}

/// Plain text assistant message bubble (non-interactive, for streaming).
pub fn view_text_bubble<'a>(content: &'a str) -> Element<'a, Message> {
    container(body(content).wrapping(iced::widget::text::Wrapping::WordOrGlyph))
        .padding([tokens::spacing::SM, tokens::spacing::MD])
        .style(|theme: &Theme| {
            let p = theme.extended_palette();
            container::Style {
                background: Some(p.background.weak.color.into()),
                text_color: Some(p.background.weak.text),
                border: iced::Border {
                    radius: iced::border::radius(tokens::radius::MD),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .width(Length::Fill)
        .into()
}
