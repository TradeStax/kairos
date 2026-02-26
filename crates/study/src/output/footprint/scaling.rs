//! Scaling strategies for footprint bar width computation.

use serde::{Deserialize, Serialize};

/// Cluster scaling strategy for footprint bar widths.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum FootprintScaling {
    Linear,
    #[default]
    Sqrt,
    Log,
    VisibleRange,
    Datapoint,
    Hybrid {
        weight: f32,
    },
}

// SAFETY: Manual Eq is sound -- `weight` is always finite
// (0.0..=1.0).
impl Eq for FootprintScaling {}

impl std::fmt::Display for FootprintScaling {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FootprintScaling::Linear => write!(f, "Linear"),
            FootprintScaling::Sqrt => write!(f, "Square Root"),
            FootprintScaling::Log => write!(f, "Logarithmic"),
            FootprintScaling::VisibleRange => {
                write!(f, "Visible Range")
            }
            FootprintScaling::Datapoint => {
                write!(f, "Datapoint")
            }
            FootprintScaling::Hybrid { weight } => {
                write!(f, "Hybrid ({weight:.1})")
            }
        }
    }
}

impl FootprintScaling {
    /// Construct a `Hybrid` variant with a validated weight in `[0.0, 1.0]`.
    ///
    /// Returns `Err` if `weight` is non-finite or outside `[0.0, 1.0]`.
    /// The `// SAFETY` comment on the manual `Eq` impl assumes finite weights;
    /// this constructor is the enforcement point for that invariant.
    pub fn hybrid(weight: f32) -> Result<Self, &'static str> {
        if !weight.is_finite() || !(0.0..=1.0).contains(&weight) {
            return Err("Hybrid weight must be in [0.0, 1.0]");
        }
        Ok(Self::Hybrid { weight })
    }
}
