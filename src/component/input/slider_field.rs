//! Labeled slider with value display.

use iced::widget::{column, container, row, slider, space, text};
use iced::{
    Alignment::{self, Center},
    Color, Element,
    Length::{self, Fill},
    Theme, border,
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

pub fn classic_slider_row<'a, Message>(
    label: iced::widget::Text<'a>,
    slider: Element<'a, Message>,
    placeholder: Option<iced::widget::Text<'a>>,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let slider = if let Some(placeholder) = placeholder {
        column![slider, placeholder]
            .spacing(tokens::spacing::XXS)
            .align_x(Alignment::Center)
    } else {
        column![slider]
    };

    container(
        row![label, slider]
            .align_y(Alignment::Center)
            .spacing(tokens::spacing::MD)
            .padding(tokens::spacing::MD),
    )
    .style(style::modal_container)
    .into()
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
        .style(|theme: &Theme, status| {
            let palette = theme.extended_palette();

            slider::Style {
                rail: slider::Rail {
                    backgrounds: (
                        palette.background.strong.color.into(),
                        Color::TRANSPARENT.into(),
                    ),
                    width: 24.0,
                    border: border::rounded(2),
                },
                handle: slider::Handle {
                    shape: slider::HandleShape::Rectangle {
                        width: 2,
                        border_radius: 2.0.into(),
                    },
                    background: match status {
                        iced::widget::slider::Status::Active => {
                            palette.background.strong.color.into()
                        }
                        iced::widget::slider::Status::Hovered => {
                            palette.primary.base.color.into()
                        }
                        iced::widget::slider::Status::Dragged => {
                            palette.primary.weak.color.into()
                        }
                    },
                    border_width: 0.0,
                    border_color: Color::TRANSPARENT,
                },
            }
        });

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
