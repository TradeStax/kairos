//! Message list rendering for the AI assistant panel.

use crate::components::primitives::{
    Icon,
    badge::{BadgeKind, badge},
    body, small, tiny,
    icon_text,
    separator::flex_space,
};
pub use crate::components::primitives::separator::divider;
use crate::screen::dashboard::pane::types::{
    AiAssistantEvent, AiAssistantState, Event, Message,
};
use crate::style::{self, palette, tokens};
use data::domain::assistant::ChatMessageKind;
use iced::{
    Alignment, Element, Length, Padding, Theme,
    widget::{
        button, column, container, mouse_area,
        scrollable,
    },
};
use iced::widget::pane_grid;

// ── Message list ──────────────────────────────────────────────────

pub fn view_messages<'a>(
    state: &'a AiAssistantState,
    id: pane_grid::Pane,
    is_linked: bool,
) -> Element<'a, Message> {
    let mut items: Vec<Element<'a, Message>> = Vec::new();

    let msgs = &state.messages;
    let mut i = 0;
    while i < msgs.len() {
        let msg = &msgs[i];

        // Tool calls are merged into their ToolResult row
        if matches!(msg.kind, ChatMessageKind::ToolCall { .. }) {
            i += 1;
            continue;
        }

        // For User messages, check if the next msg is a
        // ContextAttachment — if so, render the attachment as a
        // compact label *above* the user bubble.
        if matches!(msg.kind, ChatMessageKind::User { .. }) {
            if let Some(next) = msgs.get(i + 1) {
                if let ChatMessageKind::ContextAttachment {
                    ticker,
                    timeframe,
                    candle_count,
                    is_live,
                    ..
                } = &next.kind
                {
                    items.push(view_context_label(
                        ticker,
                        timeframe,
                        *candle_count,
                        *is_live,
                    ));
                    items.push(view_message_item(
                        msg, i, state, id,
                    ));
                    i += 2; // skip the ContextAttachment
                    continue;
                }
            }
        }

        // Skip ContextAttachment rendered via look-ahead above
        if matches!(
            msg.kind,
            ChatMessageKind::ContextAttachment { .. }
        ) {
            i += 1;
            continue;
        }

        items.push(view_message_item(msg, i, state, id));
        i += 1;
    }

    if state.is_streaming {
        // Show in-progress thinking indicator
        if state.in_think_block && !state.thinking_buffer.is_empty() {
            items.push(view_streaming_thinking());
        }

        for tc in &state.streaming_tool_calls {
            if !tc.is_complete {
                items.push(
                    view_tool_pending(&tc.name, &tc.display_summary),
                );
            }
        }
        if !state.streaming_buffer.is_empty() {
            items.push(view_text_bubble(&state.streaming_buffer));
        } else if state.streaming_tool_calls.is_empty()
            && !state.in_think_block
        {
            items.push(view_thinking_indicator());
        }
    }

    if items.is_empty() {
        return view_empty_state(is_linked);
    }

    let content = scrollable(
        column(items)
            .spacing(tokens::spacing::XS)
            .padding(Padding {
                top:    tokens::spacing::MD,
                right:  tokens::spacing::MD,
                bottom: tokens::spacing::SM,
                left:   tokens::spacing::MD,
            })
            .width(Length::Fill),
    )
    .id(state.scroll_id.clone())
    .height(Length::Fill)
    .width(Length::Fill);

    // Wrap in mouse_area to track cursor position for context menus
    mouse_area(content)
        .on_move(move |p| {
            Message::PaneEvent(
                id,
                Event::AiAssistant(AiAssistantEvent::CursorMoved(p)),
            )
        })
        .into()
}

fn view_empty_state<'a>(is_linked: bool) -> Element<'a, Message> {
    let hint = if is_linked {
        "Ask about chart patterns, order flow, volume profile,\n\
         price levels, or market structure."
    } else {
        "Set a link group to connect this pane to a chart,\n\
         then ask about patterns, order flow, and price levels."
    };

    container(
        column![
            icon_text(Icon::MessageSquare, 28)
                .style(palette::neutral_text),
            small(hint)
                .style(palette::neutral_text)
                .align_x(iced::alignment::Horizontal::Center),
        ]
        .spacing(tokens::spacing::SM)
        .align_x(Alignment::Center),
    )
    .center(Length::Fill)
    .into()
}

// ── Render DisplayMessage by kind ─────────────────────────────────

