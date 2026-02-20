//! Labeled text input with optional validation.

use iced::widget::{column, text, text_input};
use iced::{Element, Length};

use crate::style;
use crate::style::{palette, tokens};

pub struct TextFieldBuilder<'a, Message> {
    label: &'a str,
    placeholder: &'a str,
    value: &'a str,
    on_input: Box<dyn Fn(String) -> Message + 'a>,
    on_submit: Option<Message>,
    is_valid: bool,
    error_message: Option<&'a str>,
    width: Option<Length>,
    text_size: Option<f32>,
    id: Option<String>,
}

impl<'a, Message: 'a> TextFieldBuilder<'a, Message> {
    pub fn new(
        label: &'a str,
        placeholder: &'a str,
        value: &'a str,
        on_input: impl Fn(String) -> Message + 'a,
    ) -> Self {
        Self {
            label,
            placeholder,
            value,
            on_input: Box::new(on_input),
            on_submit: None,
            is_valid: true,
            error_message: None,
            width: None,
            text_size: None,
            id: None,
        }
    }

    pub fn validate(mut self, is_valid: bool) -> Self {
        self.is_valid = is_valid;
        self
    }

    pub fn error_message(mut self, msg: &'a str) -> Self {
        self.error_message = Some(msg);
        self
    }

    pub fn on_submit(mut self, message: Message) -> Self {
        self.on_submit = Some(message);
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = Some(width.into());
        self
    }

    pub fn text_size(mut self, size: f32) -> Self {
        self.text_size = Some(size);
        self
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn into_element(self) -> Element<'a, Message>
    where
        Message: Clone,
    {
        let is_valid = self.is_valid;
        let mut input = text_input(self.placeholder, self.value)
            .on_input(self.on_input)
            .style(move |theme, status| style::validated_text_input(theme, status, is_valid));

        if let Some(size) = self.text_size {
            input = input.size(size);
        }

        if let Some(msg) = self.on_submit {
            input = input.on_submit(msg);
        }

        if let Some(id) = self.id {
            input = input.id(id);
        }

        if let Some(w) = self.width {
            input = input.width(w);
        }

        let label_widget = text(self.label).size(tokens::text::LABEL);

        let mut col = column![label_widget, input].spacing(tokens::spacing::XS);

        if !self.is_valid
            && let Some(err) = self.error_message
        {
            col = col.push(
                text(err)
                    .size(tokens::text::TINY)
                    .color(palette::error_color()),
            );
        }

        col.into()
    }
}

impl<'a, Message: Clone + 'a> From<TextFieldBuilder<'a, Message>> for Element<'a, Message> {
    fn from(builder: TextFieldBuilder<'a, Message>) -> Self {
        builder.into_element()
    }
}
