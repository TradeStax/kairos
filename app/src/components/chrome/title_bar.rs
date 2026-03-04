use crate::components::primitives::icons::{Icon, icon_text};
use crate::style::{self, tokens};
use crate::window;

use iced::widget::{button, container, mouse_area, row, space, text};
use iced::{Alignment, Element, Length, mouse, padding};

/// Messages produced by the title bar. The caller maps these to their
/// own message type via the `map` parameter.
#[allow(dead_code)]
pub enum Action {
    Drag(window::Id),
    Minimize(window::Id),
    ToggleMaximize(window::Id),
    Close(window::Id),
    Hover(bool),
}

#[allow(dead_code)]
pub fn view_title_bar<'a, Message: 'a + Clone>(
    window_id: window::Id,
    title: String,
    is_maximized: bool,
    hovered: bool,
    map: impl Fn(Action) -> Message + 'a,
) -> Element<'a, Message> {
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
    .width(tokens::component::button::WINDOW_CONTROL_WIDTH)
    .height(tokens::layout::TITLE_BAR_HEIGHT)
    .on_press(map(Action::Minimize(window_id)))
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
    .width(tokens::component::button::WINDOW_CONTROL_WIDTH)
    .height(tokens::layout::TITLE_BAR_HEIGHT)
    .on_press(map(Action::ToggleMaximize(window_id)))
    .style(style::button::window_control);

    let close_btn = button(
        container(icon_text(Icon::Close, 10))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(tokens::component::button::WINDOW_CONTROL_WIDTH)
    .height(tokens::layout::TITLE_BAR_HEIGHT)
    .on_press(map(Action::Close(window_id)))
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
    .style(move |theme| style::window_title_bar(theme, hovered));

    mouse_area(bar)
        .on_press(map(Action::Drag(window_id)))
        .on_double_click(map(Action::ToggleMaximize(window_id)))
        .on_enter(map(Action::Hover(true)))
        .on_exit(map(Action::Hover(false)))
        .interaction(mouse::Interaction::Grab)
        .into()
}
