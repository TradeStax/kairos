pub mod ladder;
pub mod timeandsales;

use crate::style::tokens;
use iced::{
    Element, padding,
    widget::{canvas, center, container, text},
};
use std::time::Instant;

#[derive(Debug, Clone, Copy)]
pub enum Message {
    Scrolled(f32),
    ResetScroll,
    Invalidate(Option<Instant>),
}

/// Placeholder for future panel-level actions (e.g. scroll-to-trade).
/// Referenced by `pane::Action::Panel` and the `Panel` trait.
pub enum Action {}

pub trait Panel: canvas::Program<Message> {
    fn scroll(&mut self, scroll: f32);

    fn reset_scroll(&mut self);

    fn invalidate(&mut self, now: Option<Instant>) -> Option<Action>;

    fn is_empty(&self) -> bool;
}

pub fn view<T: Panel>(panel: &'_ T, _timezone: data::UserTimezone) -> Element<'_, Message> {
    if panel.is_empty() {
        return center(text("Waiting for data...").size(tokens::text::HEADING)).into();
    }

    container(
        canvas(panel)
            .height(iced::Length::Fill)
            .width(iced::Length::Fill),
    )
    .padding(
        padding::left(tokens::spacing::XXXS)
            .right(tokens::spacing::XXXS)
            .bottom(tokens::spacing::XXXS),
    )
    .into()
}

pub fn update<T: Panel>(panel: &mut T, message: Message) {
    match message {
        Message::Scrolled(delta) => {
            panel.scroll(delta);
        }
        Message::ResetScroll => {
            panel.reset_scroll();
        }
        Message::Invalidate(now) => {
            panel.invalidate(now);
        }
    }
}
