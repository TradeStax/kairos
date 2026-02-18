use iced::widget::{column, row, rule, space, text};
use iced::{Alignment, Element};

use crate::style;
use crate::style::tokens;

/// Builds a section heading row with optional trailing control and divider.
pub struct SectionHeaderBuilder<'a, Message> {
    label: String,
    trailing: Option<Element<'a, Message>>,
    with_divider: bool,
    _message: std::marker::PhantomData<&'a Message>,
}

impl<'a, Message: 'a> SectionHeaderBuilder<'a, Message> {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            trailing: None,
            with_divider: false,
            _message: std::marker::PhantomData,
        }
    }

    /// Add a trailing element (e.g. a button or toggle) on the right side.
    pub fn trailing(mut self, element: impl Into<Element<'a, Message>>) -> Self {
        self.trailing = Some(element.into());
        self
    }

    /// When true, a horizontal rule is drawn below the header.
    pub fn with_divider(mut self, show: bool) -> Self {
        self.with_divider = show;
        self
    }

    pub fn into_element(self) -> Element<'a, Message> {
        let title = text(self.label).size(tokens::text::LABEL);

        let header_row: Element<'a, Message> = if let Some(trailing) = self.trailing {
            row![title, space::horizontal(), trailing]
                .align_y(Alignment::Center)
                .spacing(tokens::spacing::MD)
                .into()
        } else {
            row![title].align_y(Alignment::Center).into()
        };

        if self.with_divider {
            column![header_row, rule::horizontal(1).style(style::split_ruler),]
                .spacing(tokens::spacing::XS)
                .into()
        } else {
            header_row
        }
    }
}

impl<'a, Message: 'a> From<SectionHeaderBuilder<'a, Message>> for Element<'a, Message> {
    fn from(builder: SectionHeaderBuilder<'a, Message>) -> Self {
        builder.into_element()
    }
}
