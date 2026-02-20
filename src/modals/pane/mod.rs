use iced::{Alignment, Element, padding};

pub mod calendar;
pub mod indicators;
pub mod settings;
pub mod stream;
pub mod tickers;

#[derive(Debug, Clone, PartialEq)]
pub enum Modal {
    StreamModifier(super::stream::Modifier),
    MiniTickersList(tickers::MiniPanel),
    DataManagement(super::download::data_management::DataManagementPanel),
    DrawingProperties(super::drawing_properties::DrawingPropertiesModal),
    Settings,
    Indicators,
    LinkGroup,
    Controls,
}

/// Positioned overlay for pane-level modals.
/// Delegates to the unified `positioned_overlay` in `modals::mod`.
pub fn stack_modal<'a, Message>(
    base: impl Into<Element<'a, Message>>,
    content: impl Into<Element<'a, Message>>,
    on_blur: Message,
    padding: padding::Padding,
    alignment: Alignment,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    super::positioned_overlay(base, content, on_blur, padding, Alignment::Start, alignment)
}
