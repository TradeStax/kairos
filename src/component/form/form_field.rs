use iced::widget::{column, container, row, text};
use iced::{Alignment, Element, Length};

use crate::style::tokens;

/// Builder for a labelled form control with optional validation error and
/// tooltip.
pub struct FormFieldBuilder<'a, Message> {
    label: String,
    control: Element<'a, Message>,
    error: Option<String>,
    tooltip_text: Option<String>,
    required: bool,
    label_width: Option<Length>,
    horizontal: bool,
    _message: std::marker::PhantomData<&'a Message>,
}

impl<'a, Message: 'a> FormFieldBuilder<'a, Message> {
    pub fn new(label: impl Into<String>, control: impl Into<Element<'a, Message>>) -> Self {
        Self {
            label: label.into(),
            control: control.into(),
            error: None,
            tooltip_text: None,
            required: false,
            label_width: None,
            horizontal: false,
            _message: std::marker::PhantomData,
        }
    }

    /// Show a validation error below the control.
    pub fn error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// Show a tooltip icon next to the label with explanatory text.
    pub fn tooltip(mut self, tip: impl Into<String>) -> Self {
        self.tooltip_text = Some(tip.into());
        self
    }

    /// Mark the field as required (appends `*` to the label).
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Set a fixed width for the label column (horizontal mode).
    pub fn label_width(mut self, width: impl Into<Length>) -> Self {
        self.label_width = Some(width.into());
        self
    }

    /// Lay out label and control side by side instead of stacked.
    pub fn horizontal(mut self, horizontal: bool) -> Self {
        self.horizontal = horizontal;
        self
    }

    pub fn into_element(self) -> Element<'a, Message> {
        let label_text = if self.required {
            format!("{} *", self.label)
        } else {
            self.label
        };

        let label_widget: Element<'a, Message> = text(label_text).size(tokens::text::LABEL).into();

        let label_widget: Element<'a, Message> = if let Some(lw) = self.label_width {
            container(label_widget).width(lw).into()
        } else {
            label_widget
        };

        // Build the control column (control + optional error)
        let mut control_col = column![].spacing(tokens::spacing::XXS);
        control_col = control_col.push(self.control);

        if let Some(err) = self.error {
            control_col = control_col.push(
                text(err)
                    .size(tokens::text::TINY)
                    .color(iced::Color::from_rgb(0.9, 0.2, 0.2)),
            );
        }

        if self.horizontal {
            row![label_widget, control_col]
                .spacing(tokens::spacing::MD)
                .align_y(Alignment::Center)
                .into()
        } else {
            column![label_widget, control_col]
                .spacing(tokens::spacing::XS)
                .into()
        }
    }
}

impl<'a, Message: 'a> From<FormFieldBuilder<'a, Message>> for Element<'a, Message> {
    fn from(builder: FormFieldBuilder<'a, Message>) -> Self {
        builder.into_element()
    }
}
