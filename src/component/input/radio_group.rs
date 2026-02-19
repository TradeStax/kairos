//! Group of radio buttons laid out in a row or column.

use iced::Element;
use iced::widget::{column, radio, row};

use crate::style::tokens;

/// Layout direction for radio group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Row,
    Column,
}

pub struct RadioGroupBuilder<'a, V, Message> {
    options: &'a [(V, &'a str)],
    selected: Option<V>,
    on_select: Box<dyn Fn(V) -> Message + 'a>,
    direction: Direction,
    spacing: f32,
}

impl<'a, V, Message> RadioGroupBuilder<'a, V, Message>
where
    V: Copy + Eq + 'a,
    Message: 'a,
{
    pub fn new(
        options: &'a [(V, &'a str)],
        selected: Option<V>,
        on_select: impl Fn(V) -> Message + 'a,
    ) -> Self {
        Self {
            options,
            selected,
            on_select: Box::new(on_select),
            direction: Direction::Column,
            spacing: tokens::spacing::MD,
        }
    }

    pub fn direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn into_element(self) -> Element<'a, Message>
    where
        Message: Clone,
    {
        let items: Vec<Element<'a, Message>> = self
            .options
            .iter()
            .map(|(value, label)| {
                let v = *value;
                radio(*label, v, self.selected, |val| (self.on_select)(val))
                    .size(14)
                    .spacing(tokens::spacing::XS)
                    .into()
            })
            .collect();

        match self.direction {
            Direction::Row => row(items).spacing(self.spacing).into(),
            Direction::Column => column(items).spacing(self.spacing).into(),
        }
    }
}

impl<'a, V, Message> From<RadioGroupBuilder<'a, V, Message>> for Element<'a, Message>
where
    V: Copy + Eq + 'a,
    Message: Clone + 'a,
{
    fn from(builder: RadioGroupBuilder<'a, V, Message>) -> Self {
        builder.into_element()
    }
}