pub fn view_message_item<'a>(
    msg: &'a data::domain::assistant::DisplayMessage,
    index: usize,
    state: &'a AiAssistantState,
    id: pane_grid::Pane,
) -> Element<'a, Message> {
    match &msg.kind {
        ChatMessageKind::User { text } => {
            view_user_bubble(text, index, id)
        }
        ChatMessageKind::AssistantText { text } => {
            view_text_bubble_interactive(text, index, id)
        }
        ChatMessageKind::Thinking { text } => {
            let is_expanded =
                state.expanded_thinking.contains(&index);
            view_thinking_block(text, index, is_expanded, id)
        }
        ChatMessageKind::ToolCall { .. } => {
            iced::widget::Space::new().into()
        }
        ChatMessageKind::ToolResult {
            name,
            display_summary,
            is_error,
            ..
        } => view_tool_result(name, display_summary, *is_error),
        ChatMessageKind::ContextAttachment { .. } => {
            // Rendered via look-ahead above the user message
            iced::widget::Space::new().into()
        }
        ChatMessageKind::SystemNotice { text, is_error } => {
            view_system_notice(text, *is_error, id)
        }
    }
}

fn view_user_bubble<'a>(
    content: &'a str,
    index: usize,
    id: pane_grid::Pane,
) -> Element<'a, Message> {
    let inner = button(
        body(content)
            .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
    )
    .style(style::button::chat_bubble_user)
    .padding([tokens::spacing::SM, tokens::spacing::MD])
    .on_press(Message::PaneEvent(id, Event::DismissContextMenu))
    .clip(false);

    let with_ctx = mouse_area(inner).on_right_press(
        Message::PaneEvent(
            id,
            Event::AiAssistant(
                AiAssistantEvent::MessageRightClicked(index),
            ),
        ),
    );

    iced::widget::row![flex_space(), container(with_ctx).max_width(500)].into()
}

/// Plain text assistant message bubble with hover + right-click.
fn view_text_bubble_interactive<'a>(
    content: &'a str,
    index: usize,
    id: pane_grid::Pane,
) -> Element<'a, Message> {
    let inner = button(
        body(content)
            .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
    )
    .style(style::button::chat_bubble)
    .padding([tokens::spacing::SM, tokens::spacing::MD])
    .on_press(Message::PaneEvent(id, Event::DismissContextMenu))
    .width(Length::Fill)
    .clip(false);

    mouse_area(inner)
        .on_right_press(Message::PaneEvent(
            id,
            Event::AiAssistant(
                AiAssistantEvent::MessageRightClicked(index),
            ),
        ))
        .into()
}

/// Plain text assistant message bubble (non-interactive, for streaming).
pub fn view_text_bubble<'a>(content: &'a str) -> Element<'a, Message> {
    container(
        body(content)
            .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
    )
    .padding([tokens::spacing::SM, tokens::spacing::MD])
    .style(|theme: &Theme| {
        let p = theme.extended_palette();
        container::Style {
            background: Some(p.background.weak.color.into()),
            text_color: Some(p.background.weak.text),
            border: iced::Border {
                radius: iced::border::radius(tokens::radius::MD),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .width(Length::Fill)
    .into()
}

// ── Thinking blocks ──────────────────────────────────────────────

/// Collapsed/expanded thinking block for a committed `<think>` message.
fn view_thinking_block<'a>(
    content: &'a str,
    index: usize,
    is_expanded: bool,
    id: pane_grid::Pane,
) -> Element<'a, Message> {
    let toggle_msg = Message::PaneEvent(
        id,
        Event::AiAssistant(AiAssistantEvent::ToggleThinking(index)),
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
                        .wrapping(
                            iced::widget::text::Wrapping::WordOrGlyph,
                        ),
                )
                .padding([0.0, tokens::spacing::SM]),
            ]
            .spacing(tokens::spacing::XXS),
        )
        .style(|theme: &Theme| {
            let p = theme.extended_palette();
            container::Style {
                background: Some(
                    p.background.weak.color
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
        container(header)
            .width(Length::Fill)
            .into()
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

/// Compact tool result row — merged view of a completed tool call + result.
pub fn view_tool_result<'a>(
    name: &'a str,
    summary: &'a str,
    is_error: bool,
) -> Element<'a, Message> {
    let (icon, icon_style): (
        Icon,
        fn(&Theme) -> iced::widget::text::Style,
    ) = if is_error {
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
pub fn view_tool_pending<'a>(
    name: &'a str,
    summary: &'a str,
) -> Element<'a, Message> {
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
                    Event::AiAssistant(
                        AiAssistantEvent::RetryLastMessage,
                    ),
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
