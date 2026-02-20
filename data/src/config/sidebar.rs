//! Sidebar configuration and state management
//!
//! Manages the sidebar's visibility, width, content selection, and persistence
//! across application sessions.

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct Sidebar {
    pub position: Position,
    #[serde(default)]
    pub date_range_preset: DateRangePreset,
    #[serde(skip)]
    pub active_menu: Option<Menu>,
}

impl Sidebar {
    pub fn set_menu(&mut self, new_menu: Menu) {
        self.active_menu = Some(new_menu);
    }

    pub fn set_position(&mut self, position: Position) {
        self.position = position;
    }

    pub fn set_date_range_preset(&mut self, preset: DateRangePreset) {
        self.date_range_preset = preset;
    }

    pub fn is_menu_active(&self, menu: Menu) -> bool {
        self.active_menu == Some(menu)
    }
}

impl Default for Sidebar {
    fn default() -> Self {
        Sidebar {
            position: Position::Left,
            date_range_preset: DateRangePreset::default(),
            active_menu: None,
        }
    }
}

pub fn deserialize_sidebar_fallback<'de, D>(deserializer: D) -> Result<Sidebar, D::Error>
where
    D: Deserializer<'de>,
{
    Sidebar::deserialize(deserializer).or(Ok(Sidebar::default()))
}

#[derive(Default, Debug, Clone, PartialEq, Copy, Deserialize, Serialize)]
pub enum Position {
    #[default]
    Left,
    Right,
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Position::Left => write!(f, "Left"),
            Position::Right => write!(f, "Right"),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Deserialize, Serialize)]
pub enum Menu {
    Layout,
    Connections,
    #[serde(alias = "DataManagement")]
    DataFeeds,
    Settings,
    ThemeEditor,
    Replay,
}

/// Date range preset for controlling how much historical data is loaded when opening charts.
#[derive(Default, Debug, Clone, PartialEq, Copy, Deserialize, Serialize)]
pub enum DateRangePreset {
    #[default]
    Day1,
    Days2,
    Week1,
    Weeks2,
    Month1,
}

impl DateRangePreset {
    /// Convert the preset to a number of days
    pub fn to_days(&self) -> i64 {
        match self {
            Self::Day1 => 1,
            Self::Days2 => 2,
            Self::Week1 => 7,
            Self::Weeks2 => 14,
            Self::Month1 => 30,
        }
    }

    /// Get all available presets for UI display
    pub const ALL: [DateRangePreset; 5] = [
        DateRangePreset::Day1,
        DateRangePreset::Days2,
        DateRangePreset::Week1,
        DateRangePreset::Weeks2,
        DateRangePreset::Month1,
    ];
}

impl std::fmt::Display for DateRangePreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Day1 => write!(f, "1 day"),
            Self::Days2 => write!(f, "2 days"),
            Self::Week1 => write!(f, "1 week"),
            Self::Weeks2 => write!(f, "2 weeks"),
            Self::Month1 => write!(f, "1 month"),
        }
    }
}
