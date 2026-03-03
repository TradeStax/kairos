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
        if !weight.is_finite() || !(0.0..=1.0).contains(&weight) {
            return Err("Hybrid weight must be in [0.0, 1.0]");
        }
        Ok(Self::Hybrid { weight })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── hybrid constructor ──────────────────────────────────

    #[test]
    fn hybrid_valid_zero() {
        let s = FootprintScaling::hybrid(0.0).unwrap();
        assert_eq!(s, FootprintScaling::Hybrid { weight: 0.0 });
    }

    #[test]
    fn hybrid_valid_one() {
        let s = FootprintScaling::hybrid(1.0).unwrap();
        assert_eq!(s, FootprintScaling::Hybrid { weight: 1.0 });
    }

    #[test]
    fn hybrid_valid_midpoint() {
        let s = FootprintScaling::hybrid(0.5).unwrap();
        assert_eq!(s, FootprintScaling::Hybrid { weight: 0.5 });
    }

    #[test]
    fn hybrid_rejects_negative() {
        assert!(FootprintScaling::hybrid(-0.1).is_err());
    }

    #[test]
    fn hybrid_rejects_above_one() {
        assert!(FootprintScaling::hybrid(1.1).is_err());
    }

    #[test]
    fn hybrid_rejects_nan() {
        assert!(FootprintScaling::hybrid(f32::NAN).is_err());
    }

    #[test]
    fn hybrid_rejects_infinity() {
        assert!(FootprintScaling::hybrid(f32::INFINITY).is_err());
        assert!(FootprintScaling::hybrid(f32::NEG_INFINITY).is_err());
    }

    // ── Display ─────────────────────────────────────────────

    #[test]
    fn display_linear() {
        assert_eq!(FootprintScaling::Linear.to_string(), "Linear");
    }

    #[test]
    fn display_sqrt() {
        assert_eq!(FootprintScaling::Sqrt.to_string(), "Square Root");
    }

    #[test]
    fn display_log() {
        assert_eq!(FootprintScaling::Log.to_string(), "Logarithmic");
    }

    #[test]
    fn display_visible_range() {
        assert_eq!(FootprintScaling::VisibleRange.to_string(), "Visible Range");
    }

    #[test]
    fn display_datapoint() {
        assert_eq!(FootprintScaling::Datapoint.to_string(), "Datapoint");
    }

    #[test]
    fn display_hybrid() {
        let s = FootprintScaling::hybrid(0.7).unwrap();
        assert_eq!(s.to_string(), "Hybrid (0.7)");
    }

    // ── Default ─────────────────────────────────────────────

    #[test]
    fn default_is_sqrt() {
        assert_eq!(FootprintScaling::default(), FootprintScaling::Sqrt);
    }

    // ── Eq ──────────────────────────────────────────────────

    #[test]
    fn eq_same_variant() {
        assert_eq!(FootprintScaling::Linear, FootprintScaling::Linear);
    }

    #[test]
    fn eq_different_variant() {
        assert_ne!(FootprintScaling::Linear, FootprintScaling::Sqrt);
    }

    #[test]
    fn eq_hybrid_same_weight() {
        let a = FootprintScaling::Hybrid { weight: 0.5 };
        let b = FootprintScaling::Hybrid { weight: 0.5 };
        assert_eq!(a, b);
    }

    #[test]
    fn eq_hybrid_different_weight() {
        let a = FootprintScaling::Hybrid { weight: 0.3 };
        let b = FootprintScaling::Hybrid { weight: 0.7 };
        assert_ne!(a, b);
    }
}
