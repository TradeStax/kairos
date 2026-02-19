//! Empty-state placeholder: icon + message + optional call-to-action.

use iced::widget::{button, column, text};
use iced::{Alignment, Element};

use crate::style;
use crate::component::primitives::{ICONS_FONT, Icon};
use crate::style::tokens;

pub struct EmptyStateBuilder<'a, Message> {
    message_text: &'a str,
    icon: Option<Icon>,
    action_label: Option<&'a str>,
    action_msg: Option<Message>,
}

impl<'a, Message: 'a> EmptyStateBuilder<'a, Message> {
    pub fn new(message_text: &'a str) -> Self {
        Self {
            message_text,
            icon: None,
            action_label: None,
            action_msg: None,
        }
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn action(mut self, label: &'a str, msg: Message) -> Self {
        self.action_label = Some(label);
        self.action_msg = Some(msg);
        self
    }

    pub fn into_element(self) -> Element<'a, Message>
    where
        Message: Clone,
    {
        let mut col = column![]
            .align_x(Alignment::Center)
            .spacing(tokens::spacing::LG);

        if let Some(icon) = self.icon {
            let icon_text = text(char::from(icon).to_string()).font(ICONS_FONT).size(32);
            col = col.push(icon_text);
        }

        col = col.push(
            text(self.message_text)
                .size(tokens::text::BODY)
                .align_x(Alignment::Center),
        );

        if let (Some(label), Some(msg)) = (self.action_label, self.action_msg) {
            let btn = button(text(label).size(tokens::text::BODY))
                .on_press(msg)
                .style(style::button::primary)
                .padding([tokens::spacing::SM, tokens::spacing::LG]);

            col = col.push(btn);
        }

        col.into()
    }
}

impl<'a, Message: Clone + 'a> From<EmptyStateBuilder<'a, Message>> for Element<'a, Message> {
    fn from(builder: EmptyStateBuilder<'a, Message>) -> Self {
        builder.into_element()
    }
}
