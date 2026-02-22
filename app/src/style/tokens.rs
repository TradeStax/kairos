//! Design tokens -- single source of truth for all visual constants.
//! All view code should reference these instead of magic numbers.
//!
//! Some constants are defined for completeness and may not yet be referenced.

// ── Spacing (4px base grid) ───────────────────────────────────────────
pub mod spacing {
    pub const XXXS: f32 = 1.0; // Hairline (dividers)
    pub const XXS: f32 = 2.0; // Tight (drag margins)
    pub const XS: f32 = 4.0; // Compact (icon padding, tight rows)
    pub const SM: f32 = 6.0; // Small (button internal padding)
    pub const MD: f32 = 8.0; // Default (row spacing, section gaps)
    pub const LG: f32 = 12.0; // Comfortable (form field spacing)
    pub const XL: f32 = 16.0; // Generous (card padding)
    pub const XXL: f32 = 24.0; // Spacious (modal padding, section breaks)
}

// ── Typography ────────────────────────────────────────────────────────
pub mod text {
    pub const TINY: f32 = 10.0; // Badges, labels
    pub const SMALL: f32 = 11.0; // Chart labels, panel data (AZERET_MONO)
    pub const BODY: f32 = 12.0; // Default UI text
    pub const LABEL: f32 = 13.0; // Form labels, section headers
    pub const TITLE: f32 = 14.0; // Dialog titles, prominent text
    pub const HEADING: f32 = 16.0; // Modal headings
}

// ── Border Radii ──────────────────────────────────────────────────────
pub mod radius {
    pub const NONE: f32 = 0.0;
    pub const SM: f32 = 2.0; // Inputs, scrollbars
    pub const MD: f32 = 4.0; // Buttons, containers, modals (default)
    pub const LG: f32 = 6.0; // Emphasized panels
    pub const ROUND: f32 = 16.0; // Circles, pills
}

// ── Border Widths ─────────────────────────────────────────────────────
pub mod border {
    pub const NONE: f32 = 0.0;
    pub const THIN: f32 = 1.0; // Standard borders
    pub const MEDIUM: f32 = 1.5; // Emphasized (confirm modals)
    pub const THICK: f32 = 2.0; // Active state, scrollbars
}

// ── Shadows ───────────────────────────────────────────────────────────
pub mod shadow {
    pub const NONE: f32 = 0.0;
    pub const SM: f32 = 2.0; // Minimal (modal containers)
    pub const MD: f32 = 4.0; // Subtle (drag rows)
    pub const LG: f32 = 8.0; // Dropdowns
    pub const XL: f32 = 12.0; // Chart modals, confirm dialogs
    pub const XXL: f32 = 20.0; // Dashboard modals (deepest)
}

// ── Layout Constants ──────────────────────────────────────────────────
pub mod layout {
    pub const TITLE_BAR_HEIGHT: f32 = 32.0;
    pub const MENU_BAR_HEIGHT: f32 = 24.0;
    pub const SIDEBAR_WIDTH: f32 = 32.0;
    pub const SIDEBAR_BUTTON_HEIGHT: f32 = 34.0;
    pub const PANEL_ROW_HEIGHT: f32 = 16.0; // Ladder
    pub const PANEL_ROW_HEIGHT_SM: f32 = 14.0; // TimeAndSales
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
}

// ── Chart Rendering ───────────────────────────────────────────────────
pub mod chart {
    pub const Y_AXIS_GUTTER: f32 = 66.0;
    pub const X_AXIS_HEIGHT: f32 = 24.0;
    pub const MIN_X_TICK_PX: f32 = 80.0;
    pub const ZOOM_SENSITIVITY: f32 = 30.0;
    pub const ZOOM_BASE: f32 = 2.0;
    pub const ZOOM_STEP_PCT: f32 = 0.05;
    pub const GAP_BREAK_MULTIPLIER: f32 = 3.0;
    pub const EMPTY_STATE_ICON_SIZE: f32 = 32.0;

    pub mod ruler {
        /// Fill alpha for the ruler rectangle
        pub const FILL_ALPHA: f32 = 0.08;
        /// Padding around ruler text
        pub const TEXT_PADDING: f32 = 8.0;
        /// Background padding for ruler label
        pub const RECT_PADDING: f32 = 4.0;
    }
}

// ── Form Layout ─────────────────────────────────────────────────────
pub mod form {
    pub const LABEL_WIDTH: f32 = 120.0;
    pub const LABEL_WIDTH_NARROW: f32 = 80.0;
}

// ── Replay Modal Layout ──────────────────────────────────────────────
pub mod replay_layout {
    /// Y offset for stream picker popup below its trigger button.
    pub const STREAM_POPUP_Y: f32 = 90.0;
    /// Y offset for date/time picker popup below its trigger button.
    pub const DATETIME_POPUP_Y: f32 = 148.0;
    /// Square calendar day cell size.
    pub const CALENDAR_CELL: f32 = 26.0;
}

// ── Debug Table Columns ──────────────────────────────────────────────
pub mod debug_table {
    pub const COL_TIME: f32 = 100.0;
    pub const COL_SIDE: f32 = 40.0;
    pub const COL_QTY: f32 = 50.0;
    pub const COL_VWAP: f32 = 80.0;
    pub const COL_FILLS: f32 = 40.0;
    pub const COL_WINDOW: f32 = 65.0;
    pub const COL_RANGE: f32 = 130.0;
}

// ── Ticker Panel ─────────────────────────────────────────────────────
pub mod ticker_panel {
    pub const COMPACT_ROW_HEIGHT: f32 = 28.0;
}

// ── Alpha Scale ───────────────────────────────────────────────────────
pub mod alpha {
    pub const FAINT: f32 = 0.2; // Disabled, dark-theme hints
    pub const SUBTLE: f32 = 0.3; // Faint backgrounds
    pub const LIGHT: f32 = 0.4; // Cards, weak shadows
    pub const MEDIUM: f32 = 0.5; // Pane grids
    pub const STRONG: f32 = 0.6; // Mid-tone backgrounds
    pub const HEAVY: f32 = 0.8; // Heavy shadows, dashed lines
    pub const BACKDROP: f32 = 0.8; // Modal backdrop overlay
    pub const OPAQUE: f32 = 0.99; // Modal backgrounds (near-opaque)
}
