pub mod cache_management;
pub mod connections;
pub mod data_feeds;
pub mod download;
pub mod drawing;
pub mod layout;
pub mod pane;
pub mod preferences;
pub mod replay;
pub mod theme;

use iced::widget::{center, container, mouse_area, opaque, stack};
use iced::{Alignment, Color, Element, Length, padding};
pub use layout::LayoutManager;
pub use pane::stream::{self, ModifierKind};
pub use theme::ThemeEditor;

/// Centered modal with dark backdrop overlay.
/// Used for full-screen dialogs (e.g. data feeds, historical download).
pub fn main_dialog_modal<'a, Message>(
    base: impl Into<Element<'a, Message>>,
    content: impl Into<Element<'a, Message>>,
    on_blur: Message,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    stack![
        base.into(),
        opaque(
            mouse_area(center(opaque(content)).style(|_theme| {
                container::Style {
                    background: Some(
                        Color {
                            a: crate::style::tokens::alpha::BACKDROP,
                            ..Color::BLACK
                        }
                        .into(),
                    ),
                    ..container::Style::default()
                }
            }))
            .on_press(on_blur)
        )
    ]
    .into()
}

/// Positioned overlay without backdrop.
/// Used for sidebar menus, pane modals, and dashboard popovers.
/// Replaces the previous `dashboard_modal()` and `pane::stack_modal()`.
pub fn positioned_overlay<'a, Message>(
    base: impl Into<Element<'a, Message>>,
    content: impl Into<Element<'a, Message>>,
    on_blur: Message,
    padding: padding::Padding,
    align_y: Alignment,
    align_x: Alignment,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    stack![
        base.into(),
        mouse_area(
            container(opaque(content))
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(padding)
                .align_y(align_y)
                .align_x(align_x)
        )
        .on_press(on_blur)
    ]
    .into()
}
