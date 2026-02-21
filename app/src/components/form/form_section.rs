use iced::Element;
use iced::widget::{column, rule};

use crate::components::layout::section_header::SectionHeaderBuilder;
use crate::style;
use crate::style::tokens;

/// Builder for a group of form fields under a section header.
pub struct FormSectionBuilder<'a, Message> {
    title: String,
    fields: Vec<Element<'a, Message>>,
    spacing: f32,
    with_top_divider: bool,
    with_header_divider: bool,
    header_trailing: Option<Element<'a, Message>>,
}

impl<'a, Message: 'a> FormSectionBuilder<'a, Message> {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            fields: Vec::new(),
            spacing: tokens::spacing::LG,
            with_top_divider: false,
            with_header_divider: true,
            header_trailing: None,
        }
    }

    /// Add a field (or any element) to the section body.
    pub fn push(mut self, field: impl Into<Element<'a, Message>>) -> Self {
        self.fields.push(field.into());
        self
    }

    /// Override vertical spacing between fields.
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    /// When true, draw a horizontal rule above the section header.
    pub fn with_top_divider(mut self, show: bool) -> Self {
        self.with_top_divider = show;
        self
    }

    /// When true, draw a divider line under the section header title.
    pub fn with_header_divider(mut self, show: bool) -> Self {
        self.with_header_divider = show;
        self
    }

    /// Add a trailing element to the section header row.
    pub fn header_trailing(mut self, element: impl Into<Element<'a, Message>>) -> Self {
        self.header_trailing = Some(element.into());
        self
    }

    pub fn into_element(self) -> Element<'a, Message> {
        let mut outer = column![].spacing(tokens::spacing::MD);

        if self.with_top_divider {
            outer = outer.push(rule::horizontal(1).style(style::split_ruler));
        }

        let mut header = SectionHeaderBuilder::new(self.title)
            .with_divider(self.with_header_divider);

        if let Some(trailing) = self.header_trailing {
            header = header.trailing(trailing);
        }

        outer = outer.push(header.into_element());

        let mut fields_col = column![].spacing(self.spacing);
        for field in self.fields {
            fields_col = fields_col.push(field);
        }

        outer = outer.push(fields_col);

        outer.into()
    }
}

impl<'a, Message: 'a> From<FormSectionBuilder<'a, Message>> for Element<'a, Message> {
    fn from(builder: FormSectionBuilder<'a, Message>) -> Self {
        builder.into_element()
    }
}
