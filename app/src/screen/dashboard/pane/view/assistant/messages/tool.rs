//! Tool call/result card rendering.

use crate::components::primitives::{
    Icon,
    badge::{BadgeKind, badge},
    icon_text, tiny,
};
use crate::screen::dashboard::pane::types::Message;
use crate::style::{palette, tokens};
use iced::{
    Alignment, Element, Theme,
    widget::container,
};

/// Compact tool result row — merged view of a completed tool call + result.
pub fn view_tool_result<'a>(
    name: &'a str,
    summary: &'a str,
    is_error: bool,
) -> Element<'a, Message> {
    let (icon, icon_style): (Icon, fn(&Theme) -> iced::widget::text::Style) = if is_error {
        (Icon::Close, palette::error_text)
    } else {
        (Icon::Checkmark, palette::success_text)
    };

    container(
        iced::widget::row![
            icon_text(icon, 11).style(icon_style),
            badge(name, BadgeKind::Default),
            tiny(summary)
                .style(palette::neutral_text)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
        ]
        .spacing(tokens::spacing::XS)
        .align_y(Alignment::Center),
    )
    .padding([tokens::spacing::XXS, tokens::spacing::XS])
    .into()
}

/// In-progress tool call indicator (shown during streaming).
pub fn view_tool_pending<'a>(name: &'a str, summary: &'a str) -> Element<'a, Message> {
    container(
        iced::widget::row![
            icon_text(Icon::RefreshCw, 11).style(palette::info_text),
            badge(name, BadgeKind::Info),
            tiny(summary).style(palette::neutral_text),
        ]
        .spacing(tokens::spacing::XS)
        .align_y(Alignment::Center),
    )
    .padding([tokens::spacing::XXS, tokens::spacing::XS])
    .style(|theme: &Theme| container::Style {
        background: Some(
            theme
                .extended_palette()
                .background
                .weak
                .color
                .scale_alpha(tokens::alpha::SUBTLE)
                .into(),
        ),
        border: iced::Border {
            radius: iced::border::radius(tokens::radius::SM),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

/// Compact right-aligned context label shown above a user message.
pub fn view_context_label<'a>(
    ticker: &'a str,
    timeframe: &'a str,
    candle_count: usize,
    is_live: bool,
) -> Element<'a, Message> {
    use crate::components::primitives::separator::flex_space;

    let live_str = if is_live { "LIVE" } else { "Historical" };
    let summary = format!(
        "{} {} | {} candles | {}",
        ticker, timeframe, candle_count, live_str
    );

    iced::widget::row![
        flex_space(),
        container(
            iced::widget::row![
                icon_text(Icon::Cpu, 9).style(palette::info_text),
                tiny(summary).style(palette::neutral_text),
            ]
            .spacing(tokens::spacing::XS)
            .align_y(Alignment::Center),
        )
        .padding([tokens::spacing::XXS, tokens::spacing::SM]),
    ]
    .into()
}
