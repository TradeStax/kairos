//! Menu bar component and its Kairos update handler.
//!
//! The `MenuBar` struct, `Message`, `Action` enums, and view/update logic all live here
//! because the menu bar is inherently app-specific: its `Action` variants include
//! `OpenBacktest`, `OpenBacktestStrategy`, and `OpenBacktestManager`, which are domain
//! events tied to Kairos features rather than reusable UI primitives.
//!
//! Previously this lived in `components/chrome/menu_bar.rs` — moved here because
//! generic component libraries must not define app-domain message variants.

use iced::Task;

use crate::screen::dashboard;
use crate::style::{self, tokens};

use iced::widget::{button, column, container, mouse_area, row, rule, text};
use iced::{Alignment, Element, Length, padding};

use super::super::Kairos;
// Note: the local `Message` enum (MenuBar's messages) shadows the app-level Message.
// The app-level message type is referenced below as `super::super::Message`.

const MENU_MIN_WIDTH: f32 = 160.0;
// Height of a single menu item: text BODY (12) + padding top XS (4) + bottom XS (4)
const MENU_ITEM_HEIGHT: f32 = tokens::text::BODY + tokens::spacing::XS * 2.0;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Menu {
    File,
    Edit,
    Layout,
}

/// Info about a pane for the Edit menu.
#[derive(Debug, Clone)]
pub struct PaneInfo {
    pub window_id: iced::window::Id,
    pub pane: iced::widget::pane_grid::Pane,
    pub label: String,
    pub is_main_window: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Message {
    Open(Menu),
    Close,
    HoverEnter(Menu),
    Quit,
    SaveLayout,
    SaveLayoutNameChanged(String),
    SaveLayoutConfirm,
    SaveLayoutCancel,
    OverwriteLayoutConfirm,
    LoadLayout(uuid::Uuid),
    ShowSubmenu,
    HideSubmenu,
    HoverPane(Option<usize>),
    ResetPane(iced::window::Id, iced::widget::pane_grid::Pane),
    SplitPane(iced::window::Id, iced::widget::pane_grid::Pane),
    OpenBacktest,
    OpenBacktestStrategy(String),
    OpenBacktestManager,
}

#[allow(dead_code)]
pub enum Action {
    None,
    CloseWindow,
    SaveLayout(String),
    OverwriteLayout(String),
    LoadLayout(uuid::Uuid),
    ResetPane(iced::window::Id, iced::widget::pane_grid::Pane),
    SplitPane(iced::window::Id, iced::widget::pane_grid::Pane),
    OpenBacktest,
    OpenBacktestStrategy(String),
    OpenBacktestManager,
}

pub struct MenuBar {
    pub open_menu: Option<Menu>,
    pub save_layout_name: String,
    pub show_save_dialog: bool,
    pub show_submenu: bool,
    /// Stashed name for overwrite confirmation flow
    pub overwrite_layout_name: String,
    /// Index of the hovered pane in the Edit menu
    pub hovered_pane_index: Option<usize>,
    /// Cached pane info for Edit menu rendering
    pub panes_cache: Vec<PaneInfo>,
}

impl MenuBar {
    pub fn new() -> Self {
        Self {
            open_menu: None,
            save_layout_name: String::new(),
            show_save_dialog: false,
            show_submenu: false,
            overwrite_layout_name: String::new(),
            hovered_pane_index: None,
            panes_cache: Vec::new(),
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
                self.hovered_pane_index = None;
            }
            Message::HoverEnter(menu) => {
                if self.open_menu.is_some() {
                    self.open_menu = Some(menu);
                    self.show_submenu = false;
                    self.hovered_pane_index = None;
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
            Message::OverwriteLayoutConfirm => {
                let name = std::mem::take(&mut self.overwrite_layout_name);
                if !name.is_empty() {
                    return Action::OverwriteLayout(name);
                }
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
            Message::HoverPane(idx) => {
                self.hovered_pane_index = idx;
            }
            Message::ResetPane(window_id, pane) => {
                self.open_menu = None;
                self.hovered_pane_index = None;
                return Action::ResetPane(window_id, pane);
            }
            Message::SplitPane(window_id, pane) => {
                self.open_menu = None;
                self.hovered_pane_index = None;
                return Action::SplitPane(window_id, pane);
            }
            Message::OpenBacktest => {
                self.open_menu = None;
                return Action::OpenBacktest;
            }
            Message::OpenBacktestStrategy(id) => {
                self.open_menu = None;
                return Action::OpenBacktestStrategy(id);
            }
            Message::OpenBacktestManager => {
                self.open_menu = None;
                return Action::OpenBacktestManager;
            }
        }
        Action::None
    }

    #[allow(dead_code)]
    pub fn view(&self) -> Element<'_, Message> {
        let file_is_open = self.open_menu.as_ref() == Some(&Menu::File);
        let edit_is_open = self.open_menu.as_ref() == Some(&Menu::Edit);
        let layout_is_open = self.open_menu.as_ref() == Some(&Menu::Layout);

        let menu_btn = |label: &str, menu: Menu, is_open: bool| {
            mouse_area(
                button(text(label.to_string()).size(tokens::text::BODY))
                    .padding(padding::Padding {
                        top: tokens::spacing::XXS,
                        bottom: tokens::spacing::XXS,
                        left: tokens::spacing::MD,
                        right: tokens::spacing::MD,
                    })
                    .on_press(Message::Open(menu.clone()))
                    .style(move |theme, status| {
                        style::button::menu_bar_item(theme, status, is_open)
                    }),
            )
            .on_enter(Message::HoverEnter(menu))
        };

        let file_btn = menu_btn("File", Menu::File, file_is_open);
        let edit_btn = menu_btn("Edit", Menu::Edit, edit_is_open);
        let layout_btn = menu_btn("Layout", Menu::Layout, layout_is_open);

        container(
            row![file_btn, edit_btn, layout_btn]
                .spacing(tokens::spacing::XXS)
                .padding(padding::left(tokens::spacing::SM))
                .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .height(tokens::layout::MENU_BAR_HEIGHT)
        .style(style::menu_bar)
        .into()
    }

    /// Provide pane info for the Edit menu. Must be called before
    /// `view_dropdown` each frame.
    pub fn set_panes(&mut self, panes: Vec<PaneInfo>) {
        self.panes_cache = panes;
    }

    /// Returns `(dropdown, submenu)`.
    ///
    /// `strategies` is a list of `(id, name)` pairs for the
    /// File → New Backtest submenu.
    pub fn view_dropdown(
        &self,
        layouts: &[(uuid::Uuid, String, bool)],
        strategies: &[(String, String)],
    ) -> Option<(Element<'_, Message>, Option<Element<'_, Message>>)> {
        let panes = &self.panes_cache;
        let menu = self.open_menu.as_ref()?;

        match menu {
            Menu::File => {
                let backtest_item = mouse_area(
                    button(
                        row![
                            text("New Backtest").size(tokens::text::BODY),
                            iced::widget::space::horizontal().width(Length::Fill),
                            text("\u{25B8}").size(tokens::text::TINY),
                        ]
                        .align_y(Alignment::Center),
                    )
                    .width(Length::Fill)
                    .padding(menu_item_padding())
                    .on_press(Message::OpenBacktest)
                    .style(style::button::pick_list_item),
                )
                .on_enter(Message::ShowSubmenu);

                let manager_item = mouse_area(menu_item(
                    "Backtest Manager",
                    Some(Message::OpenBacktestManager),
                ))
                .on_enter(Message::HideSubmenu);

                let quit_item = mouse_area(menu_item("Quit", Some(Message::Quit)))
                    .on_enter(Message::HideSubmenu);

                let panel = menu_panel(
                    column![backtest_item, manager_item, rule::horizontal(1), quit_item,]
                        .width(MENU_MIN_WIDTH),
                );

                let submenu = if self.show_submenu && !strategies.is_empty() {
                    let mut col = column![].spacing(tokens::spacing::XXXS);
                    for (id, name) in strategies {
                        let strategy_id = id.clone();
                        col = col.push(
                            button(text(name.clone()).size(tokens::text::BODY))
                                .width(Length::Fill)
                                .padding(menu_item_padding())
                                .on_press(Message::OpenBacktestStrategy(strategy_id))
                                .style(|theme, status| {
                                    style::button::pick_list_item(theme, status)
                                }),
                        );
                    }
                    Some(
                        container(menu_panel(col.width(MENU_MIN_WIDTH)))
                            .padding(padding::top(0.0))
                            .into(),
                    )
                } else {
                    None
                };

                Some((panel, submenu))
            }
            Menu::Edit => {
                let mut col = column![].spacing(tokens::spacing::XXXS);

                if panes.is_empty() {
                    col = col.push(menu_item("No panes", None));
                } else {
                    for (idx, info) in panes.iter().enumerate() {
                        let has_arrow = info.is_main_window;
                        let pane_row = mouse_area(
                            button(
                                row![
                                    text(info.label.clone()).size(tokens::text::BODY),
                                    iced::widget::space::horizontal().width(Length::Fill),
                                    if has_arrow {
                                        text("\u{25B8}").size(tokens::text::TINY)
                                    } else {
                                        text("").size(tokens::text::TINY)
                                    },
                                ]
                                .align_y(Alignment::Center),
                            )
                            .width(Length::Fill)
                            .padding(menu_item_padding())
                            .on_press(Message::HoverPane(Some(idx)))
                            .style(style::button::pick_list_item),
                        )
                        .on_enter(Message::HoverPane(Some(idx)));

                        col = col.push(pane_row);
                    }
                }

                let panel = menu_panel(col.width(MENU_MIN_WIDTH));

                // Submenu for hovered pane
                let submenu = self.hovered_pane_index.and_then(|idx| {
                    let info = panes.get(idx)?;
                    if !info.is_main_window {
                        return None;
                    }

                    let win = info.window_id;
                    let pane = info.pane;

                    let reset_btn = button(text("Reset").size(tokens::text::BODY))
                        .width(Length::Fill)
                        .padding(menu_item_padding())
                        .on_press(Message::ResetPane(win, pane))
                        .style(style::button::pick_list_item);

                    let split_btn = button(text("Split").size(tokens::text::BODY))
                        .width(Length::Fill)
                        .padding(menu_item_padding())
                        .on_press(Message::SplitPane(win, pane))
                        .style(style::button::pick_list_item);

                    let sub_col = column![reset_btn, split_btn].width(MENU_MIN_WIDTH);
                    let offset = idx as f32 * MENU_ITEM_HEIGHT;

                    Some(
                        container(menu_panel(sub_col))
                            .padding(padding::top(offset))
                            .into(),
                    )
                });

                Some((panel, submenu))
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
                    .style(style::button::pick_list_item),
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
        .style(style::button::pick_list_item);

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

// ── Kairos handler ────────────────────────────────────────────────────

impl Kairos {
    pub(crate) fn handle_menu_bar(&mut self, msg: Message) -> Task<crate::app::Message> {
        // Pre-fill save dialog name when opening
        if matches!(msg, Message::SaveLayout) {
            self.menu_bar.save_layout_name = self
                .persistence
                .layout_manager
                .generate_unique_layout_name();
        }

        // Refresh pane info when Edit menu is opened or hovered
        if matches!(
            msg,
            Message::Open(Menu::Edit) | Message::HoverEnter(Menu::Edit)
        ) {
            self.refresh_edit_menu_panes();
        }

        let action = self.menu_bar.update(msg);

        match action {
            Action::CloseWindow => {
                return self.handle_window_close(self.main_window.id);
            }
            Action::SaveLayout(name) => {
                // Check if a layout with this name already exists
                let collision = self
                    .persistence
                    .layout_manager
                    .layouts
                    .iter()
                    .any(|l| l.id.name == name);

                if collision {
                    // Stash name and show overwrite confirmation
                    self.menu_bar.overwrite_layout_name = name.clone();
                    self.ui.confirm_dialog =
                        Some(crate::components::overlay::confirm_dialog::ConfirmDialog {
                            message: format!("Layout \"{}\" already exists. Overwrite?", name),
                            on_confirm: Box::new(crate::app::Message::MenuBar(
                                Message::OverwriteLayoutConfirm,
                            )),
                            on_confirm_btn_text: Some("Overwrite".into()),
                        });
                } else if let Some(active_id) = self
                    .persistence
                    .layout_manager
                    .active_layout_id()
                    .map(|l| l.unique)
                {
                    self.handle_layout_clone(active_id);
                    // Rename the newly created layout (last in the list)
                    if let Some(new_layout) = self.persistence.layout_manager.layouts.last() {
                        let new_id = new_layout.id.unique;
                        let unique_name = self
                            .persistence
                            .layout_manager
                            .ensure_unique_name(&name, new_id);
                        if let Some(layout) = self.persistence.layout_manager.layouts.last_mut() {
                            layout.id.name = unique_name;
                        }
                    }
                }
            }
            Action::OverwriteLayout(name) => {
                self.ui.confirm_dialog = None;
                if let Some(active_dashboard) = self.active_dashboard() {
                    let ser = crate::persistence::Dashboard::from(active_dashboard);
                    if let Some(layout) = self
                        .persistence
                        .layout_manager
                        .layouts
                        .iter_mut()
                        .find(|l| l.id.name == name)
                    {
                        let mut popout_windows = Vec::new();
                        for (pane, window_spec) in &ser.popout {
                            popout_windows.push((
                                crate::persistence::configuration(pane.clone()),
                                window_spec.clone(),
                            ));
                        }

                        layout.dashboard = crate::screen::dashboard::Dashboard::from_config(
                            crate::persistence::configuration(ser.pane),
                            popout_windows,
                            None,
                            self.persistence.data_index.clone(),
                        );
                    }
                }
            }
            Action::LoadLayout(id) => {
                return self.handle_layout_select(id);
            }
            Action::ResetPane(window_id, pane) => {
                return self.handle_dashboard(
                    None,
                    dashboard::Message::Pane(
                        window_id,
                        dashboard::pane::Message::ReplacePane(pane),
                    ),
                );
            }
            Action::SplitPane(window_id, pane) => {
                return self.handle_dashboard(
                    None,
                    dashboard::Message::Pane(
                        window_id,
                        dashboard::pane::Message::SplitPane(
                            iced::widget::pane_grid::Axis::Horizontal,
                            pane,
                        ),
                    ),
                );
            }
            Action::OpenBacktest => {
                return Task::done(crate::app::Message::Backtest(
                    crate::app::messages::BacktestMessage::OpenLaunchModal { strategy_id: None },
                ));
            }
            Action::OpenBacktestStrategy(id) => {
                return Task::done(crate::app::Message::Backtest(
                    crate::app::messages::BacktestMessage::OpenLaunchModal {
                        strategy_id: Some(id),
                    },
                ));
            }
            Action::OpenBacktestManager => {
                return Task::done(crate::app::Message::Backtest(
                    crate::app::messages::BacktestMessage::OpenManager,
                ));
            }
            Action::None => {}
        }
        Task::none()
    }
}
