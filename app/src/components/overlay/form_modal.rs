use iced::widget::{button, row, space, text};
use iced::{Alignment, Element};

use crate::style;
use crate::style::tokens;

use super::modal_shell::{ModalKind, ModalShell};

/// Builder for a modal dialog that contains a form body with Save / Cancel
/// footer buttons.
pub struct FormModalBuilder<'a, Message> {
    title: String,
    body: Element<'a, Message>,
    on_save: Message,
    on_cancel: Message,
    save_text: String,
    cancel_text: String,
    max_width: f32,
    _lifetime: std::marker::PhantomData<&'a ()>,
}

impl<'a, Message: Clone + 'a> FormModalBuilder<'a, Message> {
    pub fn new(
        title: impl Into<String>,
        body: impl Into<Element<'a, Message>>,
        on_save: Message,
        on_cancel: Message,
    ) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            on_save,
            on_cancel,
            save_text: "Save".into(),
            cancel_text: "Cancel".into(),
            max_width: tokens::layout::MODAL_MAX_WIDTH as f32,
            _lifetime: std::marker::PhantomData,
        }
    }

    /// Override the maximum width of the modal panel.
    pub fn max_width(mut self, max_width: f32) -> Self {
        self.max_width = max_width;
        self
    }

    /// Render the form modal on top of `base`.
    pub fn view(self, base: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
        let save_btn = button(text(self.save_text))
            .on_press(self.on_save)
            .style(style::button::primary);

        let cancel_btn = button(text(self.cancel_text))
            .on_press(self.on_cancel.clone())
            .style(|theme, status| style::button::transparent(theme, status, false));

        let footer: Element<'a, Message> = row![space::horizontal(), cancel_btn, save_btn,]
            .spacing(tokens::spacing::MD)
            .align_y(Alignment::Center)
            .into();

        ModalShell::new(self.body, self.on_cancel)
            .title(self.title)
            .kind(ModalKind::Dashboard)
            .footer(footer)
            .max_width(self.max_width)
            .view(base)
    }
}
