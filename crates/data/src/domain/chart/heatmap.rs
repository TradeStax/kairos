//! Heatmap chart types — indicators, studies, and coalescing configuration.

use serde::{Deserialize, Serialize};

/// Heatmap cell indicator (what value to render per price-time cell).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, enum_map::Enum)]
pub enum HeatmapIndicator {
    /// Total volume at the price level
    Volume,
    /// Buy minus sell volume (delta) at the price level
    Delta,
    /// Number of trades at the price level
    Trades,
}

impl std::fmt::Display for HeatmapIndicator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HeatmapIndicator::Volume => write!(f, "Volume"),
            HeatmapIndicator::Delta => write!(f, "Delta"),
            HeatmapIndicator::Trades => write!(f, "Trades"),
        }
    }
}

impl HeatmapIndicator {
    /// Return all available heatmap indicators
    #[must_use]
    pub fn all_indicators() -> Vec<HeatmapIndicator> {
        vec![
            HeatmapIndicator::Volume,
            HeatmapIndicator::Delta,
            HeatmapIndicator::Trades,
        ]
    }
}

/// A study overlay rendered on the heatmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeatmapStudy {
    /// Volume profile overlay
    VolumeProfile(ProfileKind),
}

/// How the volume profile is anchored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileKind {
    /// Profile covers the visible chart range
    VisibleRange,
    /// Profile covers a fixed window of N candles
    FixedWindow {
        /// Number of candles in the window
        candles: usize,
    },
    /// Profile with a fixed number of levels
    Fixed(usize),
}

/// Strategy for coalescing adjacent heatmap cells.
///
/// # Manual `Eq` implementation
///
/// `Eq` is sound because the `f32` thresholds are always finite values
/// set via UI sliders — `NaN` is never constructed.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CoalesceKind {
    /// No coalescing
    None,
    /// Coalesce adjacent cells
    Adjacent,
    /// Coalesce all cells in the column
    All,
    /// Coalesce cells whose average exceeds the threshold
    Average(f32),
    /// Coalesce cells where the first exceeds the threshold
    First(f32),
    /// Coalesce cells where the max exceeds the threshold
    Max(f32),
}

impl Eq for CoalesceKind {}

impl CoalesceKind {
    /// Return the threshold value, or 0.0 for non-threshold variants
    #[must_use]
    pub fn threshold(&self) -> f32 {
        match self {
            CoalesceKind::Average(t) | CoalesceKind::First(t) | CoalesceKind::Max(t) => *t,
            _ => 0.0,
        }
    }

    /// Return a copy with an updated threshold value
    #[must_use]
    pub fn with_threshold(&self, threshold: f32) -> Self {
        match self {
            CoalesceKind::Average(_) => CoalesceKind::Average(threshold),
            CoalesceKind::First(_) => CoalesceKind::First(threshold),
            CoalesceKind::Max(_) => CoalesceKind::Max(threshold),
            other => *other,
        }
    }
}

impl HeatmapStudy {
    /// All available heatmap studies.
    pub const ALL: &'static [HeatmapStudy] =
        &[HeatmapStudy::VolumeProfile(ProfileKind::VisibleRange)];

    /// Return `true` if `self` and `other` are the same study variant
    /// (ignoring inner configuration)
    #[must_use]
    pub fn is_same_type(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

impl std::fmt::Display for HeatmapStudy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HeatmapStudy::VolumeProfile(kind) => {
                write!(f, "Volume Profile ({})", kind)
            }
        }
    }
}

impl std::fmt::Display for ProfileKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileKind::VisibleRange => {
                write!(f, "Visible Range")
            }
            ProfileKind::FixedWindow { candles } => {
                write!(f, "Fixed Window ({})", candles)
            }
            ProfileKind::Fixed(n) => write!(f, "Fixed ({})", n),
        }
    }
}

/// Number of stale heatmap cells that triggers a cleanup pass.
pub const CLEANUP_THRESHOLD: usize = 1000;
