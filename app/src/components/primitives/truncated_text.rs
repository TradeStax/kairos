//! Truncated text that clips to a max width.

use iced::widget::{container, text};
use iced::{Element, Renderer, Theme};

use crate::style::tokens;

/// Text element that clips at `max_width` pixels without wrapping.
pub fn truncated<'a, Message: 'a>(
    content: &'a str,
    max_width: f32,
) -> Element<'a, Message, Theme, Renderer> {
    container(
        text(content)
            .size(tokens::text::BODY)
            .wrapping(text::Wrapping::None),
    )
    .max_width(max_width)
    .clip(true)
    .into()
}
