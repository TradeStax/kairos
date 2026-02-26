//! Canvas stroke styles for chart overlays (crosshair, dashed lines).

use iced::Theme;
use iced::theme::palette::Extended;
use iced::widget::canvas::{LineDash, Stroke};

use crate::style::tokens;

pub fn dashed_line(theme: &'_ Theme) -> Stroke<'_> {
    let palette = theme.extended_palette();

    Stroke::with_color(
        Stroke {
            width: tokens::border::THIN,
            line_dash: LineDash {
                segments: &[4.0, 4.0],
                offset: 8,
            },
            ..Default::default()
        },
        palette
            .secondary
            .strong
            .color
            .scale_alpha(if palette.is_dark {
                tokens::alpha::HEAVY
            } else {
                1.0
            }),
    )
}

pub fn dashed_line_from_palette(palette: &'_ Extended) -> Stroke<'_> {
    Stroke::with_color(
        Stroke {
            width: tokens::border::THIN,
            line_dash: LineDash {
                segments: &[4.0, 4.0],
                offset: 8,
            },
            ..Default::default()
        },
        palette
            .secondary
            .strong
            .color
            .scale_alpha(if palette.is_dark {
                tokens::alpha::HEAVY
            } else {
                1.0
            }),
    )
}
