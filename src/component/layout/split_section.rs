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

/// Creates a column with horizontal rules between each item.
///
/// # Examples
/// ```ignore
/// split_column![
///     text("Item 1"),
///     text("Item 2"),
///     text("Item 3"),
/// ] ; spacing = 8, align_x = Alignment::Start
/// ```
#[macro_export]
macro_rules! split_column {
    () => {
        column![]
    };

    ($item:expr $(,)?) => {
        column![$item]
    };

    ($first:expr, $($rest:expr),+ $(,)?) => {{
        let mut col = column![$first];
        $(
            col = col.push(iced::widget::rule::horizontal(1.0).style($crate::style::split_ruler));
            col = col.push($rest);
        )+
        col
    }};

    ($($item:expr),* $(,)?; spacing = $spacing:expr) => {{
        $crate::split_column![$($item),*].spacing($spacing)
    }};

    ($($item:expr),* $(,)?; spacing = $spacing:expr, align_x = $align:expr) => {{
        $crate::split_column![$($item),*].spacing($spacing).align_x($align)
    }};
}
