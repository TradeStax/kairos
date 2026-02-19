//! Themed checkbox with optional tooltip.

use iced::Element;
use iced::widget::{checkbox, container, text, tooltip};

use crate::style;
use crate::style::tokens;

pub struct CheckboxFieldBuilder<'a, Message> {
    label: &'a str,
    is_checked: bool,
    on_toggle: Box<dyn Fn(bool) -> Message + 'a>,
    tooltip_text: Option<&'a str>,
    text_size: Option<f32>,
    spacing: Option<f32>,
}

impl<'a, Message: 'a> CheckboxFieldBuilder<'a, Message> {
    pub fn new(label: &'a str, is_checked: bool, on_toggle: impl Fn(bool) -> Message + 'a) -> Self {
        Self {
            label,
            is_checked,
            on_toggle: Box::new(on_toggle),
            tooltip_text: None,
            text_size: None,
            spacing: None,
        }
    }

    pub fn tooltip(mut self, text: &'a str) -> Self {
        self.tooltip_text = Some(text);
        self
    }

    pub fn text_size(mut self, size: f32) -> Self {
        self.text_size = Some(size);
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = Some(spacing);
        self
    }

    pub fn into_element(self) -> Element<'a, Message>
    where
        Message: Clone,
    {
        let mut cb = checkbox(self.is_checked)
            .label(self.label)
            .on_toggle(self.on_toggle);

        if let Some(size) = self.text_size {
            cb = cb.text_size(size);
        }
        if let Some(sp) = self.spacing {
            cb = cb.spacing(sp);
        }

        let element: Element<'a, Message> = cb.into();

        match self.tooltip_text {
            Some(tip) => tooltip(
                element,
                container(text(tip))
                    .style(style::tooltip)
                    .padding(tokens::spacing::MD),
                tooltip::Position::Top,
            )
            .into(),
            None => element,
        }
    }
}

impl<'a, Message: Clone + 'a> From<CheckboxFieldBuilder<'a, Message>> for Element<'a, Message> {
    fn from(builder: CheckboxFieldBuilder<'a, Message>) -> Self {
        builder.into_element()
    }
}
