//! Key-value pair display: "Label: Value".

use iced::widget::{row, text};
use iced::{Color, Element, Length};

use crate::components::primitives::AZERET_MONO;
use crate::style::tokens;

pub struct KeyValueBuilder<'a, Message> {
    key: &'a str,
    value: &'a str,
    mono_value: bool,
    value_color: Option<Color>,
    key_width: Option<Length>,
    _message: std::marker::PhantomData<Message>,
}

impl<'a, Message: 'a> KeyValueBuilder<'a, Message> {
    pub fn new(key: &'a str, value: &'a str) -> Self {
        Self {
            key,
            value,
            mono_value: false,
            value_color: None,
            key_width: None,
            _message: std::marker::PhantomData,
        }
    }

    pub fn mono_value(mut self, mono: bool) -> Self {
        self.mono_value = mono;
        self
    }

    pub fn value_color(mut self, color: Color) -> Self {
        self.value_color = Some(color);
        self
    }

    pub fn key_width(mut self, width: impl Into<Length>) -> Self {
        self.key_width = Some(width.into());
        self
    }

    pub fn into_element(self) -> Element<'a, Message> {
        let mut key_text = text(self.key).size(tokens::text::SMALL);

        if let Some(w) = self.key_width {
            key_text = key_text.width(w);
        }

        let mut val_text = text(self.value).size(tokens::text::SMALL);

        if self.mono_value {
            val_text = val_text.font(AZERET_MONO);
        }

        if let Some(c) = self.value_color {
            val_text = val_text.color(c);
        }

        row![key_text, val_text].spacing(tokens::spacing::XS).into()
    }
}

impl<'a, Message: 'a> From<KeyValueBuilder<'a, Message>> for Element<'a, Message> {
    fn from(builder: KeyValueBuilder<'a, Message>) -> Self {
        builder.into_element()
    }
}
