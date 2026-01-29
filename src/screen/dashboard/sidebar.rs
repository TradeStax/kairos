//! Sidebar for navigation and menu access
//!
//! Provides navigation buttons for layout management, audio controls, data management,
//! and settings. Ticker selection is handled via pane dropdowns (MiniTickersList modal).

use crate::{
    layout::SavedState,
    style::{Icon, icon_text},
    widget::button_with_tooltip,
};
use iced::widget::tooltip::Position as TooltipPosition;
use data::sidebar;

use iced::{
    Alignment, Element, Task,
    widget::{column, space},
};

#[derive(Debug, Clone)]
pub enum Message {
    ToggleSidebarMenu(Option<sidebar::Menu>),
    SetDateRangePreset(sidebar::DateRangePreset),
}

pub struct Sidebar {
    pub state: data::Sidebar,
}

impl Sidebar {
    pub fn new(state: &SavedState) -> Self {
        Self {
            state: state.sidebar.clone(),
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ToggleSidebarMenu(menu) => {
                self.set_menu(menu.filter(|&m| !self.is_menu_active(m)));
            }
            Message::SetDateRangePreset(preset) => {
                self.state.date_range_preset = preset;
            }
        }

        Task::none()
    }

    pub fn view(&self, audio_volume: Option<f32>) -> Element<'_, Message> {
        let state = &self.state;

        let tooltip_position = if state.position == sidebar::Position::Left {
            TooltipPosition::Right
        } else {
            TooltipPosition::Left
        };

        let nav_buttons = self.nav_buttons(audio_volume, tooltip_position);

        nav_buttons.into()
    }

    fn nav_buttons(
        &self,
        audio_volume: Option<f32>,
        tooltip_position: TooltipPosition,
    ) -> iced::widget::Column<'_, Message> {
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

        let data_mgmt_button = {
            let is_active = self.is_menu_active(sidebar::Menu::DataManagement);

            button_with_tooltip(
                icon_text(Icon::Folder, 14)
                    .width(24)
                    .align_x(Alignment::Center),
                Message::ToggleSidebarMenu(Some(sidebar::Menu::DataManagement)),
                Some("Data Management"),
                tooltip_position,
                move |theme, status| crate::style::button::transparent(theme, status, is_active),
            )
        };

        column![
            layout_modal_button,
            audio_btn,
            space::vertical(),
            data_mgmt_button,
            settings_modal_button,
        ]
        .width(32)
        .spacing(8)
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
