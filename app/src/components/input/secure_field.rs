//! Password / API-key field with masking and optional set-indicator.

use iced::widget::{column, row, text, text_input};
use iced::{Element, Length};

use crate::style::{self, palette, tokens};

pub struct SecureFieldBuilder<'a, Message> {
    label: &'a str,
    placeholder: &'a str,
    value: &'a str,
    on_input: Box<dyn Fn(String) -> Message + 'a>,
    secure: bool,
    show_set_indicator: bool,
    is_set: bool,
    width: Option<Length>,
    _message: std::marker::PhantomData<Message>,
}

impl<'a, Message: 'a> SecureFieldBuilder<'a, Message> {
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
            secure: true,
            show_set_indicator: false,
            is_set: false,
            width: None,
            _message: std::marker::PhantomData,
        }
    }

    pub fn secure(mut self, secure: bool) -> Self {
        self.secure = secure;
        self
    }

    pub fn show_set_indicator(mut self, is_set: bool) -> Self {
        self.show_set_indicator = true;
        self.is_set = is_set;
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
        let mut input = text_input(self.placeholder, self.value)
            .on_input(self.on_input)
            .secure(self.secure)
            .style(|theme, status| style::validated_text_input(theme, status, true));

        if let Some(w) = self.width {
            input = input.width(w);
        }

        let label_widget = text(self.label).size(tokens::text::LABEL);

        let label_row: Element<'a, Message> = if self.show_set_indicator {
            let indicator = if self.is_set {
                text("(set)")
                    .size(tokens::text::TINY)
                    .style(palette::success_text)
            } else {
                text("(not set)")
                    .size(tokens::text::TINY)
                    .style(palette::neutral_text)
            };
            row![label_widget, indicator]
                .spacing(tokens::spacing::XS)
                .into()
        } else {
            label_widget.into()
        };

        column![label_row, input]
            .spacing(tokens::spacing::XS)
            .into()
    }
}

impl<'a, Message: Clone + 'a> From<SecureFieldBuilder<'a, Message>> for Element<'a, Message> {
    fn from(builder: SecureFieldBuilder<'a, Message>) -> Self {
        builder.into_element()
    }
}
