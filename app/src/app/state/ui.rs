//! UI chrome state: sidebar, theme, scale factor, timezone, AI prefs, notifications, confirm dialog.

use crate::components::display::toast::Toast;
use crate::screen::dashboard;

use super::super::Message;

/// Maximum number of toast notifications retained in memory.
/// Oldest notifications are drained when this limit is exceeded.
const MAX_NOTIFICATIONS: usize = 100;

pub(crate) struct UiState {
    pub(crate) sidebar: dashboard::Sidebar,
    pub(crate) title_bar_hovered: bool,
    pub(crate) theme: crate::config::Theme,
    pub(crate) ui_scale_factor: crate::config::ScaleFactor,
    pub(crate) timezone: crate::config::UserTimezone,
    pub(crate) ai_preferences: crate::persistence::AiPreferences,
    pub(crate) notifications: Vec<Toast>,
    pub(crate) confirm_dialog:
        Option<crate::components::overlay::confirm_dialog::ConfirmDialog<Message>>,
}

impl UiState {
    /// Push a toast notification, capping the vector at [`MAX_NOTIFICATIONS`]
    /// by draining the oldest entries when the limit is exceeded.
    pub(crate) fn push_notification(&mut self, toast: Toast) {
        self.notifications.push(toast);
        if self.notifications.len() > MAX_NOTIFICATIONS {
            let excess = self.notifications.len() - MAX_NOTIFICATIONS;
            self.notifications.drain(..excess);
        }
    }
}
