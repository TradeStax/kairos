//! Footprint rendering configuration: modes, data types, and layout
//! constants.
//!
//! These enums and structs control how the footprint study's output is
//! drawn by the chart renderer.

use serde::{Deserialize, Serialize};

/// Layout constants for a candle-replace study (footprint).
///
/// Overrides the chart's default cell sizing, zoom bounds, and initial
/// candle window to accommodate the wider footprint bars.
#[derive(Debug, Clone, Copy)]
pub struct CandleRenderConfig {
    /// Default cell width in logical pixels.
    pub default_cell_width: f32,
    /// Maximum cell width when zoomed in.
    pub max_cell_width: f32,
    /// Minimum cell width when zoomed out.
    pub min_cell_width: f32,
    /// Height-to-width ratio for footprint cells.
    pub cell_height_ratio: f32,
    /// Number of candles visible at the default zoom level.
    pub initial_candle_window: usize,
    /// Number of extra cells for x-axis auto-scale padding.
    pub autoscale_x_cells: f32,
}

/// How the renderer handles footprint level grouping at different zoom
/// levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FootprintGroupingMode {
    /// Renderer merges levels dynamically based on the y-axis scale
    /// multiplied by `factor`.
    Automatic {
        /// Multiplier applied to the y-axis scale to determine the
        /// dynamic merge quantum.
        factor: i64,
    },
    /// Levels are pre-grouped by the study; renderer uses them as-is.
    Manual,
}

/// Style for the thin candle marker drawn alongside footprint bars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OutsideBarStyle {
    /// Filled body only (no wicks).
    #[default]
    Body,
    /// Full candlestick with wicks.
    Candle,
    /// No outside bar marker.
    None,
}

impl std::fmt::Display for OutsideBarStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutsideBarStyle::Body => write!(f, "Body"),
            OutsideBarStyle::Candle => write!(f, "Candle"),
            OutsideBarStyle::None => write!(f, "None"),
        }
    }
}

/// Text formatting mode for footprint level values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TextFormat {
    /// Choose formatting automatically based on value magnitude.
    #[default]
    Automatic,
    /// Always show the raw numeric value.
    Normal,
    /// Use "K" suffix for thousands (e.g. "1.5K").
    K,
}

impl std::fmt::Display for TextFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextFormat::Automatic => write!(f, "Automatic"),
            TextFormat::Normal => write!(f, "Normal"),
            TextFormat::K => write!(f, "K"),
        }
    }
}

/// Background coloring mode for footprint cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BackgroundColorMode {
    /// Color intensity proportional to total volume at each level.
    #[default]
    VolumeIntensity,
    /// Color intensity proportional to net delta at each level.
    DeltaIntensity,
    /// No background coloring.
    None,
}

impl std::fmt::Display for BackgroundColorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackgroundColorMode::VolumeIntensity => {
                write!(f, "Volume Intensity")
            }
            BackgroundColorMode::DeltaIntensity => {
                write!(f, "Delta Intensity")
            }
            BackgroundColorMode::None => write!(f, "None"),
        }
    }
}

/// Rendering mode for footprint candles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FootprintRenderMode {
    /// Grid of cells with buy/sell values at each price level.
    Box,
    /// Horizontal volume profile bars within each candle.
    #[default]
    Profile,
}

impl std::fmt::Display for FootprintRenderMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FootprintRenderMode::Box => write!(f, "Box"),
            FootprintRenderMode::Profile => write!(f, "Profile"),
        }
    }
}

/// Data type shown in footprint cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FootprintDataType {
    /// Total volume per level.
    #[default]
    Volume,
    /// Bid and ask volume shown separately.
    BidAskSplit,
    /// Net delta (ask - bid) per level.
    Delta,
    /// Delta alongside total volume.
    DeltaAndVolume,
}

impl std::fmt::Display for FootprintDataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FootprintDataType::Volume => write!(f, "Volume"),
            FootprintDataType::BidAskSplit => write!(f, "Bid/Ask Split"),
            FootprintDataType::Delta => write!(f, "Delta"),
            FootprintDataType::DeltaAndVolume => {
                write!(f, "Delta + Volume")
            }
        }
    }
}

/// Position of the thin candle body relative to the footprint bars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FootprintCandlePosition {
    /// No candle body marker.
    None,
    /// Candle body on the left side of the footprint bar.
    #[default]
    Left,
    /// Candle body centered within the footprint bar.
    Center,
    /// Candle body on the right side of the footprint bar.
    Right,
}

impl std::fmt::Display for FootprintCandlePosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FootprintCandlePosition::None => write!(f, "None"),
            FootprintCandlePosition::Left => write!(f, "Left"),
            FootprintCandlePosition::Center => write!(f, "Center"),
            FootprintCandlePosition::Right => write!(f, "Right"),
        }
    }
}
