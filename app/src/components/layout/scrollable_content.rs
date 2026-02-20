use iced::Element;
use iced::widget::scrollable;

use crate::style::tokens;

pub fn scrollable_content<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    scrollable::Scrollable::with_direction(
        content,
        scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(tokens::layout::SCROLLBAR_WIDTH)
                .scroller_width(tokens::layout::SCROLLBAR_WIDTH),
        ),
    )
    .into()
}
