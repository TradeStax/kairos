//! Visual separators and spacers.

use iced::widget::{Space, rule};
use iced::{Element, Length, Renderer, Theme};

/// Thin horizontal divider (1px) styled with `split_ruler`.
pub fn divider<'a, Message: 'a>() -> Element<'a, Message, Theme, Renderer> {
    rule::horizontal(1).style(crate::style::split_ruler).into()
}

/// Thin vertical divider (1px) styled with `split_ruler`.
pub fn vertical_divider<'a, Message: 'a>() -> Element<'a, Message, Theme, Renderer> {
    rule::vertical(1).style(crate::style::split_ruler).into()
}

/// Flexible space that fills available room along the main axis.
pub fn flex_space<'a, Message: 'a>() -> Element<'a, Message, Theme, Renderer> {
    Space::new()
        .width(Length::Fill)
        .height(Length::Shrink)
        .into()
}
