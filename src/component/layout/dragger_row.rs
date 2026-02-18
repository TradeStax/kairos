use iced::widget::{container, row};
use iced::{Alignment, Element};

use crate::component::primitives::{Icon, icon_text};
use crate::style::{self, tokens};

pub fn dragger_row<'a, Message>(
    content: Element<'a, Message>,
    is_enabled: bool,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let content = if is_enabled {
        let icon = icon_text(Icon::DragHandle, 11);
        row![icon, content,]
            .align_y(Alignment::Center)
            .spacing(tokens::spacing::XXS)
            .into()
    } else {
        content
    };

    container(content)
        .padding(tokens::spacing::XXS)
        .style(style::dragger_row_container)
        .into()
}
