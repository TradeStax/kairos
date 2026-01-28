//! API Key Configuration Modal
//!
//! Provides UI for configuring Databento and Massive API keys.
//! Keys are stored securely in the OS keyring.

use data::{ApiKeyStatus, ApiProvider, SecretsManager};
use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, row, text, text_input, Space},
};

use crate::style::{self, Icon, icon_text};

/// What action triggered showing the API key config modal
#[derive(Debug, Clone)]
pub enum TriggeredBy {
    /// User manually opened settings
    Settings,
    /// User tried to download data
    DataDownload,
    /// User tried to load options data
    OptionsData,
}

/// Messages for the API key config modal
#[derive(Debug, Clone)]
pub enum Message {
    /// Switch to a different provider tab
    SelectProvider(ApiProvider),
    /// API key input changed
    KeyInput(String),
    /// Toggle password visibility
    ToggleVisibility,
    /// Save the current key
    Save,
    /// Close modal
    Close,
}

/// Actions that the modal can trigger
#[derive(Debug, Clone)]
pub enum Action {
    /// Close the modal (no changes pending)
    Close,
    /// Reinitialize service (key was just saved)
    ReinitializeService(ApiProvider),
    /// Show error toast
    ShowError(String),
}

/// State for the API key config modal
pub struct ApiKeyConfigModal {
    /// Currently selected provider tab
    selected_provider: ApiProvider,
    /// Current input value
    key_input: String,
    /// Original key value when modal opened (to detect changes)
    original_key: String,
    /// Whether to show the password
    show_password: bool,
    /// What triggered showing this modal (for retry behavior)
    triggered_by: Option<TriggeredBy>,
    /// Secrets manager
    secrets: SecretsManager,
}

impl ApiKeyConfigModal {
    /// Create a new modal for a specific provider
    pub fn new(provider: ApiProvider, triggered_by: Option<TriggeredBy>) -> Self {
        let secrets = SecretsManager::new();

        // Pre-fill with existing key if from keyring
        let key_input = match secrets.get_api_key(provider) {
            ApiKeyStatus::FromKeyring(key) => key,
            _ => String::new(),
        };

        Self {
            selected_provider: provider,
            original_key: key_input.clone(),
            key_input,
            show_password: false,
            triggered_by,
            secrets,
        }
    }

    /// Get the provider that triggered this modal (if any specific one)
    pub fn triggered_by(&self) -> Option<&TriggeredBy> {
        self.triggered_by.as_ref()
    }

    /// Check if the current input differs from the original/saved key
    fn has_changes(&self) -> bool {
        self.key_input != self.original_key
    }

