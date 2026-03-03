//! Labeled slider with value display.

use iced::widget::{container, row, slider, space, text};
use iced::{
    Center, Element,
    Length::{self, Fill},
};

use crate::style::{self, tokens};

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
            .height(tokens::layout::SLIDER_HEIGHT);

        if let Some(step) = self.step {
            s = s.step(step);
        }

        let slider_group = row![s, text(value_str).size(tokens::text::SMALL)]
            .spacing(tokens::spacing::SM)
            .align_y(iced::Alignment::Center)
            .width(Length::FillPortion(3));

        let content = row![
            text(self.label).size(tokens::text::LABEL),
            space::horizontal(),
            slider_group,
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

/// A row combining a label, a slider widget, and an optional value display.
///
/// Used by feature-gated heatmap/ladder settings panels.
#[cfg(feature = "heatmap")]
pub fn classic_slider_row<'a, Message: Clone + 'a>(
    label: impl Into<Element<'a, Message>>,
    slider_widget: Element<'a, Message>,
    value_label: Option<impl Into<Element<'a, Message>>>,
) -> Element<'a, Message> {
    let mut r = row![label.into(), slider_widget]
        .spacing(tokens::spacing::SM)
        .align_y(iced::Alignment::Center);
    if let Some(val) = value_label {
        r = r.push(val.into());
    }
    r.into()
}

pub fn labeled_slider<'a, T, Message: Clone + 'static>(
    label: impl text::IntoFragment<'a>,
    range: std::ops::RangeInclusive<T>,
    current: T,
    on_change: impl Fn(T) -> Message + 'a,
    to_string: impl Fn(&T) -> String,
    step: Option<T>,
) -> Element<'a, Message>
where
    T: 'static + Copy + PartialOrd + Into<f64> + From<u8> + num_traits::FromPrimitive,
{
    let mut slider = iced::widget::slider(range, current, on_change)
        .width(Fill)
        .height(tokens::layout::SLIDER_HEIGHT)
        .style(style::slider::flat);

    if let Some(v) = step {
        slider = slider.step(v);
    }

    iced::widget::stack![
        container(slider).style(style::modal_container),
        row![text(label), space::horizontal(), text(to_string(&current))]
            .padding([0.0, tokens::spacing::LG])
            .height(Fill)
            .align_y(Center),
    ]
    .into()
}
