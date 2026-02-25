//! Profile chart configuration and enums.

use crate::util::serde_defaults;
use serde::{Deserialize, Serialize};

/// Profile line style — mirrors `LineStyleValue` in the study crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ProfileLineStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

impl ProfileLineStyle {
    pub const ALL: [ProfileLineStyle; 3] = [
        ProfileLineStyle::Solid,
        ProfileLineStyle::Dashed,
        ProfileLineStyle::Dotted,
    ];
}

impl std::fmt::Display for ProfileLineStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileLineStyle::Solid => write!(f, "Solid"),
            ProfileLineStyle::Dashed => write!(f, "Dashed"),
            ProfileLineStyle::Dotted => write!(f, "Dotted"),
        }
    }
}

/// Profile line extend direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ProfileExtendDirection {
    #[default]
    None,
    Left,
    Right,
    Both,
}

impl ProfileExtendDirection {
    pub const ALL: [ProfileExtendDirection; 4] = [
        ProfileExtendDirection::None,
        ProfileExtendDirection::Left,
        ProfileExtendDirection::Right,
        ProfileExtendDirection::Both,
    ];
}

impl std::fmt::Display for ProfileExtendDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileExtendDirection::None => write!(f, "None"),
            ProfileExtendDirection::Left => write!(f, "Left"),
            ProfileExtendDirection::Right => write!(f, "Right"),
            ProfileExtendDirection::Both => write!(f, "Both"),
        }
    }
}

/// Profile volume node detection method.
// TODO: unify with study::output::NodeDetectionMethod
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ProfileNodeDetectionMethod {
    Percentile,
    #[default]
    Relative,
    StdDev,
}

impl ProfileNodeDetectionMethod {
    pub const ALL: [ProfileNodeDetectionMethod; 3] = [
        ProfileNodeDetectionMethod::Percentile,
        ProfileNodeDetectionMethod::Relative,
        ProfileNodeDetectionMethod::StdDev,
    ];
}

impl std::fmt::Display for ProfileNodeDetectionMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileNodeDetectionMethod::Percentile => write!(f, "Percentile"),
            ProfileNodeDetectionMethod::Relative => write!(f, "Relative"),
            ProfileNodeDetectionMethod::StdDev => write!(f, "Std Dev"),
        }
    }
}

/// Profile chart display type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ProfileDisplayType {
    #[default]
    Volume,
    BidAskVolume,
    Delta,
    DeltaAndTotal,
    DeltaPercentage,
}

impl std::fmt::Display for ProfileDisplayType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileDisplayType::Volume => write!(f, "Volume"),
            ProfileDisplayType::BidAskVolume => write!(f, "Bid/Ask Volume"),
            ProfileDisplayType::Delta => write!(f, "Delta"),
            ProfileDisplayType::DeltaAndTotal => write!(f, "Delta & Total"),
            ProfileDisplayType::DeltaPercentage => write!(f, "Delta %"),
        }
    }
}

/// Profile chart split unit
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ProfileSplitUnit {
    #[default]
    Days,
    Hours,
    Minutes,
}

impl ProfileSplitUnit {
    pub const ALL: &'static [Self] = &[
        Self::Days,
        Self::Hours,
        Self::Minutes,
    ];
}

impl std::fmt::Display for ProfileSplitUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileSplitUnit::Days => write!(f, "Days"),
            ProfileSplitUnit::Hours => write!(f, "Hours"),
            ProfileSplitUnit::Minutes => write!(f, "Minutes"),
        }
    }
}

