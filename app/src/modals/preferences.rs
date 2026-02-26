//! Settings panel — flyout + modal pattern
//!
//! A category flyout opens from the sidebar cog button, then selecting
//! a category opens a centered modal with draft-based Save/Cancel.

use crate::components::display::tooltip::button_with_tooltip;
use crate::components::form::form_field::FormFieldBuilder;
use crate::components::form::form_section::FormSectionBuilder;
use crate::components::primitives::{Icon, icon_text};
use crate::config::sidebar;
use crate::config::{ScaleFactor, Theme, UserTimezone};
use crate::style;
use crate::style::tokens;

use iced::{
    Alignment, Element,
    widget::{
        button, column, container, pick_list, row, text, tooltip::Position as TooltipPosition,
    },
};

// ── Types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPage {
    General,
    Appearance,
}

impl SettingsPage {
    pub fn label(&self) -> &'static str {
        match self {
            SettingsPage::General => "General",
            SettingsPage::Appearance => "Appearance",
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    ToggleFlyout(bool),
    OpenPage(SettingsPage),
    CloseModal,
    SaveModal,
    // Draft mutations
    SetTimezone(UserTimezone),
    SetDateRangePreset(sidebar::DateRangePreset),
    SetTheme(Theme),
    ScaleFactorChanged(ScaleFactor),
    // Immediate actions
    OpenThemeEditor,
    OpenDataFolder,
    OpenDataManagement,
}

#[derive(Debug, Clone)]
pub enum Action {
    FlyoutToggled,
    OpenModal(SettingsPage),
    CloseModal,
    SaveSettings(SettingsDraft),
    OpenThemeEditor,
    OpenDataFolder,
    OpenDataManagement,
}

#[derive(Debug, Clone)]
pub struct SettingsDraft {
    pub timezone: UserTimezone,
    pub date_range_preset: sidebar::DateRangePreset,
    pub theme: Theme,
    pub scale_factor: ScaleFactor,
    /// Snapshot of the custom iced theme for pick_list display.
    pub custom_iced_theme: Option<iced::Theme>,
}

// ── Panel state ──────────────────────────────────────────────────────

pub struct SettingsPanel {
    pub flyout_expanded: bool,
    pub active_modal: Option<(SettingsPage, SettingsDraft)>,
}

impl SettingsPanel {
    pub fn new() -> Self {
        Self {
            flyout_expanded: false,
            active_modal: None,
        }
    }

    pub fn create_draft(
        timezone: UserTimezone,
        date_range_preset: sidebar::DateRangePreset,
        theme: Theme,
        scale_factor: ScaleFactor,
        custom_iced_theme: Option<iced::Theme>,
    ) -> SettingsDraft {
        SettingsDraft {
            timezone,
            date_range_preset,
            theme,
            scale_factor,
            custom_iced_theme,
        }
    }

    // ── Update ───────────────────────────────────────────────────────

    pub fn update(&mut self, msg: Message) -> Option<Action> {
        match msg {
            Message::ToggleFlyout(expanded) => {
                self.flyout_expanded = expanded;
                Some(Action::FlyoutToggled)
            }
            Message::OpenPage(page) => {
                self.flyout_expanded = false;
                Some(Action::OpenModal(page))
            }
            Message::CloseModal => {
                self.active_modal = None;
                Some(Action::CloseModal)
            }
            Message::SaveModal => {
                if let Some((_, draft)) = self.active_modal.take() {
                    Some(Action::SaveSettings(draft))
                } else {
                    None
                }
            }
            // Draft mutations — update in place, no action bubbled.
            Message::SetTimezone(tz) => {
                if let Some((_, ref mut draft)) = self.active_modal {
                    draft.timezone = tz;
                }
                None
            }
            Message::SetDateRangePreset(preset) => {
                if let Some((_, ref mut draft)) = self.active_modal {
                    draft.date_range_preset = preset;
                }
                None
            }
            Message::SetTheme(theme) => {
                if let Some((_, ref mut draft)) = self.active_modal {
                    draft.theme = theme;
                }
                None
            }
            Message::ScaleFactorChanged(factor) => {
                if let Some((_, ref mut draft)) = self.active_modal {
                    draft.scale_factor = factor;
                }
                None
            }
            // Immediate actions
            Message::OpenThemeEditor => Some(Action::OpenThemeEditor),
            Message::OpenDataFolder => Some(Action::OpenDataFolder),
            Message::OpenDataManagement => {
                self.flyout_expanded = false;
                Some(Action::OpenDataManagement)
            }
        }
    }

    // ── Views ────────────────────────────────────────────────────────

    /// Flyout listing settings pages as square icon buttons with tooltips.
    pub fn view_flyout(&self, tooltip_position: TooltipPosition) -> Option<Element<'_, Message>> {
        if !self.flyout_expanded {
            return None;
        }

