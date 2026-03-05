use crate::components::display::toast;
use crate::config::sidebar;
use crate::screen::dashboard;
use crate::style::tokens;
use crate::{style, window};

use iced::{
    Alignment, Element, Length, padding,
    widget::{column, container, row, text},
};

use crate::app::update::menu_bar;
use crate::components::chrome::title_bar;

use super::super::{Kairos, Message};

#[allow(dead_code)]
fn map_title_bar_action(action: title_bar::Action) -> Message {
    use super::super::WindowMessage;
    match action {
        title_bar::Action::Drag(id) => Message::Window(WindowMessage::Drag(id)),
        title_bar::Action::Minimize(id) => Message::Window(WindowMessage::Minimize(id)),
        title_bar::Action::ToggleMaximize(id) => Message::Window(WindowMessage::ToggleMaximize(id)),
        title_bar::Action::Close(id) => Message::Window(WindowMessage::Close(id)),
        title_bar::Action::Hover(hovered) => Message::Window(WindowMessage::TitleBarHover(hovered)),
    }
}

impl Kairos {
    #[allow(unused_variables)]
    pub fn view(&self, id: window::Id) -> Element<'_, Message> {
        let Some(dashboard) = self.active_dashboard() else {
            return container(text("No active layout"))
                .center(Length::Fill)
                .into();
        };
        let sidebar_pos = self.ui.sidebar.position();
        let window_title = self.title(id);

        let tickers_info = &self.persistence.tickers_info;
        let ticker_ranges = &self.persistence.ticker_ranges;

        let content = if id == self.main_window.id {
            let sidebar_view = self.ui.sidebar.view().map(Message::Sidebar);

            let dashboard_view = dashboard
                .view(
                    &self.main_window,
                    tickers_info,
                    self.ui.timezone,
                    ticker_ranges,
                )
                .map(move |msg| Message::Dashboard {
                    layout_id: None,
                    event: Box::new(msg),
                });

            let header: Element<'_, Message> = {
                #[cfg(target_os = "macos")]
                {
                    use super::super::APP_NAME;
                    iced::widget::center(
                        text(APP_NAME)
                            .font(iced::Font {
                                weight: iced::font::Weight::Bold,
                                ..Default::default()
                            })
                            .size(tokens::text::HEADING)
                            .style(style::title_text),
                    )
                    .height(20)
                    .align_y(Alignment::Center)
                    .padding(padding::top(tokens::spacing::XS))
                    .into()
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let title = title_bar::view_title_bar(
                        id,
                        window_title.clone(),
                        self.main_window.is_maximized,
                        self.ui.title_bar_hovered,
                        map_title_bar_action,
                    );
                    let menu = self.menu_bar.view().map(Message::MenuBar);
                    column![title, menu].into()
                }
            };

            let base = column![
                header,
                match sidebar_pos {
                    sidebar::Position::Left => row![sidebar_view, dashboard_view,],
                    sidebar::Position::Right => row![dashboard_view, sidebar_view],
                }
                .spacing(tokens::spacing::MD)
                .padding(tokens::spacing::MD),
            ];

            // Layer the drawing tool flyout over the base if expanded
            let base: Element<'_, Message> =
                if let Some(flyout_content) = self.ui.sidebar.view_tool_flyout() {
                    use iced::widget::{mouse_area, opaque, stack};

                    let flyout_content = flyout_content.map(Message::Sidebar);
                    let y = self.ui.sidebar.flyout_y_offset();
                    let sidebar_w = tokens::layout::SIDEBAR_WIDTH + tokens::spacing::MD;

                    let positioned = match sidebar_pos {
                        sidebar::Position::Left => container(opaque(flyout_content))
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .padding(padding::Padding {
                                top: y,
                                right: 0.0,
                                bottom: 0.0,
                                left: sidebar_w,
                            }),
                        sidebar::Position::Right => container(opaque(flyout_content))
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .padding(padding::Padding {
                                top: y,
                                right: sidebar_w,
                                bottom: 0.0,
                                left: 0.0,
                            })
                            .align_x(Alignment::End),
                    };

                    let on_close = Message::Sidebar(dashboard::sidebar::Message::DrawingTools(
                        crate::modals::drawing::tools::Message::ExpandGroup(None),
                    ));

                    stack![base, mouse_area(positioned).on_press(on_close)].into()
                } else {
                    base.into()
                };

