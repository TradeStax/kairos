use iced::widget::container;
use iced::{Element, Length, Padding};

use crate::style;
use crate::style::tokens;

/// Visual style variant for a card.
#[derive(Debug, Clone, Copy, Default)]
pub enum CardKind {
    /// Flat card using `modal_container` style.
    #[default]
    Default,
    /// Elevated card using `dashboard_modal` style (deeper shadow).
    Elevated,
    /// Interactive card using `ticker_card` container style.
    Interactive,
}

/// Builder for a themed container card.
///
/// Wraps arbitrary content in a styled `container` chosen by [`CardKind`].
pub struct CardBuilder<'a, Message> {
    content: Element<'a, Message>,
    kind: CardKind,
    padding: Padding,
    max_width: Option<f32>,
    width: Option<Length>,
}

impl<'a, Message: 'a> CardBuilder<'a, Message> {
    /// Start building a card that wraps `content`.
    pub fn new(content: impl Into<Element<'a, Message>>) -> Self {
        Self {
            content: content.into(),
            kind: CardKind::Default,
            padding: Padding::new(tokens::spacing::LG),
            max_width: None,
            width: None,
        }
    }

    /// Set the visual kind.
    pub fn kind(mut self, kind: CardKind) -> Self {
        self.kind = kind;
        self
    }

    /// Override padding (default 12).
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Constrain the maximum width.
    pub fn max_width(mut self, max_width: f32) -> Self {
        self.max_width = Some(max_width);
        self
    }

    /// Set the width of the card.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = Some(width.into());
        self
    }

    /// Consume the builder and produce an [`Element`].
    pub fn into_element(self) -> Element<'a, Message> {
        let style_fn: fn(&iced::Theme) -> container::Style = match self.kind {
            CardKind::Default => style::modal_container,
            CardKind::Elevated => style::dashboard_modal,
            CardKind::Interactive => style::ticker_card,
        };

        let mut c = container(self.content)
            .padding(self.padding)
            .style(style_fn);

        if let Some(mw) = self.max_width {
            c = c.max_width(mw);
        }
        if let Some(w) = self.width {
            c = c.width(w);
        }

        c.into()
    }
}

impl<'a, Message: 'a> From<CardBuilder<'a, Message>> for Element<'a, Message> {
    fn from(builder: CardBuilder<'a, Message>) -> Self {
        builder.into_element()
    }
}
