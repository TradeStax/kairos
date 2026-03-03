//! Message list rendering for the AI assistant panel.

mod assistant;
mod system;
mod thinking;
mod tool;
mod user;

pub use crate::components::primitives::separator::divider;

use crate::components::primitives::{Icon, icon_text, small};
use crate::screen::dashboard::pane::types::{AiAssistantEvent, AiAssistantState, Event, Message};
use crate::style::{palette, tokens};
use ai::ChatMessageKind;
use iced::widget::pane_grid;
use iced::{
    Alignment, Element, Length, Padding,
    widget::{column, container, mouse_area, scrollable},
};

use assistant::{view_text_bubble, view_text_bubble_interactive};
use system::view_system_notice;
use thinking::{view_streaming_thinking, view_thinking_block, view_thinking_indicator};
use tool::{view_context_label, view_tool_pending, view_tool_result};
use user::view_user_bubble;

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
        if matches!(msg.kind, ChatMessageKind::User { .. })
            && let Some(next) = msgs.get(i + 1)
            && let ChatMessageKind::ContextAttachment {
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
            items.push(view_message_item(msg, i, state, id));
            i += 2; // skip the ContextAttachment
            continue;
        }

        // Skip ContextAttachment rendered via look-ahead above
        if matches!(msg.kind, ChatMessageKind::ContextAttachment { .. }) {
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
                items.push(view_tool_pending(&tc.name, &tc.display_summary));
            }
        }
        if !state.streaming_buffer.is_empty() {
            items.push(view_text_bubble(&state.streaming_buffer));
        } else if state.streaming_tool_calls.is_empty() && !state.in_think_block {
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
                top: tokens::spacing::MD,
                right: tokens::spacing::MD,
                bottom: tokens::spacing::SM,
                left: tokens::spacing::MD,
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
                Box::new(Event::AiAssistant(AiAssistantEvent::CursorMoved(p))),
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
            icon_text(Icon::MessageSquare, 28).style(palette::neutral_text),
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
    msg: &'a ai::DisplayMessage,
    index: usize,
    state: &'a AiAssistantState,
    id: pane_grid::Pane,
) -> Element<'a, Message> {
    match &msg.kind {
        ChatMessageKind::User { text } => view_user_bubble(text, index, id),
        ChatMessageKind::AssistantText { text } => view_text_bubble_interactive(text, index, id),
        ChatMessageKind::Thinking { text } => {
            let is_expanded = state.expanded_thinking.contains(&index);
            view_thinking_block(text, index, is_expanded, id)
        }
        ChatMessageKind::ToolCall { .. } => iced::widget::Space::new().into(),
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
        ChatMessageKind::SystemNotice { text, is_error } => view_system_notice(text, *is_error, id),
    }
}
