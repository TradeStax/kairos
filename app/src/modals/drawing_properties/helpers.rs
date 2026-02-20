//! Standalone helper functions for drawing properties modal views.

use iced::{
    Color, Element, Length,
    widget::{button, container, row, space, text, text_input},
};
use palette::Hsva;

use crate::components::input::color_picker::color_picker;
use crate::style::{self, tokens};

use super::Message;

/// Option row: label text on the left, control on the right.
pub(super) fn option_row<'a>(
    label: &'a str,
    control: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    row![
        text(label).size(tokens::text::BODY),
        space::horizontal(),
        control.into(),
    ]
    .align_y(iced::Alignment::Center)
    .width(Length::Fill)
    .into()
}

/// Small color swatch button that toggles a color picker popup.
pub(super) fn color_swatch<'a>(
    color: Color,
    is_active: bool,
    on_press: Message,
) -> Element<'a, Message> {
    button(space::horizontal().width(22).height(22))
        .style(move |_theme, _status| button::Style {
            background: Some(color.into()),
            border: iced::border::rounded(3)
                .width(if is_active { 2.0 } else { 1.0 })
                .color(if is_active {
                    Color::WHITE
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.3)
                }),
            ..button::Style::default()
        })
        .padding(0)
        .on_press(on_press)
        .into()
}

/// Hex color text input with validation styling.
pub(super) fn hex_text_input<'a>(
    hex_value: &str,
    is_valid: bool,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    text_input("", hex_value)
        .on_input(on_input)
        .width(80)
        .style(move |theme: &iced::Theme, status| {
            let palette = theme.extended_palette();
            iced::widget::text_input::Style {
                border: iced::Border {
                    color: if is_valid {
                        palette.background.strong.color
                    } else {
                        palette.danger.base.color
                    },
                    width: tokens::border::THIN,
                    radius: tokens::radius::SM.into(),
                },
                ..iced::widget::text_input::default(theme, status)
            }
        })
        .into()
}

/// Compact square color picker popup.
pub(super) fn picker_popup<'a>(
    hsva: Hsva,
    on_color: impl Fn(Hsva) -> Message + Clone + 'a,
) -> Element<'a, Message> {
    container(color_picker(hsva, on_color, 180.0))
        .padding(tokens::spacing::SM)
        .style(style::dropdown_container)
        .into()
}