        let general_btn = button_with_tooltip(
            icon_text(Icon::Cog, 14)
                .width(24)
                .height(24)
                .align_x(Alignment::Center),
            Message::OpenPage(SettingsPage::General),
            Some("General"),
            tooltip_position,
            |theme, status| style::button::transparent(theme, status, false),
        );

        let appearance_btn = button_with_tooltip(
            icon_text(Icon::Edit, 14)
                .width(24)
                .height(24)
                .align_x(Alignment::Center),
            Message::OpenPage(SettingsPage::Appearance),
            Some("Appearance"),
            tooltip_position,
            |theme, status| style::button::transparent(theme, status, false),
        );

        let data_btn = button_with_tooltip(
            icon_text(Icon::Folder, 14)
                .width(24)
                .height(24)
                .align_x(Alignment::Center),
            Message::OpenDataManagement,
            Some("Data"),
            tooltip_position,
            |theme, status| style::button::transparent(theme, status, false),
        );

        let col = column![general_btn, appearance_btn, data_btn]
            .spacing(tokens::spacing::XS)
            .width(tokens::layout::SIDEBAR_WIDTH)
            .align_x(Alignment::Center);

        let panel = container(col)
            .padding(tokens::spacing::XS)
            .style(style::floating_panel);

        Some(panel.into())
    }

    /// Body content for the active modal page (if any).
    pub fn view_modal_body(&self) -> Option<(SettingsPage, Element<'_, Message>)> {
        let (page, draft) = self.active_modal.as_ref()?;
        let body = match page {
            SettingsPage::General => Self::view_general_page(draft),
            SettingsPage::Appearance => Self::view_appearance_page(draft),
        };
        Some((*page, body))
    }

    fn view_general_page(draft: &SettingsDraft) -> Element<'_, Message> {
        let date_range_field = FormFieldBuilder::new(
            "Date range",
            pick_list(
                sidebar::DateRangePreset::ALL,
                Some(draft.date_range_preset),
                Message::SetDateRangePreset,
            ),
        );

        let open_folder_btn = button(
            row![icon_text(Icon::Folder, 14), text("Open data folder"),]
                .spacing(tokens::spacing::SM)
                .align_y(Alignment::Center),
        )
        .on_press(Message::OpenDataFolder)
        .style(|theme, status| style::button::transparent(theme, status, false));

        FormSectionBuilder::new("")
            .push(date_range_field)
            .push(open_folder_btn)
            .into_element()
    }

    fn view_appearance_page(draft: &SettingsDraft) -> Element<'_, Message> {
        // Timezone
        let timezone_field = FormFieldBuilder::new(
            "Time zone",
            pick_list(
                [UserTimezone::Utc, UserTimezone::Local],
                Some(draft.timezone),
                Message::SetTimezone,
            ),
        );

        // Theme
        let theme_field = {
            let mut themes: Vec<iced::Theme> = iced_core::Theme::ALL.to_vec();
            themes.push(crate::style::theme::default_iced_theme());
            if let Some(custom) = &draft.custom_iced_theme {
                themes.push(custom.clone());
            }
            let current_iced = crate::style::theme::theme_to_iced(&draft.theme);

            FormFieldBuilder::new(
                "Theme",
                pick_list(themes, Some(current_iced), |theme| {
                    Message::SetTheme(crate::style::theme::iced_theme_to_data(theme))
                }),
            )
        };

        // Interface scale
        let scale_field = {
            let v: f32 = draft.scale_factor.into();

            let dec = if v > crate::config::scale_factor::MIN_SCALE {
                button(text("-")).on_press(Message::ScaleFactorChanged((v - 0.1).into()))
            } else {
                button(text("-"))
            };

            let inc = if v < crate::config::scale_factor::MAX_SCALE {
                button(text("+")).on_press(Message::ScaleFactorChanged((v + 0.1).into()))
            } else {
                button(text("+"))
            };

            let scale_control = container(
                row![
                    dec,
                    text(format!("{:.0}%", v * 100.0)).size(tokens::text::TITLE),
                    inc,
                ]
                .align_y(Alignment::Center)
                .spacing(tokens::spacing::MD)
                .padding(tokens::spacing::XS),
            )
            .style(style::modal_container);

            FormFieldBuilder::new("Interface scale", scale_control)
        };

        // Theme editor button (immediate action)
        let theme_editor_btn = button(text("Theme editor"))
            .on_press(Message::OpenThemeEditor)
            .style(|theme, status| style::button::transparent(theme, status, false));

        FormSectionBuilder::new("")
            .push(timezone_field)
            .push(theme_field)
            .push(scale_field)
            .push(theme_editor_btn)
            .into_element()
    }
}
