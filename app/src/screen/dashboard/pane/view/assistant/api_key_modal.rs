//! API key modal overlay for the AI assistant panel.

use crate::components::input::secure_field::SecureFieldBuilder;
use crate::components::primitives::{
    Icon, icon_button::icon_button, separator::flex_space, small, title,
};
use crate::screen::dashboard::pane::types::{AiAssistantEvent, AiAssistantState, Event, Message};
use crate::style::{self, palette, tokens};
use iced::widget::pane_grid;
use iced::{
    Alignment, Element, Length, Padding, Theme,
    widget::{button, column, container, mouse_area, opaque, row},
};

pub fn view_api_key_modal<'a>(
    state: &'a AiAssistantState,
    id: pane_grid::Pane,
) -> Element<'a, Message> {
    let dismiss_msg = Message::PaneEvent(
        id,
        Box::new(Event::AiAssistant(AiAssistantEvent::DismissApiKeyModal)),
    );
    let save_msg = Message::PaneEvent(
        id,
        Box::new(Event::AiAssistant(AiAssistantEvent::SaveApiKey)),
    );
    let dismiss_msg2 = dismiss_msg.clone();
    let dismiss_msg3 = dismiss_msg.clone();

    let modal_content: Element<'a, Message> = container(column![
        row![
            title("OpenRouter API Key"),
            flex_space(),
            icon_button(Icon::Close)
                .size(12.0)
                .padding(tokens::spacing::SM)
                .on_press(dismiss_msg2)
                .into_element(),
        ]
        .align_y(Alignment::Center)
        .padding(Padding {
            top: tokens::spacing::LG,
            bottom: tokens::spacing::MD,
            left: tokens::spacing::XL,
            right: tokens::spacing::MD,
        }),
        column![
            small("Kairos uses OpenRouter to power AI analysis."),
            small("Get a free API key at openrouter.ai/keys").style(palette::info_text),
            SecureFieldBuilder::new("API Key", "sk-or-...", &state.api_key_input, move |s| {
                Message::PaneEvent(
                    id,
                    Box::new(Event::AiAssistant(AiAssistantEvent::ApiKeyInputChanged(s))),
                )
            },)
            .into_element(),
        ]
        .spacing(tokens::spacing::MD)
        .padding(Padding {
            top: 0.0,
            bottom: tokens::spacing::LG,
            left: tokens::spacing::XL,
            right: tokens::spacing::XL,
        }),
        row![
            flex_space(),
            button(small("Cancel"))
                .style(style::button::secondary)
                .on_press(dismiss_msg3)
                .padding([tokens::spacing::SM, tokens::spacing::MD]),
            button(small("Save Key"))
                .style(style::button::primary)
                .on_press(save_msg)
                .padding([tokens::spacing::SM, tokens::spacing::MD]),
        ]
        .spacing(tokens::spacing::SM)
        .padding(Padding {
            top: 0.0,
            bottom: tokens::spacing::LG,
            left: tokens::spacing::XL,
            right: tokens::spacing::XL,
        }),
    ])
    .style(style::chart_modal)
    .width(Length::Fixed(tokens::layout::MODAL_WIDTH_LG))
    .into();

    let centered_modal = container(opaque(modal_content))
        .center(Length::Fill)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme: &Theme| container::Style {
            background: Some(iced::Color::BLACK.scale_alpha(tokens::alpha::MEDIUM).into()),
            ..Default::default()
        });

    mouse_area(centered_modal).on_press(dismiss_msg).into()
}
