//! Labeled pick-list dropdown.

use iced::widget::{column, pick_list, text};
use iced::{Element, Length};

use crate::style::tokens;

pub struct DropdownBuilder<'a, T, Message> {
    label: &'a str,
    options: &'a [T],
    selected: Option<T>,
    on_selected: Box<dyn Fn(T) -> Message + 'a>,
    placeholder: Option<&'a str>,
    text_size: Option<f32>,
    width: Option<Length>,
}

impl<'a, T, Message> DropdownBuilder<'a, T, Message>
where
    T: Clone + std::fmt::Display + PartialEq + 'a,
    Message: 'a,
{
    pub fn new(
        label: &'a str,
        options: &'a [T],
        selected: Option<T>,
        on_selected: impl Fn(T) -> Message + 'a,
    ) -> Self {
        Self {
            label,
            options,
            selected,
            on_selected: Box::new(on_selected),
            placeholder: None,
            text_size: None,
            width: None,
        }
    }

    pub fn placeholder(mut self, text: &'a str) -> Self {
        self.placeholder = Some(text);
        self
    }

    pub fn text_size(mut self, size: f32) -> Self {
        self.text_size = Some(size);
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
        let label_widget = text(self.label).size(tokens::text::LABEL);

        let mut pl = pick_list(self.options, self.selected, self.on_selected);

        if let Some(p) = self.placeholder {
            pl = pl.placeholder(p);
        }
        if let Some(s) = self.text_size {
            pl = pl.text_size(s);
        }
        if let Some(w) = self.width {
            pl = pl.width(w);
        }

        column![label_widget, pl]
            .spacing(tokens::spacing::XS)
            .into()
    }
}

impl<'a, T, Message> From<DropdownBuilder<'a, T, Message>> for Element<'a, Message>
where
    T: Clone + std::fmt::Display + PartialEq + 'a,
    Message: Clone + 'a,
{
    fn from(builder: DropdownBuilder<'a, T, Message>) -> Self {
        builder.into_element()
    }
}
