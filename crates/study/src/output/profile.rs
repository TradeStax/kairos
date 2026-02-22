//! Volume profile and VBP output types.
//!
//! Contains profile level data, VBP configuration types
//! (POC, Value Area, Node detection, VWAP), grouping modes,
//! period settings, and the renderer-side resolved cache.

use crate::config::LineStyleValue;
use data::SerializableColor;
use serde::{Deserialize, Serialize};

/// Which side a volume profile renders on.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize,
)]
pub enum ProfileSide {
    Left,
    Right,
    Both,
}

/// A single level within a volume profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileLevel {
    pub price: f64,
    /// Pre-computed price in i64 units (10^-8) to avoid repeated
    /// f64->Price conversions during rendering.
    pub price_units: i64,
    pub buy_volume: f32,
    pub sell_volume: f32,
}

/// Direction for extending horizontal lines beyond the profile
/// bounds.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize,
    Deserialize,
)]
pub enum ExtendDirection {
    #[default]
    None,
    Left,
    Right,
    Both,
}

impl std::fmt::Display for ExtendDirection {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            ExtendDirection::None => write!(f, "None"),
            ExtendDirection::Left => write!(f, "Left"),
            ExtendDirection::Right => write!(f, "Right"),
            ExtendDirection::Both => write!(f, "Both"),
        }
    }
}

/// Method for detecting high/low volume nodes.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize,
    Deserialize,
)]
pub enum NodeDetectionMethod {
    #[default]
    Percentile,
    Relative,
    StdDev,
}

impl std::fmt::Display for NodeDetectionMethod {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            NodeDetectionMethod::Percentile => {
                write!(f, "Percentile")
            }
            NodeDetectionMethod::Relative => {
                write!(f, "Relative")
            }
            NodeDetectionMethod::StdDev => write!(f, "Std Dev"),
        }
    }
}

/// A detected high or low volume node in a profile.
#[derive(Debug, Clone)]
pub struct VolumeNode {
    /// Price level in fixed-point units (10^-8)
    pub price_units: i64,
    /// Price level as f64
    pub price: f64,
    /// Total volume at this level
    pub volume: f32,
}

/// Visualization type for VBP study.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize,
    Deserialize,
)]
pub enum VbpType {
    #[default]
    Volume,
    BidAskVolume,
    Delta,
    DeltaAndTotalVolume,
    DeltaPercentage,
}

impl std::fmt::Display for VbpType {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            VbpType::Volume => write!(f, "Volume"),
            VbpType::BidAskVolume => {
                write!(f, "Bid/Ask Volume")
            }
            VbpType::Delta => write!(f, "Delta"),
            VbpType::DeltaAndTotalVolume => {
                write!(f, "Delta & Total Volume")
            }
            VbpType::DeltaPercentage => {
                write!(f, "Delta Percentage")
            }
        }
    }
}

/// Time period mode for VBP computation range.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize,
    Deserialize,
)]
pub enum VbpPeriod {
    /// Split into multiple profiles by time interval.
    #[default]
    Split,
    /// Fixed time range (used by drawing tool).
    Custom,
}

impl std::fmt::Display for VbpPeriod {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            VbpPeriod::Split => write!(f, "Split"),
            VbpPeriod::Custom => write!(f, "Custom"),
        }
    }
}

/// How to split candles into profile segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VbpSplitPeriod {
    /// One profile per trading day (86_400_000 ms).
    #[default]
    Day,
    /// One profile every N hours.
    Hours(u32),
    /// One profile every N minutes.
    Minutes(u32),
    /// One profile every N candles.
    Contracts(u32),
}

/// How the renderer should handle VBP level grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VbpGroupingMode {
    /// Renderer merges levels dynamically based on y-axis
    /// scale * factor.
    Automatic { factor: i64 },
    /// Levels are pre-grouped by the study; renderer uses as-is.
    Manual,
}

