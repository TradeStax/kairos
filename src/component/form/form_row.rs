use iced::widget::{container, row, text};
use iced::{Alignment, Element, Length};

use crate::style::tokens;

/// Create a horizontal label : control row.
///
/// A convenience wrapper around a `Row` with a fixed-width label and a
/// fill-width control area.
pub fn form_row<'a, Message: 'a>(
    label: impl Into<String>,
    control: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let label_widget = container(text(label.into()).size(tokens::text::LABEL)).width(120);

    row![label_widget, control.into()]
        .spacing(tokens::spacing::MD)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
}
