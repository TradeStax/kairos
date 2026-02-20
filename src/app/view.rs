use crate::components::display::toast;
use crate::screen::dashboard;
use crate::style::tokens;
use crate::{style, window};
use data::sidebar;

use iced::{
    Alignment, Element, Length, padding,
    widget::{column, container, row, text},
};

use crate::components::chrome::{menu_bar, title_bar};

use super::{Kairos, Message, APP_NAME};

impl Kairos {
    pub fn view(&self, id: window::Id) -> Element<'_, Message> {
        let Some(dashboard) = self.active_dashboard() else {
            return container(text("No active layout"))
                .center(Length::Fill)
                .into();
        };
        let sidebar_pos = self.sidebar.position();
        let window_title = self.title(id);

        let tickers_info = &self.tickers_info;

        let content = if id == self.main_window.id {
            let sidebar_view = self.sidebar.view().map(Message::Sidebar);

            let dashboard_view = dashboard
                .view(&self.main_window, tickers_info, self.timezone)
                .map(move |msg| Message::Dashboard {
                    layout_id: None,
                    event: msg,
                });

            let header: Element<'_, Message> = {
                #[cfg(target_os = "macos")]
                {
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
                if let Some(flyout_content) = self.sidebar.view_tool_flyout() {
                    use iced::widget::{mouse_area, opaque, stack};

                    let flyout_content = flyout_content.map(Message::Sidebar);
                    let y = self.sidebar.flyout_y_offset();
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
                        crate::modals::drawing_tools::Message::ExpandGroup(None),
                    ));

                    stack![base, mouse_area(positioned).on_press(on_close)].into()
                } else {
                    base.into()
                };

            // Menu bar dropdown overlay
            let base: Element<'_, Message> = {
                use iced::widget::{mouse_area, opaque, stack};

                let active_id = self.layout_manager.active_layout_id().map(|lid| lid.unique);
                let layouts: Vec<(uuid::Uuid, String, bool)> = self
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

                if let Some((dropdown, submenu)) = self.menu_bar.view_dropdown(&layouts) {
                    let y = tokens::layout::TITLE_BAR_HEIGHT + tokens::layout::MENU_BAR_HEIGHT;
                    let x = match self.menu_bar.open_menu {
                        Some(menu_bar::Menu::File) => tokens::spacing::SM,
                        Some(menu_bar::Menu::Layout) => tokens::spacing::SM + 46.0,
                        None => 0.0,
                    };

                    // Primary dropdown + optional submenu side-by-side
                    let dropdown = dropdown.map(Message::MenuBar);
                    let menus: Element<'_, Message> = if let Some(sub) = submenu {
                        row![opaque(dropdown), opaque(sub.map(Message::MenuBar)),].into()
                    } else {
                        opaque(dropdown).into()
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

            if let Some(menu) = self.sidebar.active_menu() {
                self.view_with_modal(base, dashboard, menu)
            } else {
                base
            }
        } else {
            let popout_content = container(
                dashboard
                    .view_window(id, &self.main_window, tickers_info, self.timezone)
                    .map(move |msg| Message::Dashboard {
                        layout_id: None,
                        event: msg,
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
                    title_bar::view_title_bar(id, window_title, false,),
                    popout_content,
                ]
                .into()
            }
        };

        // Overlay the floating replay controller when replay is active
        // and controller is visible
        let content = if self.replay_manager.data_loaded && self.replay_manager.controller_visible {
            use iced::widget::stack;
            let pos = self.replay_manager.panel_position;
            let controller = self
                .replay_manager
                .view_floating_controller(self.timezone)
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

        toast::Manager::new(
            content,
            &self.notifications,
            match sidebar_pos {
                sidebar::Position::Left => Alignment::Start,
                sidebar::Position::Right => Alignment::End,
            },
            Message::RemoveNotification,
        )
        .into()
    }
}
