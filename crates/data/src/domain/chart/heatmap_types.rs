//! Heatmap chart types: indicators, studies, coalescing

use serde::{Deserialize, Serialize};

/// Heatmap indicator types (display toggles for heatmap rendering)
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, enum_map::Enum,
)]
pub enum HeatmapIndicator {
    Volume,
    Delta,
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
    /// Get all available indicators
    pub fn all_indicators() -> Vec<HeatmapIndicator> {
        vec![
            HeatmapIndicator::Volume,
            HeatmapIndicator::Delta,
            HeatmapIndicator::Trades,
        ]
    }
}

/// Heatmap study types
pub mod heatmap {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum HeatmapStudy {
        VolumeProfile(ProfileKind),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum ProfileKind {
        VisibleRange,
        FixedWindow { candles: usize },
        Fixed(usize), // Alias for FixedWindow
    }

    #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
    pub enum CoalesceKind {
        None,
        Adjacent,
        All,
        Average(f32),
        First(f32),
        Max(f32),
    }

    // SAFETY: Manual Eq is sound here because f32 thresholds in Average/First/Max
    // are always finite values set via UI sliders. NaN is never constructed.
    impl Eq for CoalesceKind {}

    impl CoalesceKind {
        pub fn threshold(&self) -> f32 {
            match self {
                CoalesceKind::Average(t)
                | CoalesceKind::First(t)
                | CoalesceKind::Max(t) => *t,
                _ => 0.0,
            }
        }

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
        pub const ALL: &'static [HeatmapStudy] =
            &[HeatmapStudy::VolumeProfile(ProfileKind::VisibleRange)];

        pub fn is_same_type(&self, other: &Self) -> bool {
            std::mem::discriminant(self) == std::mem::discriminant(other)
        }
    }

    impl std::fmt::Display for HeatmapStudy {
        fn fmt(
            &self,
            f: &mut std::fmt::Formatter<'_>,
        ) -> std::fmt::Result {
            match self {
                HeatmapStudy::VolumeProfile(kind) => {
                    write!(f, "Volume Profile ({})", kind)
                }
            }
        }
    }

    impl std::fmt::Display for ProfileKind {
        fn fmt(
            &self,
            f: &mut std::fmt::Formatter<'_>,
        ) -> std::fmt::Result {
            match self {
                ProfileKind::VisibleRange => write!(f, "Visible Range"),
                ProfileKind::FixedWindow { candles } => {
                    write!(f, "Fixed Window ({})", candles)
                }
                ProfileKind::Fixed(n) => write!(f, "Fixed ({})", n),
            }
        }
    }

    pub const CLEANUP_THRESHOLD: usize = 1000;

    // Re-export HeatmapConfig for UI
    pub use crate::state::pane::HeatmapConfig as Config;
}
