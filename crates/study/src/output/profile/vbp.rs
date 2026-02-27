//! VBP (Volume by Price) configuration types.
//!
//! Covers visualization mode, period splitting, grouping strategy, render
//! cache, and sub-feature configs: POC, Value Area, HVN/LVN Nodes, and
//! anchored VWAP.

use crate::config::LineStyleValue;
use data::SerializableColor;
use serde::{Deserialize, Serialize};

use super::types::{ExtendDirection, NodeDetectionMethod, ProfileLevel};

/// Visualization type for VBP bars.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
pub enum VbpType {
    /// Total volume per level (single color).
    #[default]
    Volume,
    /// Bid and ask volume shown side by side.
    BidAskVolume,
    /// Net delta (ask - bid) per level.
    Delta,
    /// Delta bars overlaid on total volume bars.
    DeltaAndTotalVolume,
    /// Delta as a percentage of total volume per level.
    DeltaPercentage,
}

impl std::fmt::Display for VbpType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VbpType::Volume => write!(f, "Volume"),
            VbpType::BidAskVolume => write!(f, "Bid/Ask Volume"),
            VbpType::Delta => write!(f, "Delta"),
            VbpType::DeltaAndTotalVolume => {
                write!(f, "Delta & Total Volume")
            }
            VbpType::DeltaPercentage => write!(f, "Delta Percentage"),
        }
    }
}

/// Time period mode for VBP computation range.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
pub enum VbpPeriod {
    /// Split the visible range into multiple profiles by time interval.
    #[default]
    Split,
    /// Fixed time range (used by drawing tool anchored profiles).
    Custom,
}

impl std::fmt::Display for VbpPeriod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VbpPeriod::Split => write!(f, "Split"),
            VbpPeriod::Custom => write!(f, "Custom"),
        }
    }
}

/// How to split the time range into individual profile segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VbpSplitPeriod {
    /// One profile per trading day (86,400,000 ms).
    #[default]
    Day,
    /// One profile every N hours.
    Hours(u32),
    /// One profile every N minutes.
    Minutes(u32),
    /// One profile every N candles.
    Contracts(u32),
}

/// How the renderer handles VBP level grouping at different zoom levels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VbpGroupingMode {
    /// Renderer merges levels dynamically based on the y-axis scale
    /// multiplied by `factor`.
    Automatic {
        /// Multiplier applied to the y-axis scale to determine the
        /// dynamic merge quantum.
        factor: i64,
    },
    /// Levels are pre-grouped by the study; renderer uses them as-is.
    #[default]
    Manual,
}

/// Cached resolved profile levels after dynamic merging.
///
/// Stored alongside [`ProfileOutput`](super::ProfileOutput) to avoid
/// recomputing the merge on every render frame. Rebuilt only when the
/// dynamic quantum changes (i.e., zoom level changes).
#[derive(Debug, Clone, Default)]
pub struct VbpResolvedCache {
    /// Dynamic quantum used to produce this cache.
    pub quantum: i64,
    /// Merged levels at the cached quantum.
    pub levels: Vec<ProfileLevel>,
    /// POC index within `levels`.
    pub poc: Option<usize>,
    /// Value area bounds as `(VAH index, VAL index)` within `levels`.
    pub value_area: Option<(usize, usize)>,
}

// ---------------------------------------------------------------------------
// Sub-feature configs
// ---------------------------------------------------------------------------

/// Point of Control (POC) configuration within VBP.
///
/// Controls visibility and styling of the POC line (highest volume level)
/// and its developing variant that updates as new candles arrive.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct VbpPocConfig {
    /// Show the POC horizontal line.
    pub show_poc: bool,
    /// POC line color.
    pub poc_color: SerializableColor,
    /// POC line width in logical pixels.
    pub poc_line_width: f32,
    /// POC line style (solid, dashed, dotted).
    pub poc_line_style: LineStyleValue,
    /// Direction to extend the POC line beyond profile bounds.
    pub poc_extend: ExtendDirection,
    /// Show a price label at the POC line.
    pub show_poc_label: bool,
    /// Show the developing POC that updates with each candle.
    pub show_developing_poc: bool,
    /// Developing POC line color.
    pub developing_poc_color: SerializableColor,
    /// Developing POC line width in logical pixels.
    pub developing_poc_line_width: f32,
    /// Developing POC line style.
    pub developing_poc_line_style: LineStyleValue,
}

