use iced::widget::{container, row, rule, space};
use iced::{Alignment, Element};

use crate::style::tokens;

/// An item inside a toolbar.
pub enum ToolbarItem<'a, Message> {
    /// A regular widget (usually a button).
    Button(Element<'a, Message>),
    /// A vertical separator line.
    Separator,
    /// Flexible space that pushes subsequent items to the right.
    FlexSpace,
}

/// Build a horizontal toolbar from a list of [`ToolbarItem`]s.
///
/// Buttons are laid out in a row with small spacing.  `Separator` items
/// draw a thin vertical rule and `FlexSpace` inserts expanding horizontal
/// space.
pub fn toolbar<'a, Message: 'a>(items: Vec<ToolbarItem<'a, Message>>) -> Element<'a, Message> {
    let mut r = row![]
        .spacing(tokens::spacing::XS)
        .align_y(Alignment::Center);

    for item in items {
        match item {
            ToolbarItem::Button(element) => {
                r = r.push(element);
            }
            ToolbarItem::Separator => {
                r = r.push(
                    container(rule::vertical(1))
                        .height(16)
                        .padding([0, tokens::spacing::XS as u16]),
                );
            }
            ToolbarItem::FlexSpace => {
                r = r.push(space::horizontal());
            }
        }
    }

    r.into()
}
