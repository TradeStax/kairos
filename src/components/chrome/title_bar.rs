use crate::components::primitives::icons::{Icon, icon_text};
use crate::style::{self, tokens};
use crate::window;

use iced::widget::{button, container, mouse_area, row, space, text};
use iced::{Alignment, Element, Length, padding};

use crate::app::Message;

pub fn view_title_bar(
    window_id: window::Id,
    title: String,
    is_maximized: bool,
) -> Element<'static, Message> {
    let title_text = text(title)
        .font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        })
        .size(tokens::text::BODY);

    let minimize_btn = button(
        container(icon_text(Icon::Minimize, 10))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(46)
    .height(tokens::layout::TITLE_BAR_HEIGHT)
    .on_press(Message::WindowMinimize(window_id))
    .style(style::button::window_control);

    let maximize_icon = if is_maximized {
        Icon::ResizeSmall
    } else {
        Icon::ResizeFull
    };
    let maximize_btn = button(
        container(icon_text(maximize_icon, 10))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(46)
    .height(tokens::layout::TITLE_BAR_HEIGHT)
    .on_press(Message::WindowToggleMaximize(window_id))
    .style(style::button::window_control);

    let close_btn = button(
        container(icon_text(Icon::Close, 10))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(46)
    .height(tokens::layout::TITLE_BAR_HEIGHT)
    .on_press(Message::WindowClose(window_id))
    .style(style::button::window_close);

    let controls = row![minimize_btn, maximize_btn, close_btn].spacing(tokens::spacing::XXS);

    let bar = container(
        row![
            title_text,
            space::horizontal().width(Length::Fill),
            controls,
        ]
        .align_y(Alignment::Center)
        .padding(padding::left(tokens::spacing::MD)),
    )
    .width(Length::Fill)
    .height(tokens::layout::TITLE_BAR_HEIGHT)
    .style(style::window_title_bar);

    mouse_area(bar)
        .on_press(Message::WindowDrag(window_id))
        .on_double_click(Message::WindowToggleMaximize(window_id))
        .into()
}
