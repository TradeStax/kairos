use iced::widget::{button, column, row, text};
use iced::{Alignment, Element};

use crate::style;
use crate::style::tokens;

/// Create an expand/collapse section.
///
/// Renders a clickable header row with a rotation arrow indicator followed by
/// the `body` element when `is_expanded` is true.
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
    let arrow = if is_expanded { "\u{25BC}" } else { "\u{25B6}" };

    let header = button(
        row![
            text(arrow).size(tokens::text::SMALL),
            text(header_text.into()).size(tokens::text::LABEL),
        ]
        .spacing(tokens::spacing::SM)
        .align_y(Alignment::Center),
    )
    .padding(tokens::spacing::XS)
    .on_press(on_toggle)
    .style(|theme, status| style::button::transparent(theme, status, false));

    if is_expanded {
        column![header, body].spacing(tokens::spacing::XS).into()
    } else {
        column![header].into()
    }
}