/// Profile chart visual configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    // Display
    #[serde(default)]
    pub display_type: ProfileDisplayType,

    // Split interval
    #[serde(default)]
    pub split_unit: ProfileSplitUnit,
    #[serde(default = "crate::util::serde_defaults::default_split_value")]
    pub split_value: i64,
    #[serde(default = "crate::util::serde_defaults::default_max_profiles")]
    pub max_profiles: i64,

    // Tick grouping
    #[serde(default = "crate::util::serde_defaults::default_true")]
    pub auto_grouping: bool,
    #[serde(default = "crate::util::serde_defaults::default_one")]
    pub auto_group_factor: i64,
    #[serde(default = "crate::util::serde_defaults::default_one")]
    pub manual_ticks: i64,

    // Value Area
    #[serde(default = "crate::util::serde_defaults::default_va_pct")]
    pub value_area_pct: f32,
    #[serde(default = "crate::util::serde_defaults::default_true")]
    pub show_va_highlight: bool,
    pub vah_color: Option<crate::config::color::Rgba>,
    pub val_color: Option<crate::config::color::Rgba>,

    // Value Area (expanded)
    #[serde(default = "crate::util::serde_defaults::default_true")]
    pub show_va_fill: bool,
    pub va_fill_color: Option<crate::config::color::Rgba>,
    #[serde(default = "crate::util::serde_defaults::default_va_fill_opacity")]
    pub va_fill_opacity: f32,
    #[serde(default = "crate::util::serde_defaults::default_line_width")]
    pub vah_line_width: f32,
    #[serde(default)]
    pub vah_line_style: ProfileLineStyle,
    #[serde(default = "crate::util::serde_defaults::default_line_width")]
    pub val_line_width: f32,
    #[serde(default)]
    pub val_line_style: ProfileLineStyle,
    #[serde(default)]
    pub va_extend: ProfileExtendDirection,
    #[serde(default)]
    pub show_va_labels: bool,

    // POC
    #[serde(default = "crate::util::serde_defaults::default_true")]
    pub show_poc: bool,
    pub poc_color: Option<crate::config::color::Rgba>,
    #[serde(default = "crate::util::serde_defaults::default_poc_width")]
    pub poc_line_width: f32,
    #[serde(default)]
    pub poc_line_style: ProfileLineStyle,
    #[serde(default)]
    pub poc_extend: ProfileExtendDirection,
    #[serde(default)]
    pub show_poc_label: bool,

    // Volume Nodes
    #[serde(default)]
    pub show_hvn: bool,
    #[serde(default)]
    pub show_lvn: bool,
    #[serde(default = "crate::util::serde_defaults::default_hvn_threshold")]
    pub hvn_threshold: f32,
    #[serde(default = "crate::util::serde_defaults::default_lvn_threshold")]
    pub lvn_threshold: f32,
    pub hvn_color: Option<crate::config::color::Rgba>,
    pub lvn_color: Option<crate::config::color::Rgba>,

    // HVN expanded
    #[serde(default)]
    pub hvn_method: ProfileNodeDetectionMethod,
    #[serde(default)]
    pub show_hvn_zones: bool,
    pub hvn_zone_color: Option<crate::config::color::Rgba>,
    #[serde(default = "crate::util::serde_defaults::default_zone_opacity")]
    pub hvn_zone_opacity: f32,
    #[serde(default)]
    pub show_peak_line: bool,
    pub peak_color: Option<crate::config::color::Rgba>,
    #[serde(default)]
    pub peak_line_style: ProfileLineStyle,
    #[serde(default = "crate::util::serde_defaults::default_line_width")]
    pub peak_line_width: f32,
    #[serde(default)]
    pub show_peak_label: bool,

    // LVN expanded
    #[serde(default)]
    pub lvn_method: ProfileNodeDetectionMethod,
    #[serde(default)]
    pub show_lvn_zones: bool,
    pub lvn_zone_color: Option<crate::config::color::Rgba>,
    #[serde(default = "crate::util::serde_defaults::default_zone_opacity")]
    pub lvn_zone_opacity: f32,
    #[serde(default)]
    pub show_valley_line: bool,
    pub valley_color: Option<crate::config::color::Rgba>,
    #[serde(default)]
    pub valley_line_style: ProfileLineStyle,
    #[serde(default = "crate::util::serde_defaults::default_line_width")]
    pub valley_line_width: f32,
    #[serde(default)]
    pub show_valley_label: bool,

    // Colors
    pub volume_color: Option<crate::config::color::Rgba>,
    pub bid_color: Option<crate::config::color::Rgba>,
    pub ask_color: Option<crate::config::color::Rgba>,
    #[serde(default = "crate::util::serde_defaults::default_opacity")]
    pub opacity: f32,

    // Settings tab state
    #[serde(default)]
    pub settings_tab: u8,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            display_type: ProfileDisplayType::default(),
            split_unit: ProfileSplitUnit::default(),
            split_value: serde_defaults::default_split_value(),
            max_profiles: serde_defaults::default_max_profiles(),
            auto_grouping: true,
            auto_group_factor: 1,
            manual_ticks: 1,
            value_area_pct: serde_defaults::default_va_pct(),
            show_va_highlight: true,
            vah_color: None,
            val_color: None,
            show_va_fill: true,
            va_fill_color: None,
            va_fill_opacity: serde_defaults::default_va_fill_opacity(),
            vah_line_width: serde_defaults::default_line_width(),
            vah_line_style: ProfileLineStyle::default(),
            val_line_width: serde_defaults::default_line_width(),
            val_line_style: ProfileLineStyle::default(),
            va_extend: ProfileExtendDirection::default(),
            show_va_labels: false,
            show_poc: true,
            poc_color: None,
            poc_line_width: serde_defaults::default_poc_width(),
            poc_line_style: ProfileLineStyle::default(),
            poc_extend: ProfileExtendDirection::default(),
            show_poc_label: false,
            show_hvn: false,
            show_lvn: false,
            hvn_threshold: serde_defaults::default_hvn_threshold(),
            lvn_threshold: serde_defaults::default_lvn_threshold(),
            hvn_color: None,
            lvn_color: None,
            hvn_method: ProfileNodeDetectionMethod::default(),
            show_hvn_zones: false,
            hvn_zone_color: None,
            hvn_zone_opacity: serde_defaults::default_zone_opacity(),
            show_peak_line: false,
            peak_color: None,
            peak_line_style: ProfileLineStyle::default(),
            peak_line_width: serde_defaults::default_line_width(),
            show_peak_label: false,
            lvn_method: ProfileNodeDetectionMethod::default(),
            show_lvn_zones: false,
            lvn_zone_color: None,
            lvn_zone_opacity: serde_defaults::default_zone_opacity(),
            show_valley_line: false,
            valley_color: None,
            valley_line_style: ProfileLineStyle::default(),
            valley_line_width: serde_defaults::default_line_width(),
            show_valley_label: false,
            volume_color: None,
            bid_color: None,
            ask_color: None,
            opacity: serde_defaults::default_opacity(),
            settings_tab: 0,
        }
    }
}
