//! Stepper control: [-] value [+]

use iced::Element;
use iced::widget::{button, row, text};

use crate::style;
use crate::style::tokens;

type FormatFn<T> = Box<dyn Fn(&T) -> String>;

pub struct StepperBuilder<'a, T, Message> {
    value: T,
    min: T,
    max: T,
    step: T,
    on_change: Box<dyn Fn(T) -> Message + 'a>,
    label: Option<&'a str>,
    format: Option<FormatFn<T>>,
}

impl<'a, T, Message> StepperBuilder<'a, T, Message>
where
    T: Copy + PartialOrd + std::ops::Add<Output = T> + std::ops::Sub<Output = T> + 'static,
    Message: Clone + 'a,
{
    pub fn new(value: T, min: T, max: T, step: T, on_change: impl Fn(T) -> Message + 'a) -> Self {
        Self {
            value,
            min,
            max,
            step,
            on_change: Box::new(on_change),
            label: None,
            format: None,
        }
    }

    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn step(mut self, step: T) -> Self {
        self.step = step;
        self
    }

    pub fn format(mut self, f: impl Fn(&T) -> String + 'static) -> Self {
        self.format = Some(Box::new(f));
        self
    }

    pub fn into_element(self) -> Element<'a, Message>
    where
        T: std::fmt::Display,
    {
        let value_str = match &self.format {
            Some(f) => f(&self.value),
            None => format!("{}", self.value),
        };

        let can_dec = self.value > self.min;
        let can_inc = self.value < self.max;

        let dec_value = if can_dec {
            let v = self.value - self.step;
            if v < self.min { self.min } else { v }
        } else {
            self.value
        };

        let inc_value = if can_inc {
            let v = self.value + self.step;
            if v > self.max { self.max } else { v }
        } else {
            self.value
        };

        let mut dec_btn = button(text("-").size(tokens::text::BODY))
            .padding([tokens::spacing::XXS, tokens::spacing::SM])
            .style(style::button::secondary);

        if can_dec {
            dec_btn = dec_btn.on_press((self.on_change)(dec_value));
        }

        let mut inc_btn = button(text("+").size(tokens::text::BODY))
            .padding([tokens::spacing::XXS, tokens::spacing::SM])
            .style(style::button::secondary);

        if can_inc {
            inc_btn = inc_btn.on_press((self.on_change)(inc_value));
        }

        let value_text = text(value_str)
            .size(tokens::text::BODY)
            .align_x(iced::Alignment::Center);

        let mut r = row![];

        if let Some(lbl) = self.label {
            r = r.push(text(lbl).size(tokens::text::LABEL));
        }

        r = r.push(dec_btn).push(value_text).push(inc_btn);

        r.spacing(tokens::spacing::XS)
            .align_y(iced::Alignment::Center)
            .into()
    }
}

impl<'a, T, Message> From<StepperBuilder<'a, T, Message>> for Element<'a, Message>
where
    T: Copy
        + PartialOrd
        + std::ops::Add<Output = T>
        + std::ops::Sub<Output = T>
        + std::fmt::Display
        + 'static,
    Message: Clone + 'a,
{
    fn from(builder: StepperBuilder<'a, T, Message>) -> Self {
        builder.into_element()
    }
}