impl Default for VbpGroupingMode {
    fn default() -> Self {
        Self::Manual
    }
}

/// Cached resolved profile levels after dynamic merging.
///
/// Stored alongside `ProfileOutput` to avoid recomputing the
/// merge on every render frame. Only rebuilt when the dynamic
/// quantum changes (i.e., zoom level changes).
#[derive(Debug, Clone, Default)]
pub struct VbpResolvedCache {
    /// Dynamic quantum used to produce this cache.
    pub quantum: i64,
    /// Merged levels at the cached quantum.
    pub levels: Vec<ProfileLevel>,
    /// POC index in `levels`.
    pub poc: Option<usize>,
    /// Value area (VAH idx, VAL idx) in `levels`.
    pub value_area: Option<(usize, usize)>,
}

/// POC configuration within VBP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VbpPocConfig {
    pub show_poc: bool,
    pub poc_color: SerializableColor,
    pub poc_line_width: f32,
    pub poc_line_style: LineStyleValue,
    pub poc_extend: ExtendDirection,
    pub show_poc_label: bool,
    pub show_developing_poc: bool,
    pub developing_poc_color: SerializableColor,
    pub developing_poc_line_width: f32,
    pub developing_poc_line_style: LineStyleValue,
}

impl Default for VbpPocConfig {
    fn default() -> Self {
        Self {
            show_poc: false,
            poc_color: SerializableColor::new(
                1.0, 0.84, 0.0, 1.0,
            ),
            poc_line_width: 1.5,
            poc_line_style: LineStyleValue::Solid,
            poc_extend: ExtendDirection::None,
            show_poc_label: false,
            show_developing_poc: false,
            developing_poc_color: SerializableColor::new(
                1.0, 0.84, 0.0, 0.5,
            ),
            developing_poc_line_width: 1.0,
            developing_poc_line_style: LineStyleValue::Dashed,
        }
    }
}

/// Value Area configuration within VBP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VbpValueAreaConfig {
    pub show_value_area: bool,
    pub value_area_pct: f32,
    pub show_va_highlight: bool,
    pub vah_color: SerializableColor,
    pub vah_line_width: f32,
    pub vah_line_style: LineStyleValue,
    pub val_color: SerializableColor,
    pub val_line_width: f32,
    pub val_line_style: LineStyleValue,
    pub show_va_fill: bool,
    pub va_fill_color: SerializableColor,
    pub va_fill_opacity: f32,
    pub va_extend: ExtendDirection,
    pub show_va_labels: bool,
}

impl Default for VbpValueAreaConfig {
    fn default() -> Self {
        Self {
            show_value_area: false,
            value_area_pct: 0.7,
            show_va_highlight: false,
            vah_color: SerializableColor::new(
                0.0, 0.7, 1.0, 0.8,
            ),
            vah_line_width: 1.0,
            vah_line_style: LineStyleValue::Solid,
            val_color: SerializableColor::new(
                0.0, 0.7, 1.0, 0.8,
            ),
            val_line_width: 1.0,
            val_line_style: LineStyleValue::Solid,
            show_va_fill: false,
            va_fill_color: SerializableColor::new(
                0.0, 0.7, 1.0, 0.15,
            ),
            va_fill_opacity: 0.15,
            va_extend: ExtendDirection::None,
            show_va_labels: false,
        }
    }
}

