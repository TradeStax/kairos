//! UI chrome state: sidebar, theme, scale factor, timezone, AI prefs, notifications, confirm dialog.

use crate::components::display::toast::Toast;
use crate::screen::dashboard;

use super::super::Message;

pub(crate) struct UiState {
    pub(crate) sidebar: dashboard::Sidebar,
    pub(crate) title_bar_hovered: bool,
    pub(crate) theme: data::Theme,
    pub(crate) ui_scale_factor: data::ScaleFactor,
    pub(crate) timezone: data::UserTimezone,
    pub(crate) ai_preferences: data::AiPreferences,
    pub(crate) notifications: Vec<Toast>,
    pub(crate) confirm_dialog:
        Option<crate::components::overlay::confirm_dialog::ConfirmDialog<Message>>,
}
