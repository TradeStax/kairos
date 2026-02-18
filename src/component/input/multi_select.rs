//! Multi-select: a panel of checkboxes for selecting multiple items.

use std::rc::Rc;

use iced::widget::{checkbox, column, text};
use iced::{Element, Renderer, Theme};

use crate::style::tokens;

/// Renders a labeled column of checkboxes for multi-selection.
pub fn multi_select<'a, T, Message>(
    label: &'a str,
    options: &'a [(T, &'a str, bool)],
    on_toggle: impl Fn(usize, bool) -> Message + 'a,
) -> Element<'a, Message, Theme, Renderer>
where
    T: 'a,
    Message: Clone + 'a,
{
    let label_widget = text(label).size(tokens::text::LABEL);

    let on_toggle = Rc::new(on_toggle);

    let mut col = column![label_widget].spacing(tokens::spacing::XS);

    for (idx, (_value, display, checked)) in options.iter().enumerate() {
        let toggle_fn = Rc::clone(&on_toggle);
        let cb = checkbox(*checked)
            .label(*display)
            .on_toggle(move |val| toggle_fn(idx, val))
            .text_size(tokens::text::BODY)
            .spacing(tokens::spacing::XS);

        col = col.push(cb);
    }

    col.into()
}
