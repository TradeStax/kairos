//! Themed tooltip helpers.

use iced::widget::{button, container, text};
use iced::{Element, Theme};

use crate::style;
use crate::style::tokens;

/// Position alias for tooltip placement.
pub type TooltipPosition = iced::widget::tooltip::Position;

/// Default delay before showing tooltips.
pub const DEFAULT_TOOLTIP_DELAY: std::time::Duration = std::time::Duration::from_millis(500);

/// Tooltip with themed styling and no delay.
pub fn tooltip<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    tip: Option<&'a str>,
    position: iced::widget::tooltip::Position,
) -> Element<'a, Message> {
    tooltip_with_delay(content, tip, position, std::time::Duration::ZERO)
}

/// Tooltip with themed styling and a custom delay.
pub fn tooltip_with_delay<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    tip: Option<&'a str>,
    position: iced::widget::tooltip::Position,
    delay: std::time::Duration,
) -> Element<'a, Message> {
    match tip {
        Some(tip_text) => iced::widget::tooltip(
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

/// Button wrapped in a themed tooltip.
pub fn button_with_tooltip<'a, M: Clone + 'a>(
    content: impl Into<Element<'a, M>>,
    message: M,
    tooltip_text: Option<&'a str>,
    tooltip_pos: TooltipPosition,
    style_fn: impl Fn(&Theme, button::Status) -> button::Style + 'static,
) -> Element<'a, M> {
    let btn = button(content).style(style_fn).on_press(message);

    if let Some(text) = tooltip_text {
        tooltip(btn, Some(text), tooltip_pos)
    } else {
        btn.into()
    }
}
