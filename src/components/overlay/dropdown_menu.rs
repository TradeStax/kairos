use iced::widget::{column, container, mouse_area, opaque, scrollable, stack};
use iced::{Element, Length, Padding};

use crate::style;
use crate::style::tokens;

/// Builder for a positioned dropdown overlay.
///
/// The dropdown is rendered on top of `base` as an opaque overlay. Clicking
/// outside the dropdown closes it via `on_close`.
pub struct DropdownMenuBuilder<'a, Message> {
    items: Element<'a, Message>,
    on_close: Message,
    offset_x: f32,
    offset_y: f32,
    max_height: Option<f32>,
    width: Option<Length>,
    padding: Padding,
}

impl<'a, Message: Clone + 'a> DropdownMenuBuilder<'a, Message> {
    /// Create a dropdown containing `items` (typically a column of buttons).
    pub fn new(items: impl Into<Element<'a, Message>>, on_close: Message) -> Self {
        Self {
            items: items.into(),
            on_close,
            offset_x: 0.0,
            offset_y: 0.0,
            max_height: None,
            width: None,
            padding: Padding::new(tokens::spacing::XS),
        }
    }

    /// Shift the dropdown position relative to the base.
    pub fn offset(mut self, x: f32, y: f32) -> Self {
        self.offset_x = x;
        self.offset_y = y;
        self
    }

    /// Constrain the dropdown height (will scroll if exceeded).
    pub fn max_height(mut self, max_height: f32) -> Self {
        self.max_height = Some(max_height);
        self
    }

    /// Override the width of the dropdown panel.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = Some(width.into());
        self
    }

    /// Override internal padding.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Render the dropdown over `base`.
    pub fn view(self, base: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
        let body = column![self.items].spacing(tokens::spacing::XXS);

        let scrolled: Element<'a, Message> = if let Some(_mh) = self.max_height {
            scrollable(body)
                .height(Length::Shrink)
                .style(style::scroll_bar)
                .into()
        } else {
            body.into()
        };

        let mut dropdown = container(scrolled)
            .padding(self.padding)
            .style(style::dropdown_container);

        if let Some(w) = self.width {
            dropdown = dropdown.width(w);
        }
        if let Some(mh) = self.max_height {
            dropdown = dropdown.max_height(mh);
        }

        let on_close = self.on_close;

        // Position the dropdown using padding offset
        let positioned = container(opaque(dropdown))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding {
                top: self.offset_y,
                right: 0.0,
                bottom: 0.0,
                left: self.offset_x,
            });

        stack![base.into(), mouse_area(positioned).on_press(on_close),].into()
    }
}
