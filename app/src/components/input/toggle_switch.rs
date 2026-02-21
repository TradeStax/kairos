//! Toggle switch wrapping Iced's `toggler` widget.

use iced::widget::{row, space, text, toggler};
use iced::{Alignment, Element, Length, Renderer, Theme};

use crate::style::tokens;

/// A labeled toggle switch (label left, toggle right-aligned).
pub fn toggle_switch<'a, Message: 'a + Clone>(
    label: impl text::IntoFragment<'a>,
    is_toggled: bool,
    on_toggle: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message, Theme, Renderer> {
    let label_widget = text(label).size(tokens::text::LABEL);

    let switch = toggler(is_toggled)
        .on_toggle(on_toggle)
        .size(tokens::layout::TOGGLER_SIZE);

    row![label_widget, space::horizontal(), switch]
        .spacing(tokens::spacing::MD)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
}
