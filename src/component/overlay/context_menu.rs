use iced::widget::{button, column, container, mouse_area, opaque, text};
use iced::{Element, Length, Padding, Point};

use crate::style;
use crate::style::tokens;

/// A single item in the context menu.
///
/// When `action` is `None` the item is rendered as a disabled label.
struct ContextMenuItem<Message> {
    label: String,
    action: Option<Message>,
}

/// Build a right-click context menu overlay.
///
/// * `items`    -- list of (label, optional-message) pairs.
/// * `position` -- where the context menu should appear.
/// * `on_close` -- sent when the backdrop is clicked (dismiss).
pub fn context_menu<'a, Message: Clone + 'a>(
    items: Vec<(String, Option<Message>)>,
    position: Point,
    on_close: Message,
) -> Element<'a, Message> {
    let mut col = column![].spacing(tokens::spacing::XXS);

    for (label, action) in items {
        let btn = if let Some(msg) = action {
            button(text(label).size(tokens::text::BODY))
                .width(Length::Fill)
                .padding([tokens::spacing::XS, tokens::spacing::MD])
                .on_press(msg)
                .style(|theme, status| style::button::pick_list_item(theme, status))
        } else {
            button(text(label).size(tokens::text::BODY))
                .width(Length::Fill)
                .padding([tokens::spacing::XS, tokens::spacing::MD])
                .style(|theme, status| style::button::pick_list_item(theme, status))
        };
        col = col.push(btn);
    }

    let menu = container(col)
        .padding(tokens::spacing::XS)
        .style(style::dropdown_container);

    let positioned = container(opaque(menu))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top: position.y,
            right: 0.0,
            bottom: 0.0,
            left: position.x,
        });

    mouse_area(positioned).on_press(on_close).into()
}
