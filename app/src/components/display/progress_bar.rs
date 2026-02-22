//! Themed progress bar with optional label and percentage.

use iced::Element;
use iced::widget::{column, progress_bar, row, text};
use iced_anim::AnimationBuilder;

use crate::style;
use crate::style::{animation, tokens};

pub struct ProgressBarBuilder<'a, Message> {
    value: f32,
    max: f32,
    show_percentage: bool,
    label: Option<&'a str>,
    girth: f32,
    _message: std::marker::PhantomData<Message>,
}

impl<'a, Message: 'a> ProgressBarBuilder<'a, Message> {
    pub fn new(value: f32, max: f32) -> Self {
        Self {
            value,
            max,
            show_percentage: false,
            label: None,
            girth: 8.0,
            _message: std::marker::PhantomData,
        }
    }

    pub fn show_percentage(mut self, show: bool) -> Self {
        self.show_percentage = show;
        self
    }

    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn girth(mut self, height: f32) -> Self {
        self.girth = height;
        self
    }

    pub fn into_element(self) -> Element<'a, Message>
    where
        Message: Clone,
    {
        let max = self.max;
        let girth = self.girth;
        let show_percentage = self.show_percentage;
        let label = self.label.map(|l| l.to_string());

        AnimationBuilder::new(self.value, move |current_value| {
            let bar = progress_bar(0.0..=max, current_value)
                .girth(girth)
                .style(style::progress_bar);

            let mut content = column![].spacing(tokens::spacing::XXS);

            if let Some(ref lbl) = label {
                let mut header =
                    row![text(lbl.clone()).size(tokens::text::SMALL)].spacing(tokens::spacing::XS);

                if show_percentage {
                    let pct = if max > 0.0 {
                        (current_value / max * 100.0) as u32
                    } else {
                        0
                    };
                    header = header.push(text(format!("{pct}%")).size(tokens::text::TINY));
                }

                content = content.push(header);
            } else if show_percentage {
                let pct = if max > 0.0 {
                    (current_value / max * 100.0) as u32
                } else {
                    0
                };
                content = content.push(text(format!("{pct}%")).size(tokens::text::TINY));
            }

            content = content.push(bar);

            content.into()
        })
        .animation(animation::spring::SUBTLE)
        .animates_layout(false)
        .into()
    }
}

impl<'a, Message: Clone + 'a> From<ProgressBarBuilder<'a, Message>> for Element<'a, Message> {
    fn from(builder: ProgressBarBuilder<'a, Message>) -> Self {
        builder.into_element()
    }
}