            // Layer settings flyout if expanded
            let base: Element<'_, Message> =
                if let Some(flyout_content) = self.ui.sidebar.view_settings_flyout() {
                    use iced::widget::{mouse_area, opaque, stack};

                    let flyout_content = flyout_content.map(Message::Sidebar);
                    let sidebar_w = tokens::layout::SIDEBAR_WIDTH + tokens::spacing::MD;

                    let positioned = match sidebar_pos {
                        sidebar::Position::Left => container(opaque(flyout_content))
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .align_y(Alignment::End)
                            .padding(padding::Padding {
                                top: 0.0,
                                right: 0.0,
                                bottom: tokens::spacing::MD,
                                left: sidebar_w,
                            }),
                        sidebar::Position::Right => container(opaque(flyout_content))
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .align_y(Alignment::End)
                            .align_x(Alignment::End)
                            .padding(padding::Padding {
                                top: 0.0,
                                right: sidebar_w,
                                bottom: tokens::spacing::MD,
                                left: 0.0,
                            }),
                    };

                    let on_close = Message::Sidebar(dashboard::sidebar::Message::Settings(
                        crate::modals::preferences::Message::ToggleFlyout(false),
                    ));

                    stack![base, mouse_area(positioned).on_press(on_close)].into()
                } else {
                    base
                };

            // Menu bar dropdown overlay
            let base: Element<'_, Message> = {
                use iced::widget::{mouse_area, opaque, stack};

                let active_id = self
                    .persistence
                    .layout_manager
                    .active_layout_id()
                    .map(|lid| lid.unique);
                let layouts: Vec<(uuid::Uuid, String, bool)> = self
                    .persistence
                    .layout_manager
                    .layouts
                    .iter()
                    .map(|l| {
                        (
                            l.id.unique,
                            l.id.name.clone(),
                            Some(l.id.unique) == active_id,
                        )
                    })
                    .collect();

                let strategies: Vec<(String, String)> = self
                    .modals
                    .backtest
                    .strategy_registry
                    .list()
                    .into_iter()
                    .map(|s| (s.id, s.name))
                    .collect();

                if let Some((dropdown, submenu)) =
                    self.menu_bar.view_dropdown(&layouts, &strategies)
                {
                    let y = tokens::layout::TITLE_BAR_HEIGHT + tokens::layout::MENU_BAR_HEIGHT;
                    // Each menu button is roughly 46px wide (text + padding)
                    let btn_w = 46.0;
                    let x = match self.menu_bar.open_menu {
                        Some(menu_bar::Menu::File) => tokens::spacing::SM,
                        Some(menu_bar::Menu::Edit) => tokens::spacing::SM + btn_w,
                        Some(menu_bar::Menu::Layout) => tokens::spacing::SM + btn_w * 2.0,
                        None => 0.0,
                    };

                    // Primary dropdown + optional submenu side-by-side
                    let dropdown = dropdown.map(Message::MenuBar);
                    let menus: Element<'_, Message> = if let Some(sub) = submenu {
                        row![opaque(dropdown), opaque(sub.map(Message::MenuBar)),].into()
                    } else {
                        opaque(dropdown)
                    };

                    let positioned = container(menus)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .padding(padding::Padding {
                            top: y,
                            right: 0.0,
                            bottom: 0.0,
                            left: x,
                        });

