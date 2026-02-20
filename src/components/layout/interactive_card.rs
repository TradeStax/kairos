use iced::widget::{button, column, container, row};
use iced::{Border, Element, Length, Padding, Theme};

use crate::style;
use crate::style::tokens;

/// A clickable card with hover highlight and optional selection state.
///
/// Renders as a `button` styled to look like a card.  When `selected` is true
/// a subtle accent bar is drawn on the left edge.
pub struct InteractiveCardBuilder<'a, Message> {
    content: Element<'a, Message>,
    on_press: Message,
    selected: bool,
    height: Option<Length>,
    padding: Padding,
    accent_bar: bool,
    width: Option<Length>,
}

impl<'a, Message: Clone + 'a> InteractiveCardBuilder<'a, Message> {
    pub fn new(content: impl Into<Element<'a, Message>>, on_press: Message) -> Self {
        Self {
            content: content.into(),
            on_press,
            selected: false,
            height: None,
            padding: Padding::new(tokens::spacing::MD),
            accent_bar: false,
            width: None,
        }
    }

    /// Mark this card as currently selected.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Override the card height.
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = Some(height.into());
        self
    }

    /// Override internal padding.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// When true, a coloured accent bar is drawn on the leading edge
    /// of the card while it is selected.
    pub fn accent_bar(mut self, show: bool) -> Self {
        self.accent_bar = show;
        self
    }

    /// Override the card width.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = Some(width.into());
        self
    }

    /// Consume the builder and return an [`Element`].
    pub fn into_element(self) -> Element<'a, Message> {
        let selected = self.selected;
        let show_accent = self.accent_bar && self.selected;

        let inner: Element<'a, Message> =
            if show_accent {
                let bar = container(column![]).width(3).height(Length::Fill).style(
                    move |theme: &Theme| {
                        let palette = theme.extended_palette();
                        iced::widget::container::Style {
                            background: Some(palette.primary.base.color.into()),
                            border: Border {
                                radius: 2.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    },
                );
                row![bar, self.content]
                    .spacing(tokens::spacing::SM)
                    .align_y(iced::Alignment::Center)
                    .into()
            } else {
                self.content
            };

        let mut btn = button(inner)
            .padding(self.padding)
            .on_press(self.on_press)
            .style(move |theme: &Theme, status| style::button::menu_body(theme, status, selected));

        if let Some(h) = self.height {
            btn = btn.height(h);
        }
        if let Some(w) = self.width {
            btn = btn.width(w);
        }

        btn.into()
    }
}

impl<'a, Message: Clone + 'a> From<InteractiveCardBuilder<'a, Message>> for Element<'a, Message> {
    fn from(builder: InteractiveCardBuilder<'a, Message>) -> Self {
        builder.into_element()
    }
}
