use iced::widget::{center, column, container, mouse_area, opaque, scrollable, stack};
use iced::{Color, Element, Length, Padding};

use crate::components::layout::modal_header::ModalHeaderBuilder;
use crate::style;
use crate::style::tokens;

/// Which container style the modal body receives.
#[derive(Debug, Clone, Copy, Default)]
pub enum ModalKind {
    /// Chart-context modal -- `style::chart_modal`.
    Chart,
    /// Dashboard-level modal -- `style::dashboard_modal`.
    #[default]
    Dashboard,
    /// Confirmation dialog -- `style::confirm_modal`.
    Confirm,
}

/// A composable modal shell that layers over a `base` element.
///
/// The shell draws:
/// 1. The `base` element underneath.
/// 2. A semi-transparent backdrop that closes the modal on click.
/// 3. A centred, styled container holding the title bar, scrollable body
///    and optional footer.
pub struct ModalShell<'a, Message> {
    body: Element<'a, Message>,
    on_close: Message,
    title: Option<String>,
    header_controls: Vec<Element<'a, Message>>,
    footer: Option<Element<'a, Message>>,
    kind: ModalKind,
    max_width: Option<f32>,
    max_height: Option<f32>,
    padding: Padding,
}

impl<'a, Message: Clone + 'a> ModalShell<'a, Message> {
    /// Start building a modal with the given body content and close message.
    pub fn new(body: impl Into<Element<'a, Message>>, on_close: Message) -> Self {
        Self {
            body: body.into(),
            on_close,
            title: None,
            header_controls: Vec::new(),
            footer: None,
            kind: ModalKind::Dashboard,
            max_width: None,
            max_height: None,
            padding: Padding::new(tokens::spacing::XL),
        }
    }

    /// Set the modal title shown above the body.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Add an extra control to the header bar (between title and close).
    pub fn header_control(
        mut self,
        control: impl Into<Element<'a, Message>>,
    ) -> Self {
        self.header_controls.push(control.into());
        self
    }

    /// Provide a footer element rendered below the body (e.g. action
    /// buttons).
    pub fn footer(mut self, footer: impl Into<Element<'a, Message>>) -> Self {
        self.footer = Some(footer.into());
        self
    }

    /// Choose the container style variant.
    pub fn kind(mut self, kind: ModalKind) -> Self {
        self.kind = kind;
        self
    }

    /// Constrain the modal panel's maximum width.
    pub fn max_width(mut self, max_width: f32) -> Self {
        self.max_width = Some(max_width);
        self
    }

    /// Constrain the modal panel's maximum height.
    pub fn max_height(mut self, max_height: f32) -> Self {
        self.max_height = Some(max_height);
        self
    }

    /// Override internal padding.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Render the modal on top of `base`.
    pub fn view(self, base: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
        let style_fn: fn(&iced::Theme) -> container::Style = match self.kind {
            ModalKind::Chart => style::chart_modal,
            ModalKind::Dashboard => style::dashboard_modal,
            ModalKind::Confirm => style::confirm_modal,
        };

        let has_title = self.title.is_some();

        // -- Build the outer column -------------------------------------------
        let mut outer = column![].width(Length::Fill);

        // Styled header bar when title is set
        if let Some(title) = self.title {
            let mut header = ModalHeaderBuilder::new(title)
                .on_close(self.on_close.clone());
            for control in self.header_controls {
                header = header.push_control(control);
            }
            outer = outer.push(header);
        }

        // Body padding — remove top when header is present
        let body_padding = if has_title {
            Padding {
                top: 0.0,
                right: self.padding.right,
                bottom: self.padding.bottom,
                left: self.padding.left,
            }
        } else {
            self.padding
        };

        // Scrollable body + optional footer inside padded container
        let mut inner =
            column![].spacing(tokens::spacing::LG).width(Length::Fill);

        let body_scrollable =
            scrollable::Scrollable::with_direction(
                self.body,
                scrollable::Direction::Vertical(
                    scrollable::Scrollbar::new()
                        .width(4)
                        .scroller_width(4)
                        .spacing(2),
                ),
            )
            .style(style::scroll_bar);
        inner = inner.push(body_scrollable);

        if let Some(footer) = self.footer {
            inner = inner.push(footer);
        }

        outer = outer.push(
            container(inner).padding(body_padding).width(Length::Fill),
        );

        // -- Container --------------------------------------------------------
        let mut modal_container =
            container(outer).style(style_fn);

        if let Some(mw) = self.max_width {
            modal_container = modal_container.max_width(mw);
        }
        if let Some(mh) = self.max_height {
            modal_container = modal_container.max_height(mh);
        }

        // -- Stack: base + backdrop + modal -----------------------------------
        let on_close = self.on_close;

        stack![
            base.into(),
            opaque(
                mouse_area(center(opaque(modal_container)).style(|_theme| {
                    container::Style {
                        background: Some(
                            Color {
                                a: tokens::alpha::BACKDROP,
                                ..Color::BLACK
                            }
                            .into(),
                        ),
                        ..container::Style::default()
                    }
                }))
                .on_press(on_close)
            )
        ]
        .into()
    }
}

// ── Convenience constructors ──────────────────────────────────────────

/// Shorthand for a chart-context modal.
pub fn chart_modal<'a, Message: Clone + 'a>(
    body: impl Into<Element<'a, Message>>,
    on_close: Message,
) -> ModalShell<'a, Message> {
    ModalShell::new(body, on_close).kind(ModalKind::Chart)
}

/// Shorthand for a dashboard-level modal.
pub fn dashboard_modal<'a, Message: Clone + 'a>(
    body: impl Into<Element<'a, Message>>,
    on_close: Message,
) -> ModalShell<'a, Message> {
    ModalShell::new(body, on_close).kind(ModalKind::Dashboard)
}
