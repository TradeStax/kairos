//! Scaling strategies for footprint bar width computation.
//!
//! Each variant defines how raw volume values map to horizontal bar width
//! within a footprint candle.

use serde::{Deserialize, Serialize};

/// Scaling strategy for footprint bar widths.
///
/// Controls how volume values at each price level are mapped to the
/// horizontal width of footprint bars.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum FootprintScaling {
    /// Direct proportional scaling: width = volume / max_volume.
    Linear,
    /// Square root scaling for reduced visual dominance of outliers.
    #[default]
    Sqrt,
    /// Logarithmic scaling for wide-range volume distributions.
    Log,
    /// Scale relative to the maximum volume in the visible range.
    VisibleRange,
    /// Each level gets equal width (useful for highlighting presence).
    Datapoint,
    /// Blend of sqrt and visible-range scaling.
    Hybrid {
        /// Blend weight in `[0.0, 1.0]`: 0.0 = pure sqrt,
        /// 1.0 = pure visible-range.
        weight: f32,
    },
}

// SAFETY: Manual Eq is sound — `weight` is always finite ([0.0, 1.0]),
// enforced by the `hybrid()` constructor.
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
            FootprintScaling::Datapoint => write!(f, "Datapoint"),
            FootprintScaling::Hybrid { weight } => {
                write!(f, "Hybrid ({weight:.1})")
            }
        }
    }
}

impl FootprintScaling {
    /// Construct a `Hybrid` variant with a validated weight.
    ///
    /// Returns `Err` if `weight` is non-finite or outside `[0.0, 1.0]`.
    /// This constructor enforces the invariant assumed by the manual
    /// `Eq` impl.
    pub fn hybrid(weight: f32) -> Result<Self, &'static str> {
        if !weight.is_finite()
            || !(0.0..=1.0).contains(&weight)
        {
            return Err("Hybrid weight must be in [0.0, 1.0]");
        }
        Ok(Self::Hybrid { weight })
    }
}
