//! Thinking block expand/collapse rendering.

use crate::components::primitives::{Icon, icon_text, small, tiny};
use crate::screen::dashboard::pane::types::{AiAssistantEvent, Event, Message};
use crate::style::{self, palette, tokens};
use iced::widget::pane_grid;
use iced::{
    Alignment, Element, Length, Theme,
    widget::{button, column, container},
};

/// Collapsed/expanded thinking block for a committed `<think>` message.
pub fn view_thinking_block<'a>(
    content: &'a str,
    index: usize,
    is_expanded: bool,
    id: pane_grid::Pane,
) -> Element<'a, Message> {
    let toggle_msg = Message::PaneEvent(
        id,
        Box::new(Event::AiAssistant(AiAssistantEvent::ToggleThinking(index))),
    );

    let icon = if is_expanded {
        Icon::ChevronDown
    } else {
        Icon::ExpandRight
    };

    let header = button(
        iced::widget::row![
            icon_text(icon, 10).style(palette::neutral_text),
            tiny("Reasoning").style(palette::neutral_text),
        ]
        .spacing(tokens::spacing::XS)
        .align_y(Alignment::Center),
    )
    .style(style::button::list_item)
    .padding([tokens::spacing::XXS, tokens::spacing::SM])
    .on_press(toggle_msg);

    if is_expanded {
        container(
            column![
                header,
                container(
                    small(content)
                        .style(palette::neutral_text)
                        .wrapping(iced::widget::text::Wrapping::WordOrGlyph,),
                )
                .padding([0.0, tokens::spacing::SM]),
            ]
            .spacing(tokens::spacing::XXS),
        )
        .style(|theme: &Theme| {
            let p = theme.extended_palette();
            container::Style {
                background: Some(
                    p.background
                        .weak
                        .color
                        .scale_alpha(tokens::alpha::FAINT)
                        .into(),
                ),
                border: iced::Border {
                    radius: iced::border::radius(tokens::radius::SM),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .padding([tokens::spacing::XXS, 0.0])
        .width(Length::Fill)
        .into()
    } else {
        container(header).width(Length::Fill).into()
    }
}

/// Streaming indicator while inside a `<think>` block.
pub fn view_streaming_thinking<'a>() -> Element<'a, Message> {
    container(
        iced::widget::row![
            icon_text(Icon::RefreshCw, 11).style(palette::info_text),
            tiny("Reasoning...").style(palette::neutral_text),
        ]
        .spacing(tokens::spacing::XS)
        .align_y(Alignment::Center),
    )
    .padding([tokens::spacing::XS, tokens::spacing::SM])
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

/// Simple "Thinking..." indicator shown when streaming starts.
pub fn view_thinking_indicator<'a>() -> Element<'a, Message> {
    container(
        iced::widget::row![
            icon_text(Icon::RefreshCw, 11).style(palette::info_text),
            tiny("Thinking...").style(palette::neutral_text),
        ]
        .spacing(tokens::spacing::XS)
        .align_y(Alignment::Center),
    )
    .padding([tokens::spacing::XS, tokens::spacing::SM])
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
