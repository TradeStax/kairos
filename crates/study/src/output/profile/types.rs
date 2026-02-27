//! Core profile primitives: levels, nodes, sides, and extend directions.
//!
//! These types are shared by both the simple Volume Profile study and the
//! full VBP (Volume by Price) study.

use serde::{Deserialize, Serialize};

/// Which side of the chart a volume profile renders on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileSide {
    /// Profile bars extend leftward from the price axis.
    Left,
    /// Profile bars extend rightward from the price axis.
    Right,
    /// Profile bars extend in both directions (mirrored).
    Both,
}

/// A single price level within a volume profile.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ProfileLevel {
    /// Price as f64 for display and axis mapping.
    pub price: f64,
    /// Pre-computed price in fixed-point units (10^-8) to avoid
    /// repeated f64-to-Price conversions during rendering.
    pub price_units: i64,
    /// Buy-side (ask) volume at this level.
    pub buy_volume: f32,
    /// Sell-side (bid) volume at this level.
    pub sell_volume: f32,
}

/// Direction for extending horizontal lines beyond the profile bounds.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
pub enum ExtendDirection {
    /// No extension — line ends at profile boundary.
    #[default]
    None,
    /// Extend leftward to chart edge.
    Left,
    /// Extend rightward to chart edge.
    Right,
    /// Extend in both directions to chart edges.
    Both,
}

impl std::fmt::Display for ExtendDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtendDirection::None => write!(f, "None"),
            ExtendDirection::Left => write!(f, "Left"),
            ExtendDirection::Right => write!(f, "Right"),
            ExtendDirection::Both => write!(f, "Both"),
        }
    }
}

/// Method for detecting high-volume nodes (HVN) and low-volume nodes
/// (LVN) within a profile.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize,
)]
pub enum NodeDetectionMethod {
    /// Levels above/below a percentile threshold of the volume
    /// distribution.
    #[default]
    Percentile,
    /// Levels compared relative to the POC volume.
    Relative,
    /// Levels beyond N standard deviations from the mean volume.
    StdDev,
}

impl std::fmt::Display for NodeDetectionMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeDetectionMethod::Percentile => write!(f, "Percentile"),
            NodeDetectionMethod::Relative => write!(f, "Relative"),
            NodeDetectionMethod::StdDev => write!(f, "Std Dev"),
        }
    }
}

/// A detected high-volume or low-volume node in a profile.
#[derive(Debug, Clone, Copy)]
pub struct VolumeNode {
    /// Price level in fixed-point units (10^-8).
    pub price_units: i64,
    /// Price level as f64 for display.
    pub price: f64,
    /// Total volume at this level.
    pub volume: f32,
}
