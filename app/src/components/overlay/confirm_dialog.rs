use iced::widget::{button, row, space, text};
use iced::{Alignment, Element};

use crate::style;
use crate::style::tokens;

use super::modal_shell::{ModalKind, ModalShell};

#[derive(Debug, Clone)]
pub struct ConfirmDialog<M> {
    pub message: String,
    pub on_confirm: Box<M>,
    pub on_confirm_btn_text: Option<String>,
}


/// Builder for a confirmation dialog that renders on top of a base element.
///
/// Uses [`ModalShell`] internally with [`ModalKind::Confirm`].
pub struct ConfirmDialogBuilder<'a, Message> {
    message_text: String,
    on_confirm: Message,
    on_cancel: Message,
    confirm_text: String,
    cancel_text: String,
    destructive: bool,
    _lifetime: std::marker::PhantomData<&'a ()>,
}

impl<'a, Message: Clone + 'a> ConfirmDialogBuilder<'a, Message> {
    pub fn new(message_text: impl Into<String>, on_confirm: Message, on_cancel: Message) -> Self {
        Self {
            message_text: message_text.into(),
            on_confirm,
            on_cancel,
            confirm_text: "Confirm".into(),
            cancel_text: "Cancel".into(),
            destructive: false,
            _lifetime: std::marker::PhantomData,
        }
    }

    /// Override the confirm button label (default "Confirm").
    pub fn confirm_text(mut self, label: impl Into<String>) -> Self {
        self.confirm_text = label.into();
        self
    }

    /// Override the cancel button label (default "Cancel").
    pub fn cancel_text(mut self, label: impl Into<String>) -> Self {
        self.cancel_text = label.into();
        self
    }

    /// When true the confirm button uses the danger style.
    pub fn destructive(mut self, destructive: bool) -> Self {
        self.destructive = destructive;
        self
    }

    /// Render the dialog on top of `base`.
    pub fn view(self, base: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
        let destructive = self.destructive;

        let confirm_btn = button(text(self.confirm_text))
            .on_press(self.on_confirm.clone())
            .style(move |theme, status| {
                if destructive {
                    style::button::danger(theme, status)
                } else {
                    style::button::primary(theme, status)
                }
            });

        let cancel_btn = button(text(self.cancel_text))
            .on_press(self.on_cancel.clone())
            .style(|theme, status| style::button::transparent(theme, status, false));

        let footer: Element<'a, Message> = row![space::horizontal(), cancel_btn, confirm_btn,]
            .spacing(tokens::spacing::MD)
            .align_y(Alignment::Center)
            .into();

        let body: Element<'a, Message> = text(self.message_text).size(tokens::text::BODY).into();

        ModalShell::new(body, self.on_cancel)
            .kind(ModalKind::Confirm)
            .footer(footer)
            .max_width(tokens::layout::CONFIRM_DIALOG_WIDTH)
            .view(base)
    }
}
