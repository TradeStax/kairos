use iced::widget::{button, column, row, text};
use iced::{Alignment, Element};
use iced_anim::AnimationBuilder;

use crate::style;
use crate::style::{animation, tokens};

/// Create an expand/collapse section with animated arrow indicator.
///
/// Renders a clickable header row with a rotation arrow indicator followed by
/// the `body` element when `is_expanded` is true. The arrow smoothly
/// transitions between collapsed (▶) and expanded (▼) states.
///
/// ```text
///  v  Section Title      <-- clicking toggles
///     [ body content ]   <-- only when expanded
/// ```
pub fn collapsible<'a, Message: Clone + 'a>(
    header_text: impl Into<String>,
    is_expanded: bool,
    on_toggle: Message,
    body: Element<'a, Message>,
) -> Element<'a, Message> {
    let target: f32 = if is_expanded { 1.0 } else { 0.0 };
    let header_str: String = header_text.into();
    let on_toggle_clone = on_toggle.clone();

    let animated_header: Element<'a, Message> =
        AnimationBuilder::new(target, move |progress| {
            let arrow = if progress > 0.5 { "\u{25BC}" } else { "\u{25B6}" };

            button(
                row![
                    text(arrow).size(tokens::text::SMALL),
                    text(header_str.clone()).size(tokens::text::LABEL),
                ]
                .spacing(tokens::spacing::SM)
                .align_y(Alignment::Center),
            )
            .padding(tokens::spacing::XS)
            .on_press(on_toggle_clone.clone())
            .style(|theme, status| style::button::transparent(theme, status, false))
            .into()
        })
        .animation(animation::spring::EXPAND)
        .into();

    if is_expanded {
        column![animated_header, body]
            .spacing(tokens::spacing::XS)
            .into()
    } else {
        column![animated_header].into()
    }
}
