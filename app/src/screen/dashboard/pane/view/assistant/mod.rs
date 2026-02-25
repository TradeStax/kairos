//! AI Assistant Panel View
//!
//! Chat interface: scrollable message history with plain text rendering,
//! tool call/result cards, thinking blocks, hover effects, right-click
//! context menus, input row with send/stop, settings dropdown,
//! API key modal.

mod api_key_modal;
mod input;
mod messages;

use crate::screen::dashboard::pane::types::{AiAssistantEvent, AiAssistantState, Event, Message};
use crate::style::{self, tokens};
use iced::{
    Alignment, Element, Length,
    widget::{column, container, mouse_area, opaque, stack},
};
use iced::widget::pane_grid;

use messages::divider;

// ── Public entry point ────────────────────────────────────────────

/// Render the AI assistant panel body.
pub fn view_assistant<'a>(
    state: &'a AiAssistantState,
    id: pane_grid::Pane,
    is_linked: bool,
) -> Element<'a, Message> {
    let body: Element<'a, Message> = column![
        messages::view_messages(state, id, is_linked),
        divider(),
        input::view_input_row(state, id, is_linked),
    ]
    .spacing(0)
    .height(Length::Fill)
    .into();

    let close_settings_msg = Message::PaneEvent(
        id,
        Event::AiAssistant(AiAssistantEvent::ToggleSettings),
    );

    // Settings dropdown overlay
    let with_settings: Element<'a, Message> = if state.show_settings {
        let panel = container(input::view_settings_panel(state, id))
            .style(style::dropdown_container)
            .padding(tokens::spacing::MD)
            .width(Length::Fixed(tokens::layout::MODAL_WIDTH_SM));

        let overlay = container(opaque(panel))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::End)
            .align_y(Alignment::Start);

        stack![
            body,
            mouse_area(overlay).on_press(close_settings_msg),
        ]
        .into()
    } else {
        body
    };

    // API key modal overlay
    if state.show_api_key_modal {
        stack![with_settings, api_key_modal::view_api_key_modal(state, id)].into()
    } else {
        with_settings
    }
}

/// Shorten a model ID to a human-readable badge label.
pub(super) fn model_short_name(id: &str) -> &'static str {
    crate::screen::dashboard::pane::types::model_display_name(id)
}
