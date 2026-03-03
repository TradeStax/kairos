//! Sidebar for navigation and menu access
//!
//! Provides navigation buttons for layout management, replay, connections,
//! settings, and drawing tools with flyout submenus.

use crate::config::Sidebar as SidebarConfig;
use crate::config::sidebar;
use crate::{
    components::display::tooltip::button_with_tooltip,
    components::primitives::{Icon, icon_text},
    modals::drawing::tools::{self, DrawingToolsPanel, SidebarGroup},
    modals::preferences::{self, SettingsPanel},
    persistence::SavedState,
    style,
    style::tokens,
};
use iced::widget::tooltip::Position as TooltipPosition;

use iced::{
    Alignment, Element, Length, Task,
    widget::{column, container, rule, space},
};

#[derive(Debug, Clone)]
pub enum Message {
    ToggleSidebarMenu(Option<sidebar::Menu>),
    DrawingTools(tools::Message),
    Settings(preferences::Message),
}

pub enum SidebarAction {
    Drawing(tools::Action),
    Settings(preferences::Action),
}

pub struct Sidebar {
    pub state: SidebarConfig,
    pub drawing_tools: DrawingToolsPanel,
    pub settings: SettingsPanel,
}

impl Sidebar {
    pub fn new(state: &SavedState) -> Self {
        Self {
            state: state.sidebar.clone(),
            drawing_tools: DrawingToolsPanel::new(),
            settings: SettingsPanel::new(),
        }
    }

    pub fn update(&mut self, message: Message) -> (Task<Message>, Option<SidebarAction>) {
        match message {
            Message::ToggleSidebarMenu(menu) => {
                self.set_menu(menu.filter(|&m| !self.is_menu_active(m)));
                // Close any open flyout when toggling sidebar menus
                self.drawing_tools.expanded_group = None;
                self.settings.flyout_expanded = false;
                (Task::none(), None)
            }
            Message::DrawingTools(msg) => {
                self.settings.flyout_expanded = false;
                let action = self.drawing_tools.update(msg);
                (Task::none(), action.map(SidebarAction::Drawing))
            }
            Message::Settings(msg) => {
                // Close drawing flyout when settings flyout opens
                if matches!(msg, preferences::Message::ToggleFlyout(true)) {
                    self.drawing_tools.expanded_group = None;
                }
                let action = self.settings.update(msg);
                (Task::none(), action.map(SidebarAction::Settings))
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let state = &self.state;

        let tooltip_position = if state.position == sidebar::Position::Left {
            TooltipPosition::Right
        } else {
            TooltipPosition::Left
        };

        self.build_sidebar_content(tooltip_position)
    }

    fn build_sidebar_content(&self, tooltip_position: TooltipPosition) -> Element<'_, Message> {
        let replay_btn = {
            let is_active = self.is_menu_active(sidebar::Menu::Replay);

            button_with_tooltip(
                icon_text(Icon::Replay, 14)
                    .width(24)
                    .height(24)
                    .align_x(Alignment::Center),
                Message::ToggleSidebarMenu(Some(sidebar::Menu::Replay)),
                Some("Replay"),
                tooltip_position,
                move |theme, status| crate::style::button::transparent(theme, status, is_active),
            )
        };

        // Drawing tool buttons - one per sidebar group
        let drawing_buttons = self.build_drawing_buttons(tooltip_position);

        let connections_btn = {
            let is_active = self.is_menu_active(sidebar::Menu::Connections);

            button_with_tooltip(
                icon_text(Icon::Link, 14)
                    .width(24)
                    .height(24)
                    .align_x(Alignment::Center),
                Message::ToggleSidebarMenu(Some(sidebar::Menu::Connections)),
                Some("Connections"),
                tooltip_position,
                move |theme, status| crate::style::button::transparent(theme, status, is_active),
            )
        };

        let settings_btn: Element<'_, Message> = {
            let is_expanded = self.settings.flyout_expanded;
            let is_active = is_expanded
                || self.settings.active_modal.is_some()
                || self.is_menu_active(sidebar::Menu::Settings)
                || self.is_menu_active(sidebar::Menu::ThemeEditor);

            let tip = if is_expanded { None } else { Some("Settings") };

            button_with_tooltip(
                icon_text(Icon::Cog, 14)
                    .width(24)
                    .height(24)
                    .align_x(Alignment::Center),
                Message::Settings(preferences::Message::ToggleFlyout(!is_expanded)),
                tip,
                tooltip_position,
                move |theme, status| style::button::transparent(theme, status, is_active),
            )
        };

        let mut content = column![]
            .width(tokens::layout::SIDEBAR_WIDTH)
            .spacing(tokens::spacing::XS)
            .align_x(Alignment::Center);

        // Top section: drawing tools
        for btn in drawing_buttons {
            content = content.push(btn);
        }

        // Spacer
        content = content.push(space::vertical().height(Length::Fill));

        // Bottom section
        content = content.push(replay_btn);
        content = content.push(connections_btn);
        content = content.push(settings_btn);

        content.into()
    }

    /// Build one button per sidebar group.
    /// Groups with submenus show a chevron on hover instead of a tooltip.
    fn build_drawing_buttons(
        &self,
        tooltip_position: TooltipPosition,
    ) -> Vec<Element<'_, Message>> {
        let active_tool = self.drawing_tools.active_tool;

