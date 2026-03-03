use crate::components;
use crate::components::overlay::confirm_dialog::ConfirmDialogBuilder;
use crate::components::overlay::form_modal::FormModalBuilder;
use crate::components::overlay::modal_shell::ModalShell;
use crate::modals::cache_management::CacheManagementMessage;
use crate::modals::{main_dialog_modal, positioned_overlay, preferences};
use crate::screen::dashboard;

use iced::{Alignment, Element, padding};

use super::super::{DownloadMessage, Kairos, Message};

impl Kairos {
    pub(crate) fn view_with_modal<'a>(
        &'a self,
        base: Element<'a, Message>,
        _dashboard: &'a dashboard::Dashboard,
        menu: crate::config::sidebar::Menu,
    ) -> Element<'a, Message> {
        let sidebar_pos = self.ui.sidebar.position();

        match menu {
            crate::config::sidebar::Menu::Settings => {
                if let Some((page, body)) = self.ui.sidebar.settings.view_modal_body() {
                    let title = page.label();
                    let body =
                        body.map(|m| Message::Sidebar(dashboard::sidebar::Message::Settings(m)));
                    let on_save = Message::Sidebar(dashboard::sidebar::Message::Settings(
                        preferences::Message::SaveModal,
                    ));
                    let on_cancel = Message::Sidebar(dashboard::sidebar::Message::Settings(
                        preferences::Message::CloseModal,
                    ));

                    FormModalBuilder::new(title, body, on_save, on_cancel)
                        .max_width(480.0)
                        .view(base)
                } else {
                    base
                }
            }
            crate::config::sidebar::Menu::Connections => {
                let (align_x, padding) = match sidebar_pos {
                    crate::config::sidebar::Position::Left => {
                        (Alignment::Start, padding::left(44).bottom(46))
                    }
                    crate::config::sidebar::Position::Right => {
                        (Alignment::End, padding::right(44).bottom(46))
                    }
                };

                positioned_overlay(
                    base,
                    self.modals
                        .connections_menu
                        .view()
                        .map(Message::ConnectionsMenu),
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                    padding,
                    Alignment::End,
                    align_x,
                )
            }
            crate::config::sidebar::Menu::DataFeeds => {
                let data_feeds_content =
                    self.modals.data_feeds_modal.view().map(Message::DataFeeds);

                let mut base_content = main_dialog_modal(
                    base,
                    data_feeds_content,
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                );

                // Stack historical download modal on top if open
                if let Some(dl_modal) = &self.modals.historical_download_modal {
                    let dl_content = dl_modal
                        .view()
                        .map(|msg| Message::Download(DownloadMessage::HistoricalDownload(msg)));
                    base_content = main_dialog_modal(
                        base_content,
                        dl_content,
                        Message::Download(DownloadMessage::HistoricalDownload(
                            crate::modals::download::HistoricalDownloadMessage::Close,
                        )),
                    );
                }

                // Stack API key setup modal on top if open
                if let Some(key_modal) = &self.modals.api_key_setup_modal {
                    let key_content = key_modal
                        .view()
                        .map(|msg| Message::Download(DownloadMessage::ApiKeySetup(msg)));
                    base_content = main_dialog_modal(
                        base_content,
                        key_content,
                        Message::Download(DownloadMessage::ApiKeySetup(
                            crate::modals::download::ApiKeySetupMessage::Close,
                        )),
                    );
                }

                if let Some(dialog) = &self.ui.confirm_dialog {
                    let on_cancel = Message::ToggleDialogModal(None);
                    let mut builder =
                        components::overlay::confirm_dialog::ConfirmDialogBuilder::new(
                            dialog.message.clone(),
                            *dialog.on_confirm.clone(),
                            on_cancel,
                        );
                    if let Some(text) = &dialog.on_confirm_btn_text {
                        builder = builder.confirm_text(text.clone());
                    }
                    builder.view(base_content)
                } else {
                    base_content
                }
            }
            crate::config::sidebar::Menu::Replay => {
                let (align_x, padding) = match sidebar_pos {
                    crate::config::sidebar::Position::Left => {
                        (Alignment::Start, padding::left(44).bottom(82))
                    }
                    crate::config::sidebar::Position::Right => {
                        (Alignment::End, padding::right(44).bottom(82))
                    }
                };

                positioned_overlay(
                    base,
                    self.modals
                        .replay_manager
                        .view_setup_modal(self.ui.timezone)
                        .map(Message::Replay),
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                    padding,
                    Alignment::End,
                    align_x,
                )
            }
            crate::config::sidebar::Menu::ThemeEditor => {
                let (align_x, padding) = match sidebar_pos {
                    crate::config::sidebar::Position::Left => {
                        (Alignment::Start, padding::left(44).bottom(4))
                    }
                    crate::config::sidebar::Position::Right => {
                        (Alignment::End, padding::right(44).bottom(4))
                    }
                };

                positioned_overlay(
                    base,
                    self.modals
                        .theme_editor
                        .view(&crate::style::theme::theme_to_iced(&self.ui.theme))
                        .map(Message::ThemeEditor),
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                    padding,
                    Alignment::End,
                    align_x,
                )
            }
            crate::config::sidebar::Menu::CacheManagement => {
                let on_close = Message::CacheManagement(CacheManagementMessage::Close);

                let body = self
                    .modals
                    .cache_management
                    .view()
                    .map(Message::CacheManagement);

                let mut base_content = ModalShell::new(body, on_close.clone())
                    .title("Data Management")
                    .max_width(700.0)
                    .max_height(600.0)
                    .view(base);

                // Stack confirm dialog on top if pending
                if let Some(target) = &self.modals.cache_management.confirm_delete {
                    let count = match target {
                        crate::modals::cache_management::DeleteTarget::Selected => {
                            self.modals.cache_management.selected_count()
                        }
                        _ => self.modals.cache_management.entries.len(),
                    };
                    let msg = target.confirm_message(count);

                    base_content = ConfirmDialogBuilder::new(
                        msg,
                        Message::CacheManagement(CacheManagementMessage::ConfirmDelete),
                        Message::CacheManagement(CacheManagementMessage::CancelDelete),
                    )
                    .confirm_text("Delete")
                    .destructive(true)
                    .view(base_content);
                }

                base_content
            }
        }
    }
}
