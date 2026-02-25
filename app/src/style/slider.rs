//! Slider and toggler styles.

use iced::widget::slider;
use iced::{Color, Theme, border};

/// Flat slider with rounded track and thin cursor handle.
pub fn flat(theme: &Theme, status: slider::Status) -> slider::Style {
    let palette = theme.extended_palette();

    slider::Style {
        rail: slider::Rail {
            backgrounds: (
                palette.background.strong.color.into(),
                Color::TRANSPARENT.into(),
            ),
            width: 24.0,
            border: border::rounded(2),
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Rectangle {
                width: 2,
                border_radius: 2.0.into(),
            },
            background: match status {
                slider::Status::Active => {
                    palette.background.strong.color.into()
                }
                slider::Status::Hovered => palette.primary.base.color.into(),
                slider::Status::Dragged => palette.primary.weak.color.into(),
            },
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        },
    }
}
