//! UI scale factor configuration.

use serde::{Deserialize, Serialize};

pub const MIN_SCALE: f32 = 0.8;
pub const MAX_SCALE: f32 = 1.5;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub struct ScaleFactor(f32);

impl Default for ScaleFactor {
    fn default() -> Self {
        Self(1.0)
    }
}

impl From<f32> for ScaleFactor {
    fn from(value: f32) -> Self {
        ScaleFactor(value.clamp(MIN_SCALE, MAX_SCALE))
    }
}

impl From<ScaleFactor> for f32 {
    fn from(value: ScaleFactor) -> Self {
        value.0
    }
}
