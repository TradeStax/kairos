//! Core profile types: levels, nodes, sides, and directions.

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
