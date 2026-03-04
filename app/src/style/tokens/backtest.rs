//! Backtest chart colors.

use iced::Color;

pub const EQUITY_LINE: Color = Color::from_rgba(0.3, 0.7, 0.9, 1.0);
pub const DRAWDOWN_FILL: Color = Color::from_rgba(0.8, 0.2, 0.2, 0.20);
pub const SELECTED_FILL: Color = Color::from_rgba(1.0, 0.9, 0.2, 0.15);
pub const WIN_ROW_BG: Color = Color::from_rgba(0.2, 0.7, 0.2, 0.07);
pub const LOSS_ROW_BG: Color = Color::from_rgba(0.7, 0.2, 0.2, 0.07);

// Management modal chart tokens
pub const MONTE_CARLO_PATH: Color = Color::from_rgba(0.3, 0.6, 1.0, 0.15);
pub const MONTE_CARLO_BAND: Color = Color::from_rgba(0.3, 0.6, 1.0, 0.15);
pub const MONTE_CARLO_MEDIAN: Color = Color::from_rgba(0.3, 0.6, 1.0, 0.8);
pub const POSITIVE_RETURN: Color = Color::from_rgba(0.2, 0.7, 0.3, 0.8);
pub const NEGATIVE_RETURN: Color = Color::from_rgba(0.7, 0.2, 0.2, 0.8);
pub const SCATTER_WIN: Color = Color::from_rgba(0.3, 0.8, 0.3, 0.6);
pub const SCATTER_LOSS: Color = Color::from_rgba(0.8, 0.3, 0.3, 0.6);
pub const GRID_LINE: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.06);
pub const AXIS_TEXT: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.4);

// Prop firm simulation
pub const PROP_FIRM_TARGET: Color = Color::from_rgba(0.3, 0.8, 0.3, 0.6);
pub const PROP_FIRM_LIMIT: Color = Color::from_rgba(0.8, 0.2, 0.2, 0.6);
pub const PROP_FIRM_DAILY: Color = Color::from_rgba(0.9, 0.6, 0.2, 0.5);
pub const PROP_FIRM_ACTIVE: Color = Color::from_rgba(0.3, 0.6, 1.0, 0.8);
pub const PROP_FIRM_PROGRESS_TRACK: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.06);
pub const PROP_FIRM_PROGRESS_FILL: Color = Color::from_rgba(0.3, 0.6, 1.0, 0.7);
pub const PROP_FIRM_PROGRESS_COMPLETE: Color = Color::from_rgba(0.3, 0.8, 0.3, 0.7);

// Table styling
pub const TABLE_HEADER_BG: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.03);
pub const TABLE_ROW_ALT: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.015);
pub const TABLE_ROW_HOVER: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.05);

// Interactive chart hover tokens
pub const CROSSHAIR_LINE: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.25);
pub const HOVER_HIGHLIGHT: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.12);
pub const SNAP_DOT: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.9);

// Trade detail chart tokens
pub const ENTRY_MARKER: Color = Color::from_rgba(0.2, 0.7, 0.9, 0.9);
pub const EXIT_MARKER_WIN: Color = Color::from_rgba(0.3, 0.8, 0.3, 0.9);
pub const EXIT_MARKER_LOSS: Color = Color::from_rgba(0.8, 0.3, 0.3, 0.9);
pub const STOP_LOSS_LINE: Color = Color::from_rgba(0.8, 0.2, 0.2, 0.6);
pub const TAKE_PROFIT_LINE: Color = Color::from_rgba(0.2, 0.7, 0.3, 0.6);
pub const MAE_BAND: Color = Color::from_rgba(0.8, 0.2, 0.2, 0.08);
pub const MFE_BAND: Color = Color::from_rgba(0.2, 0.7, 0.3, 0.08);
pub const STRATEGY_OVERLAY: Color = Color::from_rgba(0.4, 0.6, 1.0, 0.15);
pub const STRATEGY_OVERLAY_LINE: Color = Color::from_rgba(0.4, 0.6, 1.0, 0.5);
