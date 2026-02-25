use crate::components;
use crate::components::overlay::form_modal::FormModalBuilder;
use crate::modals::{main_dialog_modal, positioned_overlay, preferences};
use crate::screen::dashboard;
use crate::style::tokens;

use iced::{Alignment, Element, padding};

use super::super::{DownloadMessage, Kairos, Message};

impl Kairos {
    /// Vertical offset for top-positioned sidebar modals to account for
    /// the custom title bar on Windows/Linux.
    #[cfg(target_os = "macos")]
    const HEADER_OFFSET: f32 = 0.0;
    #[cfg(not(target_os = "macos"))]
    const HEADER_OFFSET: f32 = tokens::layout::TITLE_BAR_HEIGHT
        + tokens::layout::MENU_BAR_HEIGHT;

    pub(crate) fn view_with_modal<'a>(
        &'a self,
        base: Element<'a, Message>,
        _dashboard: &'a dashboard::Dashboard,
        menu: data::sidebar::Menu,
    ) -> Element<'a, Message> {
        let sidebar_pos = self.ui.sidebar.position();

        match menu {
            data::sidebar::Menu::Settings => {
                if let Some((page, body)) =
                    self.ui.sidebar.settings.view_modal_body()
                {
                    let title = page.label();
                    let body = body.map(|m| {
                        Message::Sidebar(
                            dashboard::sidebar::Message::Settings(m),
                        )
                    });
                    let on_save = Message::Sidebar(
                        dashboard::sidebar::Message::Settings(
                            preferences::Message::SaveModal,
                        ),
                    );
                    let on_cancel = Message::Sidebar(
                        dashboard::sidebar::Message::Settings(
                            preferences::Message::CloseModal,
                        ),
                    );

                    FormModalBuilder::new(
                        title, body, on_save, on_cancel,
                    )
                    .max_width(480.0)
                    .view(base)
                } else {
                    base
                }
            }
            data::sidebar::Menu::Connections => {
                let (align_x, padding) = match sidebar_pos {
                    data::sidebar::Position::Left => {
                        (Alignment::Start, padding::left(44).bottom(46))
                    }
                    data::sidebar::Position::Right => {
                        (Alignment::End, padding::right(44).bottom(46))
                    }
                };

                positioned_overlay(
                    base,
                    self.modals.connections_menu
                        .view()
                        .map(Message::ConnectionsMenu),
                    Message::Sidebar(
                        dashboard::sidebar::Message::ToggleSidebarMenu(
                            None,
                        ),
                    ),
                    padding,
                    Alignment::End,
                    align_x,
                )
            }
            data::sidebar::Menu::DataFeeds => {
                let data_feeds_content =
                    self.modals.data_feeds_modal.view().map(Message::DataFeeds);

                let mut base_content = main_dialog_modal(
                    base,
                    data_feeds_content,
                    Message::Sidebar(
                        dashboard::sidebar::Message::ToggleSidebarMenu(
                            None,
                        ),
                    ),
                );

                // Stack historical download modal on top if open
                if let Some(dl_modal) = &self.modals.historical_download_modal {
                    let dl_content = dl_modal.view().map(|msg| {
                        Message::Download(
                            DownloadMessage::HistoricalDownload(msg),
                        )
                    });
                    base_content = main_dialog_modal(
                        base_content,
                        dl_content,
                        Message::Download(
                            DownloadMessage::HistoricalDownload(
                                crate::modals::download::HistoricalDownloadMessage::Close,
                            ),
                        ),
                    );
                }

                // Stack API key setup modal on top if open
                if let Some(key_modal) = &self.modals.api_key_setup_modal {
                    let key_content = key_modal.view().map(|msg| {
                        Message::Download(
                            DownloadMessage::ApiKeySetup(msg),
                        )
                    });
                    base_content = main_dialog_modal(
                        base_content,
                        key_content,
                        Message::Download(
                            DownloadMessage::ApiKeySetup(
                                crate::modals::download::ApiKeySetupMessage::Close,
                            ),
                        ),
                    );
                }

                if let Some(dialog) = &self.ui.confirm_dialog {
                    let on_cancel =
                        Message::ToggleDialogModal(None);
                    let mut builder =
                        components::overlay::confirm_dialog::ConfirmDialogBuilder::new(
                            dialog.message.clone(),
                            *dialog.on_confirm.clone(),
                            on_cancel,
                        );
                    if let Some(text) =
                        &dialog.on_confirm_btn_text
                    {
                        builder =
                            builder.confirm_text(text.clone());
                    }
                    builder.view(base_content)
                } else {
                    base_content
                }
            }
            data::sidebar::Menu::Replay => {
                let (align_x, padding) = match sidebar_pos {
                    data::sidebar::Position::Left => (
                        Alignment::Start,
                        padding::left(44)
                            .top(46.0 + Self::HEADER_OFFSET),
                    ),
                    data::sidebar::Position::Right => (
                        Alignment::End,
                        padding::right(44)
                            .top(46.0 + Self::HEADER_OFFSET),
                    ),
                };

                positioned_overlay(
                    base,
                    self.modals.replay_manager
                        .view_setup_modal(self.ui.timezone)
                        .map(Message::Replay),
                    Message::Sidebar(
                        dashboard::sidebar::Message::ToggleSidebarMenu(
                            None,
                        ),
                    ),
                    padding,
                    Alignment::Start,
                    align_x,
                )
            }
            data::sidebar::Menu::ThemeEditor => {
                let (align_x, padding) = match sidebar_pos {
                    data::sidebar::Position::Left => (
                        Alignment::Start,
                        padding::left(44).bottom(4),
                    ),
                    data::sidebar::Position::Right => (
                        Alignment::End,
                        padding::right(44).bottom(4),
                    ),
                };

                positioned_overlay(
                    base,
                    self.modals.theme_editor
                        .view(
                            &crate::style::theme::theme_to_iced(
                                &self.ui.theme,
                            ),
                        )
                        .map(Message::ThemeEditor),
                    Message::Sidebar(
                        dashboard::sidebar::Message::ToggleSidebarMenu(
                            None,
                        ),
                    ),
                    padding,
                    Alignment::End,
                    align_x,
                )
            }
        }
    }
}
