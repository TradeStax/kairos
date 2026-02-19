//! Button that toggles between on/off visual states.

use iced::widget::{button, text};
use iced::{Element, Length, Padding, Theme};

use crate::style;
use crate::style::tokens;

type StyleFn = Box<dyn Fn(&Theme, button::Status) -> button::Style>;

pub struct ToggleButtonBuilder<'a, Message> {
    label: &'a str,
    is_on: bool,
    on_press: Message,
    style_fn: Option<StyleFn>,
    padding: Padding,
    width: Option<Length>,
}

impl<'a, Message: 'a> ToggleButtonBuilder<'a, Message> {
    pub fn new(label: &'a str, is_on: bool, on_press: Message) -> Self {
        Self {
            label,
            is_on,
            on_press,
            style_fn: None,
            padding: Padding::from(tokens::spacing::SM),
            width: None,
        }
    }

    pub fn style(mut self, f: impl Fn(&Theme, button::Status) -> button::Style + 'static) -> Self {
        self.style_fn = Some(Box::new(f));
        self
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
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
        let is_on = self.is_on;

        let mut btn = button(text(self.label).size(tokens::text::BODY))
            .padding(self.padding)
            .on_press(self.on_press);

        if let Some(style_fn) = self.style_fn {
            btn = btn.style(move |theme, status| style_fn(theme, status));
        } else {
            btn = btn
                .style(move |theme, status| style::button::bordered_toggle(theme, status, is_on));
        }

        if let Some(w) = self.width {
            btn = btn.width(w);
        }

        btn.into()
    }
}

impl<'a, Message: Clone + 'a> From<ToggleButtonBuilder<'a, Message>> for Element<'a, Message> {
    fn from(builder: ToggleButtonBuilder<'a, Message>) -> Self {
        builder.into_element()
    }
}
