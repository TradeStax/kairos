use iced::widget::{button, row, space};
use iced::{Alignment, Element, Length, Padding};

use crate::style;
use crate::style::tokens;

/// Builder for a selectable list row.
///
/// The row has an optional leading element (e.g. an icon), a main content
/// area, and an optional trailing element (e.g. a badge or action button).
pub struct ListItemBuilder<'a, Message> {
    content: Element<'a, Message>,
    on_press: Message,
    selected: bool,
    leading: Option<Element<'a, Message>>,
    trailing: Option<Element<'a, Message>>,
    height: Option<Length>,
    padding: Padding,
}

impl<'a, Message: Clone + 'a> ListItemBuilder<'a, Message> {
    pub fn new(content: impl Into<Element<'a, Message>>, on_press: Message) -> Self {
        Self {
            content: content.into(),
            on_press,
            selected: false,
            leading: None,
            trailing: None,
            height: None,
            padding: Padding::new(tokens::spacing::SM),
        }
    }

    /// Highlight this item as selected.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Add a leading element (icon, avatar, etc.).
    pub fn leading(mut self, element: impl Into<Element<'a, Message>>) -> Self {
        self.leading = Some(element.into());
        self
    }

    /// Add a trailing element (badge, secondary action, etc.).
    pub fn trailing(mut self, element: impl Into<Element<'a, Message>>) -> Self {
        self.trailing = Some(element.into());
        self
    }

    /// Set a fixed height.
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = Some(height.into());
        self
    }

    /// Override padding.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    pub fn into_element(self) -> Element<'a, Message> {
        let selected = self.selected;

        let mut r = row![]
            .spacing(tokens::spacing::MD)
            .align_y(Alignment::Center);

        if let Some(leading) = self.leading {
            r = r.push(leading);
        }

        r = r.push(self.content);
        r = r.push(space::horizontal());

        if let Some(trailing) = self.trailing {
            r = r.push(trailing);
        }

        let mut btn = button(r)
            .padding(self.padding)
            .width(Length::Fill)
            .on_press(self.on_press)
            .style(move |theme, status| style::button::menu_body(theme, status, selected));

        if let Some(h) = self.height {
            btn = btn.height(h);
        }

        btn.into()
    }
}

impl<'a, Message: Clone + 'a> From<ListItemBuilder<'a, Message>> for Element<'a, Message> {
    fn from(builder: ListItemBuilder<'a, Message>) -> Self {
        builder.into_element()
    }
}
