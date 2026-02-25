pub mod form_field;
pub mod form_row;
pub mod form_section;

pub use form_row::FormRow;

use iced::Element;

/// Create a horizontal label : control row with default label width.
///
/// Shim over `FormRow::new(label, control).into_element()`.
/// For a non-default label width use `FormRow::new(...).label_width(...).into_element()`.
pub fn form_row<'a, Message: 'a>(
    label: impl Into<String>,
    control: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    FormRow::new(label, control).into_element()
}
