//! Icon button with optional tooltip.

use iced::widget::{button, container, text, tooltip};
use iced::{Alignment, Element, Length, Padding, Theme};

use super::{ICONS_FONT, Icon};
use crate::style;
use crate::style::tokens;

/// Shorthand: create an `IconButtonBuilder` for the given icon.
pub fn icon_button<'a, Message>(icon: Icon) -> IconButtonBuilder<'a, Message> {
    IconButtonBuilder::new(icon)
}

/// Shorthand: create a toolbar-sized icon button (small padding, 14px icon).
pub fn toolbar_icon<'a, Message>(icon: Icon, on_press: Message) -> IconButtonBuilder<'a, Message> {
    IconButtonBuilder::new(icon)
        .size(tokens::component::icon::MD)
        .padding(Padding::from(tokens::spacing::XS))
        .on_press(on_press)
}

type ButtonStyleFn = Box<dyn Fn(&Theme, button::Status) -> button::Style>;

pub struct IconButtonBuilder<'a, Message> {
    icon: Icon,
    size: f32,
    on_press: Option<Message>,
    tooltip_text: Option<&'a str>,
    tooltip_position: tooltip::Position,
    style_fn: Option<ButtonStyleFn>,
    is_active: bool,
    padding: Padding,
    width: Option<Length>,
    height: Option<Length>,
}

impl<'a, Message> IconButtonBuilder<'a, Message> {
    pub fn new(icon: Icon) -> Self {
        Self {
            icon,
            size: tokens::component::icon::LG,
            on_press: None,
            tooltip_text: None,
            tooltip_position: tooltip::Position::Bottom,
            style_fn: None,
            is_active: false,
            padding: Padding::from(tokens::spacing::SM),
            width: None,
            height: None,
        }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn on_press(mut self, message: Message) -> Self {
        self.on_press = Some(message);
        self
    }

    pub fn tooltip(mut self, text: &'a str) -> Self {
        self.tooltip_text = Some(text);
        self
    }

    pub fn tooltip_position(mut self, position: tooltip::Position) -> Self {
        self.tooltip_position = position;
        self
    }

    pub fn style(mut self, f: impl Fn(&Theme, button::Status) -> button::Style + 'static) -> Self {
        self.style_fn = Some(Box::new(f));
        self
    }

    pub fn active(mut self, is_active: bool) -> Self {
        self.is_active = is_active;
        self
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = Some(width.into());
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = Some(height.into());
        self
    }

    pub fn into_element(self) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        let icon_text = if self.icon.uses_default_font() {
            text(char::from(self.icon).to_string())
                .size(iced::Pixels(self.size))
                .line_height(1.0)
                .align_y(Alignment::Center)
        } else {
            text(char::from(self.icon).to_string())
                .font(ICONS_FONT)
                .size(iced::Pixels(self.size))
                .line_height(1.0)
                .align_y(Alignment::Center)
        };

        let sz = Length::Fixed(self.size);
        let icon_content = container(icon_text)
            .width(sz)
            .height(sz)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center);

        let is_active = self.is_active;
        let mut btn = button(icon_content).padding(self.padding);

        if let Some(style_fn) = self.style_fn {
            btn = btn.style(move |theme, status| style_fn(theme, status));
        } else {
            btn = btn
                .style(move |theme, status| style::button::transparent(theme, status, is_active));
        }

        if let Some(msg) = self.on_press {
            btn = btn.on_press(msg);
        }

        if let Some(w) = self.width {
            btn = btn.width(w);
        }

        if let Some(h) = self.height {
            btn = btn.height(h);
        }

        let btn_element: Element<'a, Message> = btn.into();

        match self.tooltip_text {
            Some(tip) => tooltip(
                btn_element,
                container(iced::widget::text(tip))
                    .style(style::tooltip)
                    .padding(tokens::spacing::MD),
                self.tooltip_position,
            )
            .into(),
            None => btn_element,
        }
    }
}

impl<'a, Message: Clone + 'a> From<IconButtonBuilder<'a, Message>> for Element<'a, Message> {
    fn from(builder: IconButtonBuilder<'a, Message>) -> Self {
        builder.into_element()
    }
}
