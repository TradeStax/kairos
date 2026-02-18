//! Themed tooltip helpers.

use iced::Element;
use iced::widget::{container, text, tooltip};

use crate::style;
use crate::style::tokens;

/// Tooltip with themed styling and no delay.
pub fn themed_tooltip<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    tip: Option<&'a str>,
    position: tooltip::Position,
) -> Element<'a, Message> {
    themed_tooltip_delayed(content, tip, position, std::time::Duration::ZERO)
}

/// Tooltip with themed styling and a custom delay.
pub fn themed_tooltip_delayed<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    tip: Option<&'a str>,
    position: tooltip::Position,
    delay: std::time::Duration,
) -> Element<'a, Message> {
    match tip {
        Some(tip_text) => tooltip(
            content,
            container(text(tip_text))
                .style(style::tooltip)
                .padding(tokens::spacing::MD),
            position,
        )
        .delay(delay)
        .into(),
        None => content.into(),
    }
}
