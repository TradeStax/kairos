use iced::Element;
use iced::widget::{column, rule};

use crate::style;
use crate::style::tokens;

/// Build a column with horizontal rule dividers between each item.
///
/// This is the function equivalent of the
/// [`split_column!`](crate::split_column) macro, useful when items are
/// collected at runtime.
pub fn split_section<'a, Message: 'a>(items: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let len = items.len();
    let mut col = column![].spacing(tokens::spacing::MD);

    for (i, item) in items.into_iter().enumerate() {
        col = col.push(item);
        if i + 1 < len {
            col = col.push(rule::horizontal(1).style(style::split_ruler));
        }
    }

    col.into()
}
