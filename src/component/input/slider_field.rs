//! Labeled slider with value display.

use iced::widget::{container, row, slider, text};
use iced::{Element, Length};

use crate::style;
use crate::style::tokens;

type FormatFn<T> = Box<dyn Fn(&T) -> String>;

pub struct SliderFieldBuilder<'a, T, Message> {
    label: &'a str,
    range: std::ops::RangeInclusive<T>,
    value: T,
    on_change: Box<dyn Fn(T) -> Message + 'a>,
    step: Option<T>,
    format: Option<FormatFn<T>>,
    in_card: bool,
}

impl<'a, T, Message> SliderFieldBuilder<'a, T, Message>
where
    T: Copy + Into<f64> + From<u8> + PartialOrd + num_traits::FromPrimitive + 'static,
    Message: Clone + 'a,
{
    pub fn new(
        label: &'a str,
        range: std::ops::RangeInclusive<T>,
        value: T,
        on_change: impl Fn(T) -> Message + 'a,
    ) -> Self {
        Self {
            label,
            range,
            value,
            on_change: Box::new(on_change),
            step: None,
            format: None,
            in_card: false,
        }
    }

    pub fn step(mut self, step: T) -> Self {
        self.step = Some(step);
        self
    }

    pub fn format(mut self, f: impl Fn(&T) -> String + 'static) -> Self {
        self.format = Some(Box::new(f));
        self
    }

    pub fn in_card(mut self, yes: bool) -> Self {
        self.in_card = yes;
        self
    }

    pub fn into_element(self) -> Element<'a, Message> {
        let value_str = match &self.format {
            Some(f) => f(&self.value),
            None => {
                let v: f64 = self.value.into();
                format!("{v:.1}")
            }
        };

        let mut s = slider(self.range, self.value, self.on_change)
            .width(Length::Fill)
            .height(24);

        if let Some(step) = self.step {
            s = s.step(step);
        }

        let content = row![
            text(self.label).size(tokens::text::LABEL),
            s,
            text(value_str).size(tokens::text::SMALL),
        ]
        .spacing(tokens::spacing::MD)
        .align_y(iced::Alignment::Center);

        if self.in_card {
            container(content)
                .padding(tokens::spacing::MD)
                .style(style::modal_container)
                .into()
        } else {
            content.into()
        }
    }
}

impl<'a, T, Message> From<SliderFieldBuilder<'a, T, Message>> for Element<'a, Message>
where
    T: Copy + Into<f64> + From<u8> + PartialOrd + num_traits::FromPrimitive + 'static,
    Message: Clone + 'a,
{
    fn from(builder: SliderFieldBuilder<'a, T, Message>) -> Self {
        builder.into_element()
    }
}