        SidebarGroup::ALL
            .iter()
            .map(|&group| {
                let selected_tool = self.drawing_tools.get_selected_for_group(group);
                let icon = group.icon(selected_tool);

                let is_active = if group == SidebarGroup::Select {
                    active_tool == crate::drawing::DrawingTool::None
                } else {
                    SidebarGroup::for_tool(active_tool) == group
                        && active_tool != crate::drawing::DrawingTool::None
                };

                let is_expanded = self.drawing_tools.expanded_group == Some(group);

                // For groups with submenus, select the tool
                // and toggle the flyout.
                // For Select, directly activate the tool.
                let msg = if group.has_submenu() {
                    Message::DrawingTools(tools::Message::GroupClicked {
                        tool: selected_tool,
                        group,
                    })
                } else {
                    Message::DrawingTools(tools::Message::ToolSelected(selected_tool))
                };

                {
                    let tip = if is_expanded {
                        None
                    } else {
                        Some(group.label())
                    };

                    button_with_tooltip(
                        icon_text(icon, 14)
                            .width(24)
                            .height(24)
                            .align_x(Alignment::Center),
                        msg,
                        tip,
                        tooltip_position,
                        move |theme, status| {
                            style::button::transparent(theme, status, is_active || is_expanded)
                        },
                    )
                }
            })
            .chain(std::iter::once(self.snap_button(tooltip_position)))
            .collect()
    }

    /// Snap toggle button using button_with_tooltip.
    fn snap_button(&self, tooltip_position: TooltipPosition) -> Element<'_, Message> {
        let snap_enabled = self.drawing_tools.snap_enabled;
        let icon = if snap_enabled {
            Icon::SnapOn
        } else {
            Icon::SnapOff
        };

        button_with_tooltip(
            icon_text(icon, 14)
                .width(24)
                .height(24)
                .align_x(Alignment::Center),
            Message::DrawingTools(tools::Message::ToggleSnap),
            Some(if snap_enabled { "Snap On" } else { "Snap Off" }),
            tooltip_position,
            move |theme, status| style::button::transparent(theme, status, snap_enabled),
        )
    }

    /// Build the flyout submenu content for the currently expanded group.
    /// Vertical column of icon buttons with tooltips, same width as sidebar.
    pub fn view_tool_flyout(&self) -> Option<Element<'_, Message>> {
        let group = self.drawing_tools.expanded_group?;

        let sections = group.tool_sections();
        let active_tool = self.drawing_tools.active_tool;

        let tooltip_pos = if self.state.position == sidebar::Position::Left {
            TooltipPosition::Right
        } else {
            TooltipPosition::Left
        };

        let mut col = column![]
            .spacing(tokens::spacing::XS)
            .width(tokens::layout::SIDEBAR_WIDTH)
            .align_x(Alignment::Center);

        for (i, section) in sections.iter().enumerate() {
            if i > 0 {
                col = col.push(rule::horizontal(1.0).style(style::split_ruler));
            }

            for &tool in *section {
                let icon = tools::tool_icon(tool);
                let label = tools::tool_label(tool);
                let is_selected = tool == active_tool;

                let btn = button_with_tooltip(
                    icon_text(icon, 14)
                        .width(24)
                        .height(24)
                        .align_x(Alignment::Center),
                    Message::DrawingTools(tools::Message::ToolSelected(tool)),
                    Some(label),
                    tooltip_pos,
                    move |theme, status| style::button::transparent(theme, status, is_selected),
                );

                col = col.push(btn);
            }
        }

        let panel = container(col)
            .padding(tokens::spacing::XS)
            .style(style::floating_panel);

        Some(panel.into())
    }

    /// Build the settings flyout content.
    pub fn view_settings_flyout(&self) -> Option<Element<'_, Message>> {
        let tooltip_position = if self.state.position == sidebar::Position::Left {
            TooltipPosition::Right
        } else {
            TooltipPosition::Left
        };

        self.settings
            .view_flyout(tooltip_position)
            .map(|e| e.map(Message::Settings))
    }

    /// Y offset for the flyout, relative to the top of the window.
    /// Accounts for header, padding, and the position of the group button.
    pub fn flyout_y_offset(&self) -> f32 {
        let group = match self.drawing_tools.expanded_group {
            Some(g) => g,
            None => return 0.0,
        };

        // Header height (macOS title bar or 0)
        let header_h: f32 = if cfg!(target_os = "macos") {
            20.0 + tokens::spacing::XS
        } else {
            0.0
        };

        // Row padding (the MD padding on the row that contains sidebar)
        let row_pad = tokens::spacing::MD;

        // Buttons above this group button:
        // Drawing tools start at the top of the sidebar
        let group_idx = SidebarGroup::ALL
            .iter()
            .position(|g| *g == group)
            .unwrap_or(0) as u32;
        let buttons_above = group_idx;

        // Each button slot: button natural height + column spacing (XS)
        // Sidebar buttons are ~32px (SIDEBAR_WIDTH) tall
        let button_h = tokens::layout::SIDEBAR_WIDTH;
        let slot = button_h + tokens::spacing::XS;

        header_h + row_pad + (buttons_above as f32 * slot)
    }

    pub fn is_menu_active(&self, menu: sidebar::Menu) -> bool {
        self.state.active_menu == Some(menu)
    }

    pub fn active_menu(&self) -> Option<sidebar::Menu> {
        self.state.active_menu
    }

    pub fn position(&self) -> sidebar::Position {
        self.state.position
    }

    pub fn set_menu(&mut self, menu: Option<sidebar::Menu>) {
        self.state.active_menu = menu;
    }
}