                    stack![
                        base,
                        mouse_area(positioned).on_press(Message::MenuBar(menu_bar::Message::Close))
                    ]
                    .into()
                } else {
                    base
                }
            };

            // Save layout dialog overlay
            let base: Element<'_, Message> = if self.menu_bar.show_save_dialog {
                use crate::components::overlay::form_modal::FormModalBuilder;
                use iced::widget::text_input;

                FormModalBuilder::new(
                    "Save Layout",
                    text_input("Layout name", &self.menu_bar.save_layout_name)
                        .on_input(|s| Message::MenuBar(menu_bar::Message::SaveLayoutNameChanged(s)))
                        .on_submit(Message::MenuBar(menu_bar::Message::SaveLayoutConfirm)),
                    Message::MenuBar(menu_bar::Message::SaveLayoutConfirm),
                    Message::MenuBar(menu_bar::Message::SaveLayoutCancel),
                )
                .max_width(320.0)
                .view(base)
            } else {
                base
            };

            // Confirm dialog overlay (e.g. layout overwrite)
            let base: Element<'_, Message> = if let Some(dialog) = &self.ui.confirm_dialog
                && self.ui.sidebar.active_menu().is_none()
            {
                use crate::components::overlay::confirm_dialog::ConfirmDialogBuilder;

                let on_cancel = Message::ToggleDialogModal(None);
                let mut builder = ConfirmDialogBuilder::new(
                    dialog.message.clone(),
                    *dialog.on_confirm.clone(),
                    on_cancel,
                );
                if let Some(btn_text) = &dialog.on_confirm_btn_text {
                    builder = builder.confirm_text(btn_text.clone());
                }
                builder.view(base)
            } else {
                base
            };

            if let Some(menu) = self.ui.sidebar.active_menu() {
                self.view_with_modal(base, dashboard, menu)
            } else {
                base
            }
        } else {
            let popout_content = container(
                dashboard
                    .view_window(
                        id,
                        &self.main_window,
                        tickers_info,
                        self.ui.timezone,
                        ticker_ranges,
                    )
                    .map(move |msg| Message::Dashboard {
                        layout_id: None,
                        event: Box::new(msg),
                    }),
            )
            .padding(padding::top(style::TITLE_PADDING_TOP));

            #[cfg(target_os = "macos")]
            {
                popout_content.into()
            }
            #[cfg(not(target_os = "macos"))]
            {
                column![
                    title_bar::view_title_bar(id, window_title, false, false, map_title_bar_action),
                    popout_content,
                ]
                .into()
            }
        };

        // Overlay the backtest launch modal when open.
        let content: Element<'_, Message> = if self.modals.backtest.show_backtest_modal {
            use crate::modals::main_dialog_modal;
            let modal_content = self.modals.backtest.backtest_launch_modal.view().map(|m| {
                Message::Backtest(
                    super::super::messages::BacktestMessage::LaunchModalInteraction(m),
                )
            });
            main_dialog_modal(
                content,
                modal_content,
                Message::Backtest(
                    super::super::messages::BacktestMessage::LaunchModalInteraction(
                        crate::screen::backtest::launch::Message::Close,
                    ),
                ),
            )
        } else {
            content
        };

        // Overlay the backtest management modal when open.
        let content: Element<'_, Message> = if self.modals.backtest.show_backtest_manager {
            use crate::modals::main_dialog_modal;
            use crate::screen::backtest::manager::ManagerMessage;
            let modal_content = self
                .modals
                .backtest
                .backtest_manager
                .view(&self.modals.backtest.backtest_history, self.ui.timezone)
                .map(|m| {
                    Message::Backtest(super::super::messages::BacktestMessage::ManagerInteraction(
                        m,
                    ))
                });
            main_dialog_modal(
                content,
                modal_content,
                Message::Backtest(super::super::messages::BacktestMessage::ManagerInteraction(
                    ManagerMessage::Close,
                )),
            )
        } else {
            content
        };

        // Overlay the floating replay controller when replay is active
        // and controller is visible
        let content = if self.modals.replay_manager.data_loaded
            && self.modals.replay_manager.controller_visible
        {
            use iced::widget::stack;
            let pos = self.modals.replay_manager.panel_position;
            let controller = self
                .modals
                .replay_manager
                .view_floating_controller(self.ui.timezone)
                .map(Message::Replay);
            let overlay = container(controller).padding(iced::Padding {
                top: pos.y,
                right: 0.0,
                bottom: 0.0,
                left: pos.x,
            });

            stack![content, overlay].into()
        } else {
            content
        };

        // Overlay update modal when visible
        let content: Element<'_, Message> = if self.modals.update_state.show_modal {
            let modal_content = crate::modals::update::view_update_modal(&self.modals.update_state)
                .map(Message::Update);
            crate::modals::main_dialog_modal(
                content,
                modal_content,
                Message::Update(super::super::messages::UpdateMessage::Dismiss),
            )
        } else {
            content
        };

        toast::Manager::new(
            content,
            &self.ui.notifications,
            match sidebar_pos {
                sidebar::Position::Left => Alignment::Start,
                sidebar::Position::Right => Alignment::End,
            },
            Message::RemoveNotification,
        )
        .into()
    }
}
