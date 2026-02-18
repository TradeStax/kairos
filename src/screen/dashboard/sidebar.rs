//! Sidebar for navigation and menu access
//!
//! Provides navigation buttons for layout management, audio controls, data management,
//! and settings. Drawing tools are displayed in the center with category-based selection.

use crate::{
    layout::SavedState,
    modal::drawing_tools::{self, DrawingToolsPanel, ToolCategory},
    component::primitives::{Icon, icon_text},
    style,
    style::tokens,
    component::display::tooltip::button_with_tooltip,
};
use iced::widget::tooltip::Position as TooltipPosition;
use data::sidebar;

use iced::{
    Alignment, Element, Length, Task,
    widget::{column, container, mouse_area, space, stack, Space},
    padding,
};

#[derive(Debug, Clone)]
pub enum Message {
    ToggleSidebarMenu(Option<sidebar::Menu>),
    SetDateRangePreset(sidebar::DateRangePreset),
    DrawingTools(drawing_tools::Message),
}

pub struct Sidebar {
    pub state: data::Sidebar,
    pub drawing_tools: DrawingToolsPanel,
}

impl Sidebar {
    pub fn new(state: &SavedState) -> Self {
        Self {
            state: state.sidebar.clone(),
            drawing_tools: DrawingToolsPanel::new(),
        }
    }

    pub fn update(&mut self, message: Message) -> (Task<Message>, Option<drawing_tools::Action>) {
        match message {
            Message::ToggleSidebarMenu(menu) => {
                self.set_menu(menu.filter(|&m| !self.is_menu_active(m)));
                // Close drawing tools dropdown when opening other menus
                if menu.is_some() {
                    self.drawing_tools.update(drawing_tools::Message::CloseDropdown);
                }
                (Task::none(), None)
            }
            Message::SetDateRangePreset(preset) => {
                self.state.date_range_preset = preset;
                (Task::none(), None)
            }
            Message::DrawingTools(msg) => {
                let action = self.drawing_tools.update(msg);
                (Task::none(), action)
            }
        }
    }

    pub fn view(&self, audio_volume: Option<f32>) -> Element<'_, Message> {
        let state = &self.state;

        let tooltip_position = if state.position == sidebar::Position::Left {
            TooltipPosition::Right
        } else {
            TooltipPosition::Left
        };

        // Build sidebar content
        let sidebar_content = self.build_sidebar_content(audio_volume, tooltip_position);

        // If a drawing tools dropdown is open, overlay it
        if let Some(dropdown) = self.drawing_tools.view_dropdown() {
            self.view_with_dropdown(sidebar_content, dropdown)
        } else {
            sidebar_content
        }
    }

    fn build_sidebar_content(
        &self,
        audio_volume: Option<f32>,
        tooltip_position: TooltipPosition,
    ) -> Element<'_, Message> {
        // Top buttons
        let layout_modal_button = {
            let is_active = self.is_menu_active(sidebar::Menu::Layout);

            button_with_tooltip(
                icon_text(Icon::Layout, 14)
                    .width(24)
                    .align_x(Alignment::Center),
                Message::ToggleSidebarMenu(Some(sidebar::Menu::Layout)),
                None,
                tooltip_position,
                move |theme, status| crate::style::button::transparent(theme, status, is_active),
            )
        };

        // Drawing tools section - centered with subtle container
        let drawing_tools_section = {
            let tools_buttons = self.drawing_tools
                .view_sidebar_buttons()
                .map(Message::DrawingTools);

            container(tools_buttons)
                .padding(padding::all(tokens::spacing::XS))
                .style(style::drawing_tools_container)
        };

        // Bottom buttons
        let audio_btn = {
            let is_active = self.is_menu_active(sidebar::Menu::Audio);

            let icon = match audio_volume.unwrap_or(0.0) {
                v if v >= 40.0 => Icon::SpeakerHigh,
                v if v > 0.0 => Icon::SpeakerLow,
                _ => Icon::SpeakerOff,
            };

            button_with_tooltip(
                icon_text(icon, 14).width(24).align_x(Alignment::Center),
                Message::ToggleSidebarMenu(Some(sidebar::Menu::Audio)),
                None,
                tooltip_position,
                move |theme, status| crate::style::button::transparent(theme, status, is_active),
            )
        };

        let connections_button = {
            let is_active = self.is_menu_active(sidebar::Menu::Connections);

            button_with_tooltip(
                icon_text(Icon::Link, 14)
                    .width(24)
                    .align_x(Alignment::Center),
                Message::ToggleSidebarMenu(Some(sidebar::Menu::Connections)),
                Some("Connections"),
                tooltip_position,
                move |theme, status| crate::style::button::transparent(theme, status, is_active),
            )
        };

        let settings_modal_button = {
            let is_active = self.is_menu_active(sidebar::Menu::Settings)
                || self.is_menu_active(sidebar::Menu::ThemeEditor);

            button_with_tooltip(
                icon_text(Icon::Cog, 14)
                    .width(24)
                    .align_x(Alignment::Center),
                Message::ToggleSidebarMenu(Some(sidebar::Menu::Settings)),
                None,
                tooltip_position,
                move |theme, status| crate::style::button::transparent(theme, status, is_active),
            )
        };

        // Layout: top, center (drawing tools), bottom
        column![
            // Top section
            layout_modal_button,
            // Spacer to push drawing tools to center
            space::vertical().height(Length::Fill),
            // Center section - drawing tools
            drawing_tools_section,
            // Spacer to push bottom buttons down
            space::vertical().height(Length::Fill),
            // Bottom section
            audio_btn,
            connections_button,
            settings_modal_button,
        ]
        .width(tokens::layout::SIDEBAR_WIDTH)
        .spacing(tokens::spacing::XS)
        .align_x(Alignment::Center)
        .into()
    }

    fn view_with_dropdown<'a>(
        &'a self,
        sidebar_content: Element<'a, Message>,
        dropdown: Element<'a, drawing_tools::Message>,
    ) -> Element<'a, Message> {
        // Calculate dropdown position based on which category is open
        let dropdown_offset = self.calculate_dropdown_offset();

        // Create the positioned dropdown
        let positioned_dropdown = container(
            dropdown.map(Message::DrawingTools)
        )
        .padding(padding::left(36).top(dropdown_offset));

        // Create a mouse area that covers the whole screen to close dropdown on outside click
        let close_overlay = mouse_area(
            container(Space::new().width(Length::Fill).height(Length::Fill))
        )
        .on_press(Message::DrawingTools(drawing_tools::Message::CloseDropdown));

        // Stack: base sidebar, overlay for closing, dropdown
        stack![
            sidebar_content,
            close_overlay,
            positioned_dropdown,
        ]
        .into()
    }

    fn calculate_dropdown_offset(&self) -> f32 {
        const DRAWING_TOOLS_BASE_OFFSET: f32 = 120.0;
        const TOOL_BUTTON_HEIGHT: f32 = 34.0; // 32px button + 2px spacing

        if let Some(category) = self.drawing_tools.open_category() {
            let category_index = ToolCategory::ALL
                .iter()
                .position(|c| *c == category)
                .unwrap_or(0);

            // Each button is approximately 32px tall with 2px spacing
            base_offset + (category_index as f32 * tokens::layout::SIDEBAR_BUTTON_HEIGHT)
        } else {
            DRAWING_TOOLS_BASE_OFFSET
        }
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

    pub fn date_range_preset(&self) -> sidebar::DateRangePreset {
        self.state.date_range_preset
    }

}
