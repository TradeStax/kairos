//! Status indicator dot and badge.
//!
//! Note: `status_color()` (ConnectionStatus -> Color mapping) lives in the consuming modals
//! (`modals/data_feeds/view.rs`, `modals/connections/mod.rs`) -- it is not part of
//! this generic component library because it depends on `data::ConnectionStatus`.

use iced::widget::{container, row, text};
use iced::{Border, Color, Element, Renderer, Theme};

use crate::style::tokens;

/// A small colored circle with theme-derived color.
pub fn status_dot_themed<'a, Message: 'a>(
    color_fn: impl Fn(&Theme) -> Color + 'a,
) -> Element<'a, Message, Theme, Renderer> {
    container(iced::widget::Space::new())
        .width(tokens::component::status_dot::SIZE)
        .height(tokens::component::status_dot::SIZE)
        .style(move |theme: &Theme| container::Style {
            background: Some(color_fn(theme).into()),
            border: Border {
                radius: tokens::radius::ROUND.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

/// Theme-aware colored dot followed by a label.
pub fn status_badge_themed<'a, Message: 'a>(
    color_fn: impl Fn(&Theme) -> Color + 'a,
    label: &'a str,
) -> Element<'a, Message, Theme, Renderer> {
    row![
        status_dot_themed(color_fn),
        text(label).size(tokens::text::SMALL),
    ]
    .spacing(tokens::spacing::XS)
    .align_y(iced::Alignment::Center)
    .into()
}
