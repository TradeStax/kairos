//! Reusable modal header bar matching the pane title bar style.
//!
//! Provides a consistent header for all modals: subtle background,
//! title text, optional inline controls, and a close button.

use iced::widget::{container, row, space, text};
use iced::{Alignment, Element, Length, Padding};

use crate::components::primitives::icon_button::icon_button;
use crate::components::primitives::icons::Icon;
use crate::style::{self, tokens};

/// Builds a styled modal header bar.
///
/// # Example
/// ```ignore
/// ModalHeaderBuilder::new("Indicators")
///     .on_close(Message::Close)
///     .into_element()
/// ```
pub struct ModalHeaderBuilder<'a, Message> {
    title: String,
    on_close: Option<Message>,
    close_icon: Icon,
    controls: Vec<Element<'a, Message>>,
}

impl<'a, Message: Clone + 'a> ModalHeaderBuilder<'a, Message> {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            on_close: None,
            close_icon: Icon::Close,
            controls: Vec::new(),
        }
    }

    /// Set the message emitted when the close button is pressed.
    pub fn on_close(mut self, message: Message) -> Self {
        self.on_close = Some(message);
        self
    }

    /// Override the close button icon (default: `Icon::Close`).
    pub fn close_icon(mut self, icon: Icon) -> Self {
        self.close_icon = icon;
        self
    }

    /// Add an inline control between the title and the close button.
    pub fn push_control(
        mut self,
        control: impl Into<Element<'a, Message>>,
    ) -> Self {
        self.controls.push(control.into());
        self
    }

    pub fn into_element(self) -> Element<'a, Message> {
        let title = text(self.title).size(tokens::text::HEADING);

        let mut header_row = row![title, space::horizontal()]
            .spacing(tokens::spacing::XS)
            .align_y(Alignment::Center)
            .width(Length::Fill);

        for control in self.controls {
            header_row = header_row.push(control);
        }

        if let Some(on_close) = self.on_close {
            header_row = header_row.push(
                icon_button(self.close_icon)
                    .size(12)
                    .padding(tokens::spacing::XS)
                    .on_press(on_close),
            );
        }

        container(header_row)
            .padding(Padding {
                top: 0.0,
                right: tokens::spacing::SM,
                bottom: 0.0,
                left: tokens::spacing::XL,
            })
            .height(tokens::layout::TITLE_BAR_HEIGHT)
            .width(Length::Fill)
            .align_y(Alignment::Center)
            .style(style::floating_panel_header)
            .into()
    }
}

impl<'a, Message: Clone + 'a> From<ModalHeaderBuilder<'a, Message>>
    for Element<'a, Message>
{
    fn from(builder: ModalHeaderBuilder<'a, Message>) -> Self {
        builder.into_element()
    }
}
