//! Semantic text primitives that map to the design token type scale.

use iced::widget::{Text, text};
use iced::{Color, Renderer, Theme};

use super::AZERET_MONO;
use crate::style::tokens::text as text_tokens;

/// Large heading (16px) -- modal headings.
pub fn heading<'a>(content: impl text::IntoFragment<'a>) -> Text<'a, Theme, Renderer> {
    text(content).size(text_tokens::HEADING)
}

/// Title (14px) -- dialog titles, prominent text.
pub fn title<'a>(content: impl text::IntoFragment<'a>) -> Text<'a, Theme, Renderer> {
    text(content).size(text_tokens::TITLE)
}

/// Label (13px) -- form labels, section headers.
pub fn label_text<'a>(content: impl text::IntoFragment<'a>) -> Text<'a, Theme, Renderer> {
    text(content).size(text_tokens::LABEL)
}

/// Body (12px) -- default UI text.
pub fn body<'a>(content: impl text::IntoFragment<'a>) -> Text<'a, Theme, Renderer> {
    text(content).size(text_tokens::BODY)
}

/// Small (11px) -- chart labels, panel data.
pub fn small<'a>(content: impl text::IntoFragment<'a>) -> Text<'a, Theme, Renderer> {
    text(content).size(text_tokens::SMALL)
}

/// Tiny (10px) -- badges, compact labels.
pub fn tiny<'a>(content: impl text::IntoFragment<'a>) -> Text<'a, Theme, Renderer> {
    text(content).size(text_tokens::TINY)
}

/// Monospaced body text using Azeret Mono.
pub fn mono<'a>(content: impl text::IntoFragment<'a>) -> Text<'a, Theme, Renderer> {
    text(content).size(text_tokens::SMALL).font(AZERET_MONO)
}

/// Body text with an explicit color override.
pub fn colored<'a>(
    content: impl text::IntoFragment<'a>,
    color: Color,
) -> Text<'a, Theme, Renderer> {
    text(content).size(text_tokens::BODY).color(color)
}

/// Text element that clips at `max_width` pixels without wrapping.
pub fn truncated<'a, Message: 'a>(
    content: &'a str,
    max_width: f32,
) -> iced::Element<'a, Message, Theme, Renderer> {
    iced::widget::container(
        text(content)
            .size(text_tokens::BODY)
            .wrapping(text::Wrapping::None),
    )
    .max_width(max_width)
    .clip(true)
    .into()
}
