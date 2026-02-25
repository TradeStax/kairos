//! Component-specific sizes (icons, forms, status, scrollbar, buttons, panels).

pub mod icon {
    pub const SM: f32 = 12.0; // Search, inline icons
    pub const MD: f32 = 14.0; // Toolbar icons
    pub const LG: f32 = 16.0; // Default icon buttons
    pub const XL: f32 = 20.0; // Prominent icons
    pub const EMPTY_STATE: f32 = 32.0; // Empty state illustrations
}

pub mod status_dot {
    pub const SIZE: f32 = 8.0; // Status indicator dot
}

pub mod scrollbar {
    pub const WIDTH: f32 = 4.0;
    pub const SCROLLER_WIDTH: f32 = 4.0;
    pub const SPACING: f32 = 2.0;
}

pub mod button {
    pub const COMPACT_PADDING: [f32; 2] = [4.0, 10.0]; // Pane control buttons
    pub const WINDOW_CONTROL_WIDTH: f32 = 46.0; // Title bar buttons
    pub const LINK_GROUP_WIDTH: f32 = 28.0;
}

/// Form layout constants.
pub mod form {
    pub const LABEL_WIDTH: f32 = 120.0;
    pub const LABEL_WIDTH_NARROW: f32 = 80.0;
}

/// Debug table column widths.
pub mod debug_table {
    pub const COL_TIME: f32 = 100.0;
    pub const COL_SIDE: f32 = 40.0;
    pub const COL_QTY: f32 = 50.0;
    pub const COL_VWAP: f32 = 80.0;
    pub const COL_FILLS: f32 = 40.0;
    pub const COL_WINDOW: f32 = 65.0;
    pub const COL_RANGE: f32 = 130.0;
}

/// Ticker panel dimensions.
pub mod ticker_panel {
    pub const COMPACT_ROW_HEIGHT: f32 = 28.0;
}

/// Replay modal layout constants.
pub mod replay {
    /// Y offset for stream picker popup below its trigger button.
    pub const STREAM_POPUP_Y: f32 = 90.0;
    /// Y offset for date/time picker popup below its trigger button.
    pub const DATETIME_POPUP_Y: f32 = 148.0;
    /// Square calendar day cell size.
    pub const CALENDAR_CELL: f32 = 26.0;
}