impl Default for VbpPocConfig {
    fn default() -> Self {
        Self {
            show_poc: false,
            poc_color: SerializableColor::new(1.0, 0.84, 0.0, 1.0),
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
///
/// The value area is the price range containing a specified percentage of
/// the total volume. Bounded by VAH (Value Area High) and VAL (Value Area
/// Low).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct VbpValueAreaConfig {
    /// Enable value area computation and rendering.
    pub show_value_area: bool,
    /// Percentage of total volume to include (e.g. 0.7 = 70%).
    pub value_area_pct: f32,
    /// Highlight value area levels within the profile bars.
    pub show_va_highlight: bool,
    /// VAH (Value Area High) line color.
    pub vah_color: SerializableColor,
    /// VAH line width in logical pixels.
    pub vah_line_width: f32,
    /// VAH line style.
    pub vah_line_style: LineStyleValue,
    /// VAL (Value Area Low) line color.
    pub val_color: SerializableColor,
    /// VAL line width in logical pixels.
    pub val_line_width: f32,
    /// VAL line style.
    pub val_line_style: LineStyleValue,
    /// Fill the region between VAH and VAL with color.
    pub show_va_fill: bool,
    /// Fill color for the value area region.
    pub va_fill_color: SerializableColor,
    /// Fill opacity for the value area region.
    pub va_fill_opacity: f32,
    /// Direction to extend VA lines beyond profile bounds.
    pub va_extend: ExtendDirection,
    /// Show price labels on VAH/VAL lines.
    pub show_va_labels: bool,
}

impl Default for VbpValueAreaConfig {
    fn default() -> Self {
        Self {
            show_value_area: false,
            value_area_pct: 0.7,
            show_va_highlight: false,
            vah_color: SerializableColor::new(0.0, 0.7, 1.0, 0.8),
            vah_line_width: 1.0,
            vah_line_style: LineStyleValue::Solid,
            val_color: SerializableColor::new(0.0, 0.7, 1.0, 0.8),
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

/// HVN/LVN (High/Low Volume Node) configuration within VBP.
///
/// Controls detection thresholds, zone highlighting, and line styling for
/// both peak (HVN) and valley (LVN) nodes. Each node type supports a
/// zone overlay, a single dominant line, and a developing variant.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct VbpNodeConfig {
    // -- Detection (shared) --
    /// Method for detecting high-volume nodes.
    pub hvn_method: NodeDetectionMethod,
    /// HVN detection threshold (interpretation depends on method).
    pub hvn_threshold: f32,
    /// Method for detecting low-volume nodes.
    pub lvn_method: NodeDetectionMethod,
    /// LVN detection threshold (interpretation depends on method).
    pub lvn_threshold: f32,
    /// Minimum prominence required for a node to be reported.
    pub min_prominence: f32,

    // -- HVN Zones --
    /// Show shaded zones for all detected HVN levels.
    pub show_hvn_zones: bool,
    /// HVN zone fill color.
    pub hvn_zone_color: SerializableColor,
    /// HVN zone fill opacity.
    pub hvn_zone_opacity: f32,

    // -- Peak Line (single dominant HVN) --
    /// Show a line at the single highest-volume node.
    pub show_peak_line: bool,
    /// Peak line color.
    pub peak_color: SerializableColor,
    /// Peak line style.
    pub peak_line_style: LineStyleValue,
    /// Peak line width in logical pixels.
    pub peak_line_width: f32,
    /// Direction to extend the peak line.
    pub peak_extend: ExtendDirection,
    /// Show a price label on the peak line.
    pub show_peak_label: bool,

    // -- Developing Peak --
    /// Show the developing peak that updates with each candle.
    pub show_developing_peak: bool,
    /// Developing peak line color.
    pub developing_peak_color: SerializableColor,
    /// Developing peak line width in logical pixels.
    pub developing_peak_line_width: f32,
    /// Developing peak line style.
    pub developing_peak_line_style: LineStyleValue,

    // -- LVN Zones --
    /// Show shaded zones for all detected LVN levels.
    pub show_lvn_zones: bool,
    /// LVN zone fill color.
    pub lvn_zone_color: SerializableColor,
    /// LVN zone fill opacity.
    pub lvn_zone_opacity: f32,

    // -- Valley Line (single deepest LVN) --
    /// Show a line at the single lowest-volume node.
    pub show_valley_line: bool,
    /// Valley line color.
    pub valley_color: SerializableColor,
    /// Valley line style.
    pub valley_line_style: LineStyleValue,
    /// Valley line width in logical pixels.
    pub valley_line_width: f32,
    /// Direction to extend the valley line.
    pub valley_extend: ExtendDirection,
    /// Show a price label on the valley line.
    pub show_valley_label: bool,

    // -- Developing Valley --
    /// Show the developing valley that updates with each candle.
    pub show_developing_valley: bool,
    /// Developing valley line color.
    pub developing_valley_color: SerializableColor,
    /// Developing valley line width in logical pixels.
    pub developing_valley_line_width: f32,
    /// Developing valley line style.
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
            peak_color: SerializableColor::new(0.0, 0.9, 0.4, 0.8),
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
            valley_color: SerializableColor::new(0.9, 0.2, 0.2, 0.8),
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
///
/// Renders a VWAP line anchored to the profile's time range, with
/// optional standard deviation bands.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct VbpVwapConfig {
    /// Show the VWAP line.
    pub show_vwap: bool,
    /// VWAP line color.
    pub vwap_color: SerializableColor,
    /// VWAP line width in logical pixels.
    pub vwap_line_width: f32,
    /// VWAP line style.
    pub vwap_line_style: LineStyleValue,
    /// Show a price label on the VWAP line.
    pub show_vwap_label: bool,
    /// Show standard deviation bands around VWAP.
    pub show_bands: bool,
    /// Standard deviation multiplier for bands (e.g. 1.0, 2.0).
    pub band_multiplier: f32,
    /// Band line color.
    pub band_color: SerializableColor,
    /// Band line style.
    pub band_line_style: LineStyleValue,
    /// Band line width in logical pixels.
    pub band_line_width: f32,
}

impl Default for VbpVwapConfig {
    fn default() -> Self {
        Self {
            show_vwap: false,
            vwap_color: SerializableColor::new(1.0, 1.0, 1.0, 0.8),
            vwap_line_width: 1.5,
            vwap_line_style: LineStyleValue::Solid,
            show_vwap_label: false,
            show_bands: false,
            band_multiplier: 1.0,
            band_color: SerializableColor::new(1.0, 1.0, 1.0, 0.4),
            band_line_style: LineStyleValue::Dashed,
            band_line_width: 1.0,
        }
    }
}
