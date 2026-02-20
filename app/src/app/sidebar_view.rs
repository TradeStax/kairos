use crate::components;
use crate::components::display::tooltip::tooltip;
use crate::modals::{main_dialog_modal, positioned_overlay};
use crate::screen::dashboard::{self, Dashboard};
use crate::style::tokens;
use crate::{split_column, style};
use data::sidebar;

use iced::{
    Alignment, Element, padding,
    widget::{
        button, column, container, pane_grid, pick_list, row, rule, scrollable, text,
        tooltip::Position as TooltipPosition,
    },
};

use super::{DownloadMessage, Kairos, Message};

impl Kairos {
    /// Vertical offset for top-positioned sidebar modals to account for
    /// the custom title bar on Windows/Linux.
    #[cfg(target_os = "macos")]
    const HEADER_OFFSET: f32 = 0.0;
    #[cfg(not(target_os = "macos"))]
    const HEADER_OFFSET: f32 = tokens::layout::TITLE_BAR_HEIGHT + tokens::layout::MENU_BAR_HEIGHT;

    pub(crate) fn view_with_modal<'a>(
        &'a self,
        base: Element<'a, Message>,
        dashboard: &'a Dashboard,
        menu: sidebar::Menu,
    ) -> Element<'a, Message> {
        let sidebar_pos = self.sidebar.position();

        match menu {
            sidebar::Menu::Settings => {
                let settings_modal = {
                    let theme_picklist = {
                        let mut themes: Vec<iced::Theme> = iced_core::Theme::ALL.to_vec();
                        themes.push(crate::style::theme_bridge::default_iced_theme());
                        if let Some(custom_theme) = &self.theme_editor.custom_theme {
                            themes.push(custom_theme.clone());
                        }
                        let current_iced = crate::style::theme_bridge::theme_to_iced(&self.theme);
                        pick_list(themes, Some(current_iced), |theme| {
                            Message::ThemeSelected(crate::style::theme_bridge::iced_theme_to_data(theme))
                        })
                    };

                    let toggle_theme_editor = button(text("Theme editor")).on_press(
                        Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(Some(
                            sidebar::Menu::ThemeEditor,
                        ))),
                    );

                    let timezone_picklist = pick_list(
                        [data::UserTimezone::Utc, data::UserTimezone::Local],
                        Some(self.timezone),
                        Message::SetTimezone,
                    );

                    let date_range_picker = pick_list(
                        sidebar::DateRangePreset::ALL,
                        Some(self.sidebar.date_range_preset()),
                        |preset| {
                            Message::Sidebar(dashboard::sidebar::Message::SetDateRangePreset(
                                preset,
                            ))
                        },
                    );

                    let scale_factor = {
                        let current_value: f32 = self.ui_scale_factor.into();

                        let decrease_btn = if current_value > data::config::MIN_SCALE {
                            button(text("-"))
                                .on_press(Message::ScaleFactorChanged((current_value - 0.1).into()))
                        } else {
                            button(text("-"))
                        };

                        let increase_btn = if current_value < data::config::MAX_SCALE {
                            button(text("+"))
                                .on_press(Message::ScaleFactorChanged((current_value + 0.1).into()))
                        } else {
                            button(text("+"))
                        };

                        container(
                            row![
                                decrease_btn,
                                text(format!("{:.0}%", current_value * 100.0))
                                    .size(tokens::text::TITLE),
                                increase_btn,
                            ]
                            .align_y(Alignment::Center)
                            .spacing(tokens::spacing::MD)
                            .padding(tokens::spacing::XS),
                        )
                        .style(style::modal_container)
                    };

                    let open_data_folder = {
                        let button =
                            button(text("Open data folder")).on_press(Message::DataFolderRequested);

                        tooltip(
                            button,
                            Some("Open the folder where the data & config is stored"),
                            TooltipPosition::Top,
                        )
                    };

                    let column_content = split_column![
                        column![open_data_folder,].spacing(tokens::spacing::MD),
                        column![text("Date range").size(tokens::text::TITLE), date_range_picker,].spacing(tokens::spacing::LG),
                        column![text("Time zone").size(tokens::text::TITLE), timezone_picklist,].spacing(tokens::spacing::LG),
                        column![text("Theme").size(tokens::text::TITLE), theme_picklist,].spacing(tokens::spacing::LG),
                        column![text("Interface scale").size(tokens::text::TITLE), scale_factor,].spacing(tokens::spacing::LG),
                        column![
                            text("Experimental").size(tokens::text::TITLE),
                            toggle_theme_editor,
                        ]
                        .spacing(tokens::spacing::LG),
                        ; spacing = tokens::spacing::XL, align_x = Alignment::Start
                    ];

                    let content = scrollable::Scrollable::with_direction(
                        column_content,
                        scrollable::Direction::Vertical(
                            scrollable::Scrollbar::new().width(8).scroller_width(6),
                        ),
                    );

                    container(content)
                        .align_x(Alignment::Start)
                        .max_width(240)
                        .padding(tokens::spacing::XXL)
                        .style(style::dashboard_modal)
                };

                let (align_x, padding) = match sidebar_pos {
                    sidebar::Position::Left => (Alignment::Start, padding::left(44).bottom(4)),
                    sidebar::Position::Right => (Alignment::End, padding::right(44).bottom(4)),
                };

                let base_content = positioned_overlay(
                    base,
                    settings_modal,
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                    padding,
                    Alignment::End,
                    align_x,
                );

                if let Some(dialog) = &self.confirm_dialog {
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
            sidebar::Menu::Layout => {
                let main_window = self.main_window.id;

                let manage_pane = if let Some((window_id, pane_id)) = dashboard.focus {
                    let selected_pane_str =
                        if let Some(state) = dashboard.get_pane(main_window, window_id, pane_id) {
                            let link_group_name: String =
                                state.link_group.as_ref().map_or_else(String::new, |g| {
                                    " - Group ".to_string() + &g.to_string()
                                });

                            state.content.to_string() + &link_group_name
                        } else {
                            "".to_string()
                        };

                    let is_main_window = window_id == main_window;

                    let reset_pane_button = {
                        let btn = button(text("Reset").align_x(Alignment::Center))
                            .width(iced::Length::Fill);
                        if is_main_window {
                            let dashboard_msg = Message::Dashboard {
                                layout_id: None,
                                event: dashboard::Message::Pane(
                                    main_window,
                                    dashboard::pane::Message::ReplacePane(pane_id),
                                ),
                            };

                            btn.on_press(dashboard_msg)
                        } else {
                            btn
                        }
                    };
                    let split_pane_button = {
                        let btn = button(text("Split").align_x(Alignment::Center))
                            .width(iced::Length::Fill);
                        if is_main_window {
                            let dashboard_msg = Message::Dashboard {
                                layout_id: None,
                                event: dashboard::Message::Pane(
                                    main_window,
                                    dashboard::pane::Message::SplitPane(
                                        pane_grid::Axis::Horizontal,
                                        pane_id,
                                    ),
                                ),
                            };
                            btn.on_press(dashboard_msg)
                        } else {
                            btn
                        }
                    };

                    column![
                        text(selected_pane_str),
                        row![
                            tooltip(
                                reset_pane_button,
                                if is_main_window {
                                    Some("Reset selected pane")
                                } else {
                                    None
                                },
                                TooltipPosition::Top,
                            ),
                            tooltip(
                                split_pane_button,
                                if is_main_window {
                                    Some("Split selected pane horizontally")
                                } else {
                                    None
                                },
                                TooltipPosition::Top,
                            ),
                        ]
                        .spacing(tokens::spacing::MD)
                    ]
                    .spacing(tokens::spacing::MD)
                } else {
                    column![text("No pane selected"),].spacing(tokens::spacing::MD)
                };

                let manage_layout_modal = {
                    let col = column![
                        manage_pane,
                        rule::horizontal(1.0).style(style::split_ruler),
                        self.layout_manager.view().map(Message::Layouts)
                    ];

                    container(col.align_x(Alignment::Center).spacing(20))
                        .width(260)
                        .padding(tokens::spacing::XXL)
                        .style(style::dashboard_modal)
                };

                let (align_x, padding) = match sidebar_pos {
                    sidebar::Position::Left => (
                        Alignment::Start,
                        padding::left(44).top(8.0 + Self::HEADER_OFFSET),
                    ),
                    sidebar::Position::Right => (
                        Alignment::End,
                        padding::right(44).top(8.0 + Self::HEADER_OFFSET),
                    ),
                };

                positioned_overlay(
                    base,
                    manage_layout_modal,
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                    padding,
                    Alignment::Start,
                    align_x,
                )
            }
            sidebar::Menu::Connections => {
                let (align_x, padding) = match sidebar_pos {
                    sidebar::Position::Left => (Alignment::Start, padding::left(44).bottom(46)),
                    sidebar::Position::Right => (Alignment::End, padding::right(44).bottom(46)),
                };

                positioned_overlay(
                    base,
                    self.connections_menu.view().map(Message::ConnectionsMenu),
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                    padding,
                    Alignment::End,
                    align_x,
                )
            }
            sidebar::Menu::DataFeeds => {
                let data_feeds_content = self.data_feeds_modal.view().map(Message::DataFeeds);

                let mut base_content = main_dialog_modal(
                    base,
                    data_feeds_content,
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                );

                // Stack historical download modal on top if open
                if let Some(dl_modal) = &self.historical_download_modal {
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
                if let Some(key_modal) = &self.api_key_setup_modal {
                    let key_content = key_modal.view().map(|msg| {
                        Message::Download(DownloadMessage::ApiKeySetup(msg))
                    });
                    base_content = main_dialog_modal(
                        base_content,
                        key_content,
                        Message::Download(DownloadMessage::ApiKeySetup(
                            crate::modals::download::ApiKeySetupMessage::Close,
                        )),
                    );
                }

                if let Some(dialog) = &self.confirm_dialog {
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
            sidebar::Menu::Replay => {
                let (align_x, padding) = match sidebar_pos {
                    sidebar::Position::Left => (
                        Alignment::Start,
                        padding::left(44).top(46.0 + Self::HEADER_OFFSET),
                    ),
                    sidebar::Position::Right => (
                        Alignment::End,
                        padding::right(44).top(46.0 + Self::HEADER_OFFSET),
                    ),
                };

                positioned_overlay(
                    base,
                    self.replay_manager
                        .view_setup_modal(self.timezone)
                        .map(Message::Replay),
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                    padding,
                    Alignment::Start,
                    align_x,
                )
            }
            sidebar::Menu::ThemeEditor => {
                let (align_x, padding) = match sidebar_pos {
                    sidebar::Position::Left => (Alignment::Start, padding::left(44).bottom(4)),
                    sidebar::Position::Right => (Alignment::End, padding::right(44).bottom(4)),
                };

                positioned_overlay(
                    base,
                    self.theme_editor
                        .view(&crate::style::theme_bridge::theme_to_iced(&self.theme))
                        .map(Message::ThemeEditor),
                    Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                    padding,
                    Alignment::End,
                    align_x,
                )
            }
        }
    }
}
