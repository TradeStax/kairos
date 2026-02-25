pub mod button_grid;
pub mod button_group;
pub mod card;
pub mod collapsible;
pub mod decorate;
pub mod dragger_row;
pub mod interactive_card;
pub mod list_item;
pub mod multi_split;
pub mod reorderable_list;
pub mod section_header;
pub mod split_section;
pub mod toolbar;

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
