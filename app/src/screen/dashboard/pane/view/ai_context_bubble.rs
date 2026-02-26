//! Floating AI Context Bubble
//!
//! Appears at the bottom-right of a pane after the user completes an
//! AiContext drawing. Shows a summary of the selected chart range and
//! a text input for the user's question.

use crate::components::primitives::{Icon, icon_text, separator::divider, small, tiny};
use crate::screen::dashboard::pane::types::{
    AiContextBubble, AiContextBubbleEvent, Event, Message,
};
use crate::style::{self, tokens};
use iced::widget::pane_grid;
use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, row, text_input},
};

/// Render the floating AI context bubble.
pub(crate) fn view<'a>(bubble: &'a AiContextBubble, pane: pane_grid::Pane) -> Element<'a, Message> {
    let summary = &bubble.range_summary;

    // Header row: icon + title + close button
    let close_btn = button(icon_text(Icon::Close, 12))
        .on_press(Message::PaneEvent(
            pane,
            Box::new(Event::AiContextBubble(AiContextBubbleEvent::Dismiss)),
        ))
        .style(|theme, status| style::button::transparent(theme, status, false))
        .padding(2);

    let header = row![
        icon_text(Icon::Cpu, 13),
        small("Analyze Range"),
        iced::widget::Space::new().width(Length::Fill),
        close_btn,
    ]
    .spacing(tokens::spacing::XS)
    .align_y(Alignment::Center);

    // Range info
    let info_line1 = row![
        tiny(format!("{} | {}", summary.ticker, summary.timeframe)),
        tiny(format!("{} candles", summary.candle_count)),
    ]
    .spacing(tokens::spacing::MD);

    let info_line2 = tiny(format!(
        "{} \u{2192} {}",
        summary.time_start_fmt, summary.time_end_fmt
    ));

    let info_line3 = row![
        tiny(format!(
            "{}\u{2013}{}",
            summary.price_low, summary.price_high
        )),
        tiny(format!("V: {}", summary.total_volume)),
        tiny(format!("\u{0394}: {}", summary.net_delta)),
    ]
    .spacing(tokens::spacing::SM);

    // Input + send
    let can_send = !bubble.input_text.trim().is_empty();

    let input = text_input("What do you see here?", &bubble.input_text)
        .on_input(move |s| {
            Message::PaneEvent(
                pane,
                Box::new(Event::AiContextBubble(AiContextBubbleEvent::InputChanged(
                    s,
                ))),
            )
        })
        .on_submit(Message::PaneEvent(
            pane,
            Box::new(Event::AiContextBubble(AiContextBubbleEvent::Submit)),
        ))
        .size(tokens::text::SMALL)
        .width(Length::Fill);

    let send_btn = button(icon_text(Icon::Send, 14))
        .on_press_maybe(can_send.then(|| {
            Message::PaneEvent(
                pane,
                Box::new(Event::AiContextBubble(AiContextBubbleEvent::Submit)),
            )
        }))
        .style(move |theme, status| {
            if can_send {
                style::button::primary(theme, status)
            } else {
                style::button::secondary(theme, status)
            }
        })
        .padding([4, 8]);

    let input_row = row![input, send_btn]
        .spacing(tokens::spacing::XS)
        .align_y(Alignment::Center);

    let content = column![
        header,
        divider(),
        info_line1,
        info_line2,
        info_line3,
        divider(),
        input_row,
    ]
    .spacing(tokens::spacing::XS);

    container(content)
        .style(style::floating_panel)
        .padding(tokens::spacing::MD)
        .width(Length::Fixed(340.0))
        .into()
}
