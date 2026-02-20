//! API Key Setup Modal
//!
//! Shown when a user tries to open the historical download modal
//! but no Databento API key is configured. Gates the download flow
//! behind credential setup.

use crate::components::input::secure_field::SecureFieldBuilder;
use crate::style::{self, tokens};
use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, row, space, text},
};

/// API key setup modal state
#[derive(Debug, Clone, PartialEq)]
pub struct ApiKeySetupModal {
    api_key_input: String,
    error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ApiKeySetupMessage {
    SetApiKey(String),
    Save,
    Close,
}

pub enum Action {
    Saved {
        provider: data::config::secrets::ApiProvider,
        key: String,
    },
    Closed,
}

impl ApiKeySetupModal {
    pub fn new() -> Self {
        Self {
            api_key_input: String::new(),
            error: None,
        }
    }

    pub fn update(&mut self, message: ApiKeySetupMessage) -> Option<Action> {
        match message {
            ApiKeySetupMessage::SetApiKey(key) => {
                self.api_key_input = key;
                self.error = None;
            }
            ApiKeySetupMessage::Save => {
                let key = self.api_key_input.trim();
                if key.is_empty() {
                    self.error = Some("API key cannot be empty".to_string());
                    return None;
                }
                if key.len() < 10 {
                    self.error =
                        Some("API key appears too short".to_string());
                    return None;
                }
                return Some(Action::Saved {
                    provider: data::config::secrets::ApiProvider::Databento,
                    key: key.to_string(),
                });
            }
            ApiKeySetupMessage::Close => {
                return Some(Action::Closed);
            }
        }
        None
    }

    pub fn view(&self) -> Element<'_, ApiKeySetupMessage> {
        let title = row![
            text("API Key Required").size(tokens::text::HEADING),
            space::horizontal().width(Length::Fill),
            button(
                text("\u{00D7}")
                    .size(tokens::text::TITLE)
                    .align_x(Alignment::Center),
            )
            .width(28)
            .height(28)
            .on_press(ApiKeySetupMessage::Close),
        ]
        .align_y(Alignment::Center);

        let description = text(
            "A Databento API key is required to download \
             historical futures data.",
        )
        .size(tokens::text::BODY);

        let link = text("Get your API key at databento.com/portal/keys")
            .size(tokens::text::SMALL)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().primary.base.color),
            });

        let key_field = SecureFieldBuilder::new(
            "API Key",
            "Enter Databento API key",
            &self.api_key_input,
            ApiKeySetupMessage::SetApiKey,
        )
        .into_element();

        let can_save = self.api_key_input.trim().len() >= 10;

        let buttons = row![
            button(
                text("Cancel")
                    .size(tokens::text::LABEL)
                    .align_x(Alignment::Center)
            )
            .width(Length::Fill)
            .on_press(ApiKeySetupMessage::Close)
            .padding([tokens::spacing::MD, tokens::spacing::XL]),
            button(
                text("Continue")
                    .size(tokens::text::LABEL)
                    .align_x(Alignment::Center)
            )
            .width(Length::Fill)
            .on_press_maybe(if can_save {
                Some(ApiKeySetupMessage::Save)
            } else {
                None
            })
            .padding([tokens::spacing::MD, tokens::spacing::XL])
            .style(style::button::primary),
        ]
        .spacing(tokens::spacing::MD);

        let mut content = column![title, description, link, key_field,]
            .spacing(tokens::spacing::LG)
            .align_x(Alignment::Start);

        if let Some(err) = &self.error {
            content = content.push(
                text(err)
                    .size(tokens::text::SMALL)
                    .style(|theme: &iced::Theme| {
                        iced::widget::text::Style {
                            color: Some(
                                theme
                                    .extended_palette()
                                    .danger
                                    .base
                                    .color,
                            ),
                        }
                    }),
            );
        }

        content = content.push(buttons);

        container(content)
            .width(Length::Fixed(tokens::layout::MODAL_WIDTH_MD))
            .padding(tokens::spacing::XXL)
            .style(style::dashboard_modal)
            .into()
    }
}
