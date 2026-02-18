//! Toggle switch wrapping Iced's `toggler` widget.

use iced::widget::{text, toggler};
use iced::{Element, Renderer, Theme};

use crate::style::tokens;

/// A labeled toggle switch.
pub fn toggle_switch<'a, Message: 'a + Clone>(
    label: impl text::IntoFragment<'a>,
    is_toggled: bool,
    on_toggle: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message, Theme, Renderer> {
    toggler(is_toggled)
        .label(label)
        .on_toggle(on_toggle)
        .size(tokens::layout::TOGGLER_SIZE)
        .spacing(tokens::spacing::MD)
        .into()
}
