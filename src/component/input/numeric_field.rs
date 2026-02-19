//! Numeric text input with validation styling.

use iced::widget::{row, text, text_input};
use iced::{Element, Length};

use crate::style;
use crate::style::tokens;

pub struct NumericFieldBuilder<'a, Message> {
    label: &'a str,
    placeholder: &'a str,
    value: &'a str,
    is_valid: bool,
    on_input: Box<dyn Fn(String) -> Message + 'a>,
    on_submit: Option<Message>,
    width: Option<Length>,
}

impl<'a, Message: 'a> NumericFieldBuilder<'a, Message> {
    pub fn new(
        label: &'a str,
        placeholder: &'a str,
        value: &'a str,
        is_valid: bool,
        on_input: impl Fn(String) -> Message + 'a,
    ) -> Self {
        Self {
            label,
            placeholder,
            value,
            is_valid,
            on_input: Box::new(on_input),
            on_submit: None,
            width: None,
        }
    }

    pub fn on_submit(mut self, message: Message) -> Self {
        self.on_submit = Some(message);
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
        let is_valid = self.is_valid;

        let mut input = text_input(self.placeholder, self.value)
            .on_input(self.on_input)
            .align_x(iced::Alignment::Center)
            .style(move |theme, status| style::validated_text_input(theme, status, is_valid));

        if let Some(msg) = self.on_submit {
            input = input.on_submit(msg);
        }

        if let Some(w) = self.width {
            input = input.width(w);
        }

        row![text(self.label).size(tokens::text::LABEL), input]
            .spacing(tokens::spacing::XS)
            .align_y(iced::Alignment::Center)
            .into()
    }
}

impl<'a, Message: Clone + 'a> From<NumericFieldBuilder<'a, Message>> for Element<'a, Message> {
    fn from(builder: NumericFieldBuilder<'a, Message>) -> Self {
        builder.into_element()
    }
}
