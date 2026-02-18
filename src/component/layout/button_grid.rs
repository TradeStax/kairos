use iced::widget::{button, column, row, text};
use iced::{Element, Length, Theme};

use crate::style;
use crate::style::tokens;

/// Lay out a grid of toggle-style buttons.
///
/// * `items`       -- the data items to render.
/// * `columns`     -- how many buttons per row.
/// * `selected`    -- index of the currently selected item (or `None`).
/// * `on_selected` -- factory that creates a `Message` for the selected
///   index.
/// * `label_fn`    -- produces the display label for each item.
pub fn button_grid<'a, T, Message, F, L>(
    items: &[T],
    columns: usize,
    selected: Option<usize>,
    on_selected: F,
    label_fn: L,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
    F: Fn(usize) -> Message + 'a + Clone,
    L: Fn(&T) -> String,
{
    let columns = columns.max(1);

    let mut col = column![].spacing(tokens::spacing::XS);
    let mut current_row = row![].spacing(tokens::spacing::XS);
    let mut col_count = 0;

    for (i, item) in items.iter().enumerate() {
        let label = label_fn(item);
        let is_selected = selected == Some(i);
        let msg = on_selected.clone()(i);

        let btn = button(text(label).size(tokens::text::BODY))
            .width(Length::Fill)
            .padding([tokens::spacing::XS, tokens::spacing::SM])
            .on_press(msg)
            .style(move |theme: &Theme, status| {
                style::button::bordered_toggle(theme, status, is_selected)
            });

        current_row = current_row.push(btn);
        col_count += 1;

        if col_count >= columns {
            col = col.push(current_row);
            current_row = row![].spacing(tokens::spacing::XS);
            col_count = 0;
        }
    }

    // Push any remaining partial row.
    if col_count > 0 {
        col = col.push(current_row);
    }

    col.into()
}
