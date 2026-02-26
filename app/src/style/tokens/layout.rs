//! App shell dimensions (title bar, sidebar, modal widths, etc.).

pub const TITLE_BAR_HEIGHT: f32 = 32.0;
pub const MENU_BAR_HEIGHT: f32 = 24.0;
pub const SIDEBAR_WIDTH: f32 = 32.0;
pub const SIDEBAR_BUTTON_HEIGHT: f32 = 34.0;
pub const PANEL_ROW_HEIGHT: f32 = 16.0; // Ladder
pub const MODAL_MAX_WIDTH: u32 = 650;

// Modal widths
pub const MODAL_WIDTH_SM: f32 = 220.0; // connections_menu
pub const MODAL_WIDTH_MD: f32 = 360.0; // data_management
pub const MODAL_WIDTH_LG: f32 = 420.0; // historical_download
pub const MODAL_WIDTH_XL: f32 = 880.0; // indicator_manager
pub const CONFIRM_DIALOG_WIDTH: f32 = 340.0;
pub const SCROLLBAR_WIDTH: f32 = 4.0;
pub const SLIDER_HEIGHT: f32 = 24.0;
pub const TOGGLER_SIZE: f32 = 18.0;
pub const TOAST_MAX_WIDTH: f32 = 200.0;

/// macOS title padding (window controls inset).
pub const TITLE_PADDING_TOP: f32 = if cfg!(target_os = "macos") { 20.0 } else { 0.0 };
