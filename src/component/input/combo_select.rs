//! Combo-select: a labeled pick_list used as a combo-box substitute.
//!
//! Iced's `combo_box` widget requires the `"lazy"` feature which is not
//! enabled in this project, so we wrap `pick_list` with a search-like UX
//! instead.

use iced::widget::{column, pick_list, text};
use iced::{Element, Renderer, Theme};

use crate::style::tokens;

/// A labeled pick-list that acts as a simple combo-select.
pub fn combo_select<'a, T, Message>(
    label: &'a str,
    options: &'a [T],
    selected: Option<T>,
    on_selected: impl Fn(T) -> Message + 'a,
) -> Element<'a, Message, Theme, Renderer>
where
    T: Clone + std::fmt::Display + PartialEq + 'a,
    Message: Clone + 'a,
{
    let label_widget = text(label).size(tokens::text::LABEL);
    let pl = pick_list(options, selected, on_selected).placeholder("Select...");

    column![label_widget, pl]
        .spacing(tokens::spacing::XS)
        .into()
}
