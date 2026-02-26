//! Footprint rendering configuration: modes, data types, and layout constants.

use serde::{Deserialize, Serialize};

/// Layout constants for a CandleReplace study.
/// Overrides the chart's default cell sizing, zoom bounds,
/// and initial candle window.
#[derive(Debug, Clone, Copy)]
pub struct CandleRenderConfig {
    pub default_cell_width: f32,
    pub max_cell_width: f32,
    pub min_cell_width: f32,
    pub cell_height_ratio: f32,
    pub initial_candle_window: usize,
    pub autoscale_x_cells: f32,
}

/// How the renderer should handle footprint level grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FootprintGroupingMode {
    /// Renderer merges levels dynamically based on y-axis
    /// scale * factor.
    Automatic { factor: i64 },
    /// Levels are pre-grouped by the study; renderer uses as-is.
    Manual,
}

/// Style for the thin candle marker drawn outside footprint bars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OutsideBarStyle {
    #[default]
    Body,
    Candle,
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
    #[default]
    Automatic,
    Normal,
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

/// Background coloring mode for footprint cells (Box mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BackgroundColorMode {
    #[default]
    VolumeIntensity,
    DeltaIntensity,
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

/// Rendering mode for footprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FootprintRenderMode {
    Box,
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

/// Data type shown in the footprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FootprintDataType {
    #[default]
    Volume,
    BidAskSplit,
    Delta,
    DeltaAndVolume,
}

impl std::fmt::Display for FootprintDataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FootprintDataType::Volume => write!(f, "Volume"),
            FootprintDataType::BidAskSplit => {
                write!(f, "Bid/Ask Split")
            }
            FootprintDataType::Delta => write!(f, "Delta"),
            FootprintDataType::DeltaAndVolume => {
                write!(f, "Delta + Volume")
            }
        }
    }
}

/// Candle body position relative to the footprint bars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FootprintCandlePosition {
    None,
    #[default]
    Left,
    Center,
    Right,
}

impl std::fmt::Display for FootprintCandlePosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FootprintCandlePosition::None => write!(f, "None"),
            FootprintCandlePosition::Left => write!(f, "Left"),
            FootprintCandlePosition::Center => {
                write!(f, "Center")
            }
            FootprintCandlePosition::Right => write!(f, "Right"),
        }
    }
}
