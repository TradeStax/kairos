use crate::style::{self, tokens};

use iced::widget::{button, column, container, mouse_area, row, text};
use iced::{Alignment, Element, Length, padding};

const MENU_MIN_WIDTH: f32 = 160.0;
// Height of a single menu item: text BODY (12) + padding top XS (4) + bottom XS (4)
const MENU_ITEM_HEIGHT: f32 = tokens::text::BODY + tokens::spacing::XS * 2.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Menu {
    File,
    Layout,
}

#[derive(Debug, Clone)]
pub enum Message {
    Open(Menu),
    Close,
    HoverEnter(Menu),
    Quit,
    SaveLayout,
    SaveLayoutNameChanged(String),
    SaveLayoutConfirm,
    SaveLayoutCancel,
    LoadLayout(uuid::Uuid),
    ShowSubmenu,
    HideSubmenu,
}

pub enum Action {
    None,
    CloseWindow,
    SaveLayout(String),
    LoadLayout(uuid::Uuid),
}

pub struct MenuBar {
    pub open_menu: Option<Menu>,
    pub save_layout_name: String,
    pub show_save_dialog: bool,
    pub show_submenu: bool,
}

impl MenuBar {
    pub fn new() -> Self {
        Self {
            open_menu: None,
            save_layout_name: String::new(),
            show_save_dialog: false,
            show_submenu: false,
        }
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Open(menu) => {
                if self.open_menu.as_ref() == Some(&menu) {
                    self.open_menu = None;
                    self.show_submenu = false;
                } else {
                    self.open_menu = Some(menu);
                    self.show_submenu = false;
                }
            }
            Message::Close => {
                self.open_menu = None;
                self.show_submenu = false;
            }
            Message::HoverEnter(menu) => {
                if self.open_menu.is_some() {
                    self.open_menu = Some(menu);
                    self.show_submenu = false;
                }
            }
            Message::Quit => {
                self.open_menu = None;
                return Action::CloseWindow;
            }
            Message::SaveLayout => {
                self.open_menu = None;
                self.show_submenu = false;
                self.show_save_dialog = true;
            }
            Message::SaveLayoutNameChanged(name) => {
                self.save_layout_name = name.chars().take(20).collect();
            }
            Message::SaveLayoutConfirm => {
                let name = self.save_layout_name.trim().to_string();
                self.show_save_dialog = false;
                self.save_layout_name.clear();
                if !name.is_empty() {
                    return Action::SaveLayout(name);
                }
            }
            Message::SaveLayoutCancel => {
                self.show_save_dialog = false;
                self.save_layout_name.clear();
            }
            Message::LoadLayout(id) => {
                self.open_menu = None;
                self.show_submenu = false;
                return Action::LoadLayout(id);
            }
            Message::ShowSubmenu => {
                self.show_submenu = true;
            }
            Message::HideSubmenu => {
                self.show_submenu = false;
            }
        }
        Action::None
    }

    pub fn view(&self) -> Element<'_, Message> {
        let file_is_open = self.open_menu.as_ref() == Some(&Menu::File);
        let layout_is_open = self.open_menu.as_ref() == Some(&Menu::Layout);

        let file_btn = mouse_area(
            button(text("File").size(tokens::text::BODY))
                .padding(padding::Padding {
                    top: tokens::spacing::XXS,
                    bottom: tokens::spacing::XXS,
                    left: tokens::spacing::MD,
                    right: tokens::spacing::MD,
                })
                .on_press(Message::Open(Menu::File))
                .style(move |theme, status| {
                    style::button::menu_bar_item(theme, status, file_is_open)
                }),
        )
        .on_enter(Message::HoverEnter(Menu::File));

        let layout_btn = mouse_area(
            button(text("Layout").size(tokens::text::BODY))
                .padding(padding::Padding {
                    top: tokens::spacing::XXS,
                    bottom: tokens::spacing::XXS,
                    left: tokens::spacing::MD,
                    right: tokens::spacing::MD,
                })
                .on_press(Message::Open(Menu::Layout))
                .style(move |theme, status| {
                    style::button::menu_bar_item(theme, status, layout_is_open)
                }),
        )
        .on_enter(Message::HoverEnter(Menu::Layout));

        container(
            row![file_btn, layout_btn]
                .spacing(tokens::spacing::XXS)
                .padding(padding::left(tokens::spacing::SM))
                .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .height(tokens::layout::MENU_BAR_HEIGHT)
        .style(style::menu_bar)
        .into()
    }

    /// Returns `(dropdown, submenu)`.
    pub fn view_dropdown(
        &self,
        layouts: &[(uuid::Uuid, String, bool)],
    ) -> Option<(Element<'_, Message>, Option<Element<'_, Message>>)> {
        let menu = self.open_menu.as_ref()?;

        match menu {
            Menu::File => {
                let quit_item = menu_item("Quit", Some(Message::Quit));
                let panel = menu_panel(column![quit_item].width(MENU_MIN_WIDTH));
                Some((panel, None))
            }
            Menu::Layout => {
                let save_item = mouse_area(menu_item("Save Layout", Some(Message::SaveLayout)))
                    .on_enter(Message::HideSubmenu);

                let load_item_row = mouse_area(
                    button(
                        row![
                            text("Load Layout").size(tokens::text::BODY),
                            iced::widget::space::horizontal().width(Length::Fill),
                            text("\u{25B8}").size(tokens::text::TINY),
                        ]
                        .align_y(Alignment::Center),
                    )
                    .width(Length::Fill)
                    .padding(menu_item_padding())
                    .on_press(Message::ShowSubmenu)
                    .style(|theme, status| style::button::pick_list_item(theme, status)),
                )
                .on_enter(Message::ShowSubmenu);

                let panel = menu_panel(column![save_item, load_item_row].width(MENU_MIN_WIDTH));

                let submenu = if self.show_submenu {
                    let mut col = column![].spacing(tokens::spacing::XXXS);

                    for (id, name, is_active) in layouts {
                        if *is_active {
                            col = col.push(
                                button(text(name.clone()).size(tokens::text::BODY))
                                    .width(Length::Fill)
                                    .padding(menu_item_padding())
                                    .style(move |theme, status| {
                                        style::button::list_item_selected(theme, status, true)
                                    }),
                            );
                        } else {
                            let layout_id = *id;
                            col = col.push(
                                button(text(name.clone()).size(tokens::text::BODY))
                                    .width(Length::Fill)
                                    .padding(menu_item_padding())
                                    .on_press(Message::LoadLayout(layout_id))
                                    .style(|theme, status| {
                                        style::button::pick_list_item(theme, status)
                                    }),
                            );
                        }
                    }

                    // Offset submenu to align with "Load Layout" row
                    Some(
                        container(menu_panel(col.width(MENU_MIN_WIDTH)))
                            .padding(padding::top(MENU_ITEM_HEIGHT))
                            .into(),
                    )
                } else {
                    None
                };

                Some((panel, submenu))
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

fn menu_item_padding() -> padding::Padding {
    padding::Padding {
        top: tokens::spacing::XS,
        bottom: tokens::spacing::XS,
        left: tokens::spacing::MD,
        right: tokens::spacing::XL,
    }
}

fn menu_item<'a>(label: &str, on_press: Option<Message>) -> Element<'a, Message> {
    let mut btn = button(text(label.to_string()).size(tokens::text::BODY))
        .width(Length::Fill)
        .padding(menu_item_padding())
        .style(|theme, status| style::button::pick_list_item(theme, status));

    if let Some(msg) = on_press {
        btn = btn.on_press(msg);
    }

    btn.into()
}

fn menu_panel<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .padding(tokens::spacing::XS)
        .style(style::dropdown_container)
        .into()
}
