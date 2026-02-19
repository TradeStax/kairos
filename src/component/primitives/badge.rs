//! Small pill-shaped badge for labels and counts.

use iced::widget::{container, text};
use iced::{Border, Element, Renderer, Theme};

use crate::style::tokens;

/// Visual style of a badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeKind {
    Default,
    Success,
    Warning,
    Danger,
    Info,
}

/// Create a small pill-shaped badge with the given text and kind.
pub fn badge<'a, Message: 'a>(
    label: &'a str,
    kind: BadgeKind,
) -> Element<'a, Message, Theme, Renderer> {
    container(text(label).size(tokens::text::TINY))
        .padding([tokens::spacing::XXS, tokens::spacing::SM])
        .style(move |theme: &Theme| {
            let palette = theme.extended_palette();

            let (bg, fg) = match kind {
                BadgeKind::Default => (
                    palette.background.strong.color,
                    palette.background.base.text,
                ),
                BadgeKind::Success => (palette.success.weak.color, palette.success.weak.text),
                BadgeKind::Warning => (palette.warning.weak.color, palette.warning.weak.text),
                BadgeKind::Danger => (palette.danger.weak.color, palette.danger.weak.text),
                BadgeKind::Info => (palette.primary.weak.color, palette.primary.weak.text),
            };

            container::Style {
                text_color: Some(fg),
                background: Some(bg.into()),
                border: Border {
                    radius: tokens::radius::ROUND.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}
