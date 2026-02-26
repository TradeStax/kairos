//! System notices rendering.

use crate::components::primitives::{separator::flex_space, tiny};
use crate::screen::dashboard::pane::types::{AiAssistantEvent, Event, Message};
use crate::style::{self, palette, tokens};
use iced::widget::pane_grid;
use iced::{
    Alignment, Element, Length, Theme,
    widget::{button, container},
};

pub fn view_system_notice<'a>(
    content: &'a str,
    is_error: bool,
    id: pane_grid::Pane,
) -> Element<'a, Message> {
    let mut notice_row: Vec<Element<'a, Message>> = vec![
        tiny(content)
            .style(palette::neutral_text)
            .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
            .into(),
    ];

    if is_error {
        notice_row.push(flex_space());
        notice_row.push(
            button(tiny("Retry"))
                .style(style::button::secondary)
                .padding([tokens::spacing::XXS, tokens::spacing::SM])
                .on_press(Message::PaneEvent(
                    id,
                    Box::new(Event::AiAssistant(AiAssistantEvent::RetryLastMessage)),
                ))
                .into(),
        );
    }

    container(
        iced::widget::row(notice_row)
            .spacing(tokens::spacing::SM)
            .align_y(Alignment::Center),
    )
    .padding([tokens::spacing::XXS, tokens::spacing::SM])
    .width(Length::Fill)
    .style(move |theme: &Theme| {
        let p = theme.extended_palette();
        let bg = if is_error {
            p.danger.weak.color.scale_alpha(tokens::alpha::SUBTLE)
        } else {
            p.background.weak.color.scale_alpha(tokens::alpha::MEDIUM)
        };
        container::Style {
            background: Some(bg.into()),
            border: iced::Border {
                radius: iced::border::radius(tokens::radius::SM),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .into()
}