    /// Handle a message
    pub fn update(&mut self, message: Message) -> Option<Action> {
        match message {
            Message::SelectProvider(provider) => {
                self.selected_provider = provider;
                // Load existing key for this provider
                let key = match self.secrets.get_api_key(provider) {
                    ApiKeyStatus::FromKeyring(key) => key,
                    _ => String::new(),
                };
                self.key_input = key.clone();
                self.original_key = key;
                self.show_password = false;
                None
            }
            Message::KeyInput(input) => {
                self.key_input = input;
                None
            }
            Message::ToggleVisibility => {
                self.show_password = !self.show_password;
                None
            }
            Message::Save => {
                if self.key_input.is_empty() {
                    return Some(Action::ShowError("API key cannot be empty".to_string()));
                }

                match self.secrets.set_api_key(self.selected_provider, &self.key_input) {
                    Ok(()) => {
                        log::info!(
                            "Saved {} API key",
                            self.selected_provider.display_name()
                        );

                        // Refresh secrets manager to pick up the new key
                        self.secrets = SecretsManager::new();

                        // Verify the key was actually saved by reading it back
                        let verify_status = self.secrets.get_api_key(self.selected_provider);
                        match &verify_status {
                            ApiKeyStatus::FromKeyring(_) | ApiKeyStatus::FromEnv(_) => {
                                // Update original_key so Save button becomes disabled
                                self.original_key = self.key_input.clone();
                                // Immediately reinitialize the service
                                Some(Action::ReinitializeService(self.selected_provider))
                            }
                            ApiKeyStatus::NotConfigured => {
                                log::error!("Key save appeared to succeed but read back failed!");
                                Some(Action::ShowError("Failed to persist key".to_string()))
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to save API key: {}", e);
                        Some(Action::ShowError(e.to_string()))
                    }
                }
            }
            Message::Close => Some(Action::Close)
        }
    }

    /// Render the modal
    pub fn view(&self) -> Element<'_, Message> {
        let status = self.secrets.get_api_key(self.selected_provider);

        // Provider tabs - full width with centered text
        let tabs = row(
            ApiProvider::all()
                .iter()
                .map(|&provider| {
                    let is_selected = provider == self.selected_provider;
                    let has_key = self.secrets.get_api_key(provider).is_available();

                    let label = if has_key {
                        text(format!("{} *", provider.display_name()))
                            .size(12)
                            .align_x(Alignment::Center)
                    } else {
                        text(provider.display_name())
                            .size(12)
                            .align_x(Alignment::Center)
                    };

                    let btn = button(label)
                        .padding([10, 0])
                        .width(Length::FillPortion(1))
                        .style(if is_selected {
                            style::button::tab_active
                        } else {
                            style::button::tab_inactive
                        });

                    if is_selected {
                        btn.into()
                    } else {
                        btn.on_press(Message::SelectProvider(provider)).into()
                    }
                })
                .collect::<Vec<Element<'_, Message>>>(),
        )
        .spacing(6);

        // Status indicator
        let status_row = {
            let (status_text, status_style): (&str, fn(&iced::Theme) -> text::Style) = match &status
            {
                ApiKeyStatus::FromKeyring(_) => ("Configured", |theme| {
                    text::Style {
                        color: Some(theme.extended_palette().success.base.color),
                    }
                }),
                ApiKeyStatus::FromEnv(_) => ("From environment", |theme| {
                    text::Style {
                        color: Some(theme.extended_palette().primary.base.color),
                    }
                }),
                ApiKeyStatus::NotConfigured => ("Not configured", |theme| {
                    text::Style {
                        color: Some(theme.extended_palette().danger.base.color),
                    }
                }),
            };

            row![
                text("Status:").size(12),
                text(status_text).size(12).style(status_style),
            ]
            .spacing(6)
            .align_y(Alignment::Center)
        };

        // Key input with visibility toggle - fixed size button matching input height
        let key_input_row = {
            let input = if self.show_password {
                text_input("Enter API key...", &self.key_input)
            } else {
                text_input("Enter API key...", &self.key_input).secure(true)
            };

            // Fixed size button to match text input height (32px)
            let visibility_btn = button(
                container(icon_text(
                    if self.show_password {
                        Icon::Unlocked
                    } else {
                        Icon::Locked
                    },
                    14,
                ))
                .center_x(Length::Fill)
                .center_y(Length::Fill),
            )
            .width(Length::Fixed(32.0))
            .height(Length::Fixed(32.0))
            .padding(0)
            .style(style::button::secondary)
            .on_press(Message::ToggleVisibility);

            row![
                input
                    .on_input(Message::KeyInput)
                    .width(Length::Fill)
                    .size(13),
                visibility_btn,
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        };

        // Description
        let description = text(self.selected_provider.description())
            .size(11)
            .style(|theme: &iced::Theme| text::Style {
                color: Some(theme.extended_palette().background.weak.text),
            });

        // Action buttons - Close and Save only
        let close_btn = button(text("Close").size(12))
            .padding([8, 12])
            .style(style::button::secondary)
            .on_press(Message::Close);

        // Save enabled only when input has changed and is not empty
        let can_save = self.has_changes() && !self.key_input.is_empty();
        let save_btn = {
            let btn = button(text("Save").size(12))
                .padding([8, 16])
                .style(style::button::primary);

            if can_save {
                btn.on_press(Message::Save)
            } else {
                btn
            }
        };

        let action_row = row![
            Space::new().width(Length::Fill),
            close_btn,
            save_btn,
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        // Tab content area
        let tab_content = column![
            text(format!("{} API Key", self.selected_provider.display_name())).size(13),
            key_input_row,
            status_row,
            description,
        ]
        .spacing(10);

        // Main content
        let content = column![
            text("API Configuration").size(16),
            tabs,
            tab_content,
            action_row,
        ]
        .spacing(16);

        container(content)
            .width(380)
            .padding(20)
            .style(style::dashboard_modal)
            .into()
    }
}
