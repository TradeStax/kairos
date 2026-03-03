//! FormRow builder — a horizontal label : control row with configurable label width.

use iced::widget::{container, row, text};
use iced::{Alignment, Element, Length};

use crate::style::tokens;

/// Builder for a horizontal label : control row.
///
/// # Usage
/// ```ignore
/// // Default label width (tokens::component::form::LABEL_WIDTH):
/// FormRow::new("Label", control).into_element()
///
/// // Narrow label (compact modals):
/// FormRow::new("Label", control)
///     .label_width(tokens::component::form::LABEL_WIDTH_NARROW)
///     .into_element()
/// ```
pub struct FormRow<'a, Message> {
    label: String,
    control: Element<'a, Message>,
    label_width: f32,
}

impl<'a, Message: 'a> FormRow<'a, Message> {
    pub fn new(label: impl Into<String>, control: impl Into<Element<'a, Message>>) -> Self {
        Self {
            label: label.into(),
            control: control.into(),
            label_width: tokens::component::form::LABEL_WIDTH,
        }
    }

    /// Override the fixed label column width.
    /// Default: `tokens::component::form::LABEL_WIDTH` (120px).
    pub fn label_width(mut self, width: f32) -> Self {
        self.label_width = width;
        self
    }

    pub fn into_element(self) -> Element<'a, Message> {
        let label_widget =
            container(text(self.label).size(tokens::text::LABEL)).width(self.label_width);

        row![label_widget, self.control]
            .spacing(tokens::spacing::MD)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .into()
    }
}

impl<'a, Message: 'a> From<FormRow<'a, Message>> for Element<'a, Message> {
    fn from(row: FormRow<'a, Message>) -> Self {
        row.into_element()
    }
}
