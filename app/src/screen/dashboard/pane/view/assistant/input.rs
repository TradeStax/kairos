//! Input row and settings panel for the AI assistant panel.

use crate::components::primitives::{Icon, icon_button::icon_button, tiny};
use crate::screen::dashboard::pane::types::{
    self, AiAssistantEvent, AiAssistantState, Event, Message,
};
use crate::style::{self, palette, tokens};
use iced::widget::pane_grid;
use iced::{
    Alignment, Element, Length, Padding, Theme,
    widget::{column, container, row, text_input},
};

// ── Settings panel ────────────────────────────────────────────────

pub fn view_settings_panel<'a>(
    state: &'a AiAssistantState,
    id: pane_grid::Pane,
) -> Element<'a, Message> {
    let model_names: Vec<&'static str> = types::AI_MODELS.iter().map(|m| m.display_name).collect();

    let selected_name = types::model_display_name(&state.model);
    let model_picker = iced::widget::pick_list(
        model_names,
        Some(selected_name),
        move |name: &'static str| {
            Message::PaneEvent(
                id,
                Box::new(Event::AiAssistant(AiAssistantEvent::ModelChanged(
                    types::model_id_from_name(name).to_string(),
                ))),
            )
        },
    );

    let temp_value = state.temperature;
    let temp_slider = iced::widget::slider(0.0..=1.0, temp_value, move |v: f32| {
        let rounded = (v * 10.0).round() / 10.0;
        Message::PaneEvent(
            id,
            Box::new(Event::AiAssistant(AiAssistantEvent::TemperatureChanged(
                rounded,
            ))),
        )
    })
    .step(0.1);

    const TOKEN_OPTIONS: &[u32] = &[1024, 2048, 4096, 8192];
    let selected_tokens = state.max_tokens;
    let token_picker =
        iced::widget::pick_list(TOKEN_OPTIONS, Some(selected_tokens), move |val: u32| {
            Message::PaneEvent(
                id,
                Box::new(Event::AiAssistant(AiAssistantEvent::MaxTokensChanged(val))),
            )
        });

    column![
        tiny("Model").style(palette::neutral_text),
        model_picker,
        tiny(format!("Temperature: {:.1}", state.temperature)).style(palette::neutral_text),
        temp_slider,
        tiny("Max Tokens").style(palette::neutral_text),
        token_picker,
    ]
    .spacing(tokens::spacing::SM)
    .into()
}

// ── Input row ─────────────────────────────────────────────────────

pub fn view_input_row<'a>(
    state: &'a AiAssistantState,
    id: pane_grid::Pane,
    is_linked: bool,
) -> Element<'a, Message> {
    let can_send = !state.input_text.trim().is_empty() && !state.is_streaming;
    let is_streaming = state.is_streaming;

    const BTN_PADDING: f32 = 10.0;

    let action_btn: Element<'a, Message> = if is_streaming {
        icon_button(Icon::Stop)
            .size(14.0)
            .tooltip("Stop")
            .style(style::button::danger)
            .padding(BTN_PADDING)
            .on_press(Message::PaneEvent(
                id,
                Box::new(Event::AiAssistant(AiAssistantEvent::StopStreaming)),
            ))
            .into_element()
    } else {
        let mut btn = icon_button(Icon::Send)
            .size(14.0)
            .tooltip("Send  Enter")
            .style(move |t: &Theme, s| {
                if can_send {
                    style::button::primary(t, s)
                } else {
                    style::button::secondary(t, s)
                }
            })
            .padding(BTN_PADDING);
        if can_send {
            btn = btn.on_press(Message::PaneEvent(
                id,
                Box::new(Event::AiAssistant(AiAssistantEvent::SendMessage)),
            ));
        }
        btn.into_element()
    };

    let placeholder = if is_streaming {
        "Responding..."
    } else if is_linked {
        "Ask about this chart..."
    } else {
        "Ask a question..."
    };

    let input: Element<'a, Message> = text_input(placeholder, &state.input_text)
        .on_input(move |s| {
            Message::PaneEvent(
                id,
                Box::new(Event::AiAssistant(AiAssistantEvent::InputChanged(s))),
            )
        })
        .on_submit(Message::PaneEvent(
            id,
            Box::new(Event::AiAssistant(AiAssistantEvent::SendMessage)),
        ))
        .size(tokens::text::BODY)
        .padding(Padding {
            top: BTN_PADDING,
            bottom: BTN_PADDING,
            left: tokens::spacing::SM,
            right: tokens::spacing::SM,
        })
        .into();

    container(
        row![input, action_btn]
            .spacing(tokens::spacing::XS)
            .align_y(Alignment::Center),
    )
    .height(Length::Fixed(46.0))
    .align_y(Alignment::Center)
    .padding([0.0_f32, tokens::spacing::SM])
    .width(Length::Fill)
    .into()
}