/// HVN/LVN (Peak & Valley) configuration within VBP.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VbpNodeConfig {
    // Detection (shared)
    pub hvn_method: NodeDetectionMethod,
    pub hvn_threshold: f32,
    pub lvn_method: NodeDetectionMethod,
    pub lvn_threshold: f32,
    pub min_prominence: f32,

    // HVN Zones
    pub show_hvn_zones: bool,
    pub hvn_zone_color: SerializableColor,
    pub hvn_zone_opacity: f32,

    // Peak Line (single dominant HVN)
    pub show_peak_line: bool,
    pub peak_color: SerializableColor,
    pub peak_line_style: LineStyleValue,
    pub peak_line_width: f32,
    pub peak_extend: ExtendDirection,
    pub show_peak_label: bool,

    // Developing Peak
    pub show_developing_peak: bool,
    pub developing_peak_color: SerializableColor,
    pub developing_peak_line_width: f32,
    pub developing_peak_line_style: LineStyleValue,

    // LVN Zones
    pub show_lvn_zones: bool,
    pub lvn_zone_color: SerializableColor,
    pub lvn_zone_opacity: f32,

    // Valley Line (single deepest LVN)
    pub show_valley_line: bool,
    pub valley_color: SerializableColor,
    pub valley_line_style: LineStyleValue,
    pub valley_line_width: f32,
    pub valley_extend: ExtendDirection,
    pub show_valley_label: bool,

    // Developing Valley
    pub show_developing_valley: bool,
    pub developing_valley_color: SerializableColor,
    pub developing_valley_line_width: f32,
    pub developing_valley_line_style: LineStyleValue,
}

impl Default for VbpNodeConfig {
    fn default() -> Self {
        Self {
            hvn_method: NodeDetectionMethod::Percentile,
            hvn_threshold: 0.85,
            lvn_method: NodeDetectionMethod::Percentile,
            lvn_threshold: 0.15,
            min_prominence: 0.15,

            show_hvn_zones: false,
            hvn_zone_color: SerializableColor::new(
                0.0, 0.9, 0.4, 0.5,
            ),
            hvn_zone_opacity: 0.08,

            show_peak_line: false,
            peak_color: SerializableColor::new(
                0.0, 0.9, 0.4, 0.8,
            ),
            peak_line_style: LineStyleValue::Solid,
            peak_line_width: 1.5,
            peak_extend: ExtendDirection::None,
            show_peak_label: false,

            show_developing_peak: false,
            developing_peak_color: SerializableColor::new(
                0.0, 0.9, 0.4, 0.5,
            ),
            developing_peak_line_width: 1.0,
            developing_peak_line_style: LineStyleValue::Dashed,

            show_lvn_zones: false,
            lvn_zone_color: SerializableColor::new(
                0.9, 0.2, 0.2, 0.5,
            ),
            lvn_zone_opacity: 0.08,

            show_valley_line: false,
            valley_color: SerializableColor::new(
                0.9, 0.2, 0.2, 0.8,
            ),
            valley_line_style: LineStyleValue::Solid,
            valley_line_width: 1.5,
            valley_extend: ExtendDirection::None,
            show_valley_label: false,

            show_developing_valley: false,
            developing_valley_color: SerializableColor::new(
                0.9, 0.2, 0.2, 0.5,
            ),
            developing_valley_line_width: 1.0,
            developing_valley_line_style: LineStyleValue::Dashed,
        }
    }
}

/// Anchored VWAP configuration within VBP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VbpVwapConfig {
    pub show_vwap: bool,
    pub vwap_color: SerializableColor,
    pub vwap_line_width: f32,
    pub vwap_line_style: LineStyleValue,
    pub show_vwap_label: bool,
    pub show_bands: bool,
    pub band_multiplier: f32,
    pub band_color: SerializableColor,
    pub band_line_style: LineStyleValue,
    pub band_line_width: f32,
}

impl Default for VbpVwapConfig {
    fn default() -> Self {
        Self {
            show_vwap: false,
            vwap_color: SerializableColor::new(
                1.0, 1.0, 1.0, 0.8,
            ),
            vwap_line_width: 1.5,
            vwap_line_style: LineStyleValue::Solid,
            show_vwap_label: false,
            show_bands: false,
            band_multiplier: 1.0,
            band_color: SerializableColor::new(
                1.0, 1.0, 1.0, 0.4,
            ),
            band_line_style: LineStyleValue::Dashed,
            band_line_width: 1.0,
        }
    }
}
