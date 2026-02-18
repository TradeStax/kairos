//! Search input with icon and clear button.

use iced::widget::{button, row, text, text_input};
use iced::{Element, Length};

use crate::style;
use crate::component::primitives::{ICONS_FONT, Icon};
use crate::style::tokens;

pub struct SearchFieldBuilder<'a, Message> {
    placeholder: &'a str,
    value: &'a str,
    on_input: Box<dyn Fn(String) -> Message + 'a>,
    on_clear: Option<Message>,
    id: Option<String>,
    width: Option<Length>,
}

impl<'a, Message: 'a> SearchFieldBuilder<'a, Message> {
    pub fn new(
        placeholder: &'a str,
        value: &'a str,
        on_input: impl Fn(String) -> Message + 'a,
    ) -> Self {
        Self {
            placeholder,
            value,
            on_input: Box::new(on_input),
            on_clear: None,
            id: None,
            width: None,
        }
    }

    pub fn on_clear(mut self, message: Message) -> Self {
        self.on_clear = Some(message);
        self
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = Some(width.into());
        self
    }

    pub fn into_element(self) -> Element<'a, Message>
    where
        Message: Clone,
    {
        let search_icon = text(char::from(Icon::Search).to_string())
            .font(ICONS_FONT)
            .size(12);

        let mut input = text_input(self.placeholder, self.value).on_input(self.on_input);

        if let Some(id) = self.id {
            input = input.id(id);
        }

        if let Some(w) = self.width {
            input = input.width(w);
        }

        let mut search_row = row![search_icon, input]
            .spacing(tokens::spacing::XS)
            .align_y(iced::Alignment::Center);

        if let Some(clear_msg) = self.on_clear
            && !self.value.is_empty() {
                let clear_icon = text(char::from(Icon::Close).to_string())
                    .font(ICONS_FONT)
                    .size(12);

                let clear_btn = button(clear_icon)
                    .on_press(clear_msg)
                    .padding(tokens::spacing::XXS)
                    .style(|theme, status| style::button::transparent(theme, status, false));

                search_row = search_row.push(clear_btn);
            }

        search_row.into()
    }
}

impl<'a, Message: Clone + 'a> From<SearchFieldBuilder<'a, Message>> for Element<'a, Message> {
    fn from(builder: SearchFieldBuilder<'a, Message>) -> Self {
        builder.into_element()
    }
}
