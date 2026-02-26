//! Calendar colors.

use iced::Color;

/// Text for days outside the current month.
pub const OTHER_MONTH_TEXT: Color = Color::from_rgba(0.5, 0.5, 0.5, 0.3);
/// Text for non-cached days in the current month.
pub const UNCACHED_TEXT: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.5);
/// Text for cached days in the current month.
pub const CACHED_TEXT: Color = Color::from_rgba(1.0, 1.0, 1.0, 1.0);
/// Background for cached day cells.
pub const CACHED_BG: Color = Color::from_rgba(0.5, 0.5, 0.5, 0.2);
