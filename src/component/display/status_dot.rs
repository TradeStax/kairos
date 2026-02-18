//! Status indicator dot and badge.

use iced::widget::{container, row, text};
use iced::{Border, Color, Element, Renderer, Theme};

use crate::style::tokens;

/// A small colored circle (8x8) indicating status.
pub fn status_dot<'a, Message: 'a>(color: Color) -> Element<'a, Message, Theme, Renderer> {
    container(iced::widget::Space::new())
        .width(8)
        .height(8)
        .style(move |_theme: &Theme| container::Style {
            background: Some(color.into()),
            border: Border {
                radius: tokens::radius::ROUND.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

/// Colored dot followed by a label.
pub fn status_badge<'a, Message: 'a>(
    color: Color,
    label: &'a str,
) -> Element<'a, Message, Theme, Renderer> {
    row![status_dot(color), text(label).size(tokens::text::SMALL),]
        .spacing(tokens::spacing::XS)
        .align_y(iced::Alignment::Center)
        .into()
}

/// Colored dot + label + optional detail text on the right.
pub fn status_row<'a, Message: 'a>(
    color: Color,
    label: &'a str,
    detail: Option<&'a str>,
) -> Element<'a, Message, Theme, Renderer> {
    let mut r = row![status_dot(color), text(label).size(tokens::text::SMALL),]
        .spacing(tokens::spacing::XS)
        .align_y(iced::Alignment::Center);

    if let Some(d) = detail {
        r = r.push(
            text(d)
                .size(tokens::text::TINY)
                .color(Color::from_rgb(0.5, 0.5, 0.5)),
        );
    }

    r.into()
}
