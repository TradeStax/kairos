//! Performance Presets
//!
//! Pre-configured settings optimized for different instruments and use cases.

use super::lod::LodLevel;
use super::perf::RenderBudget;
use data::ChartBasis;

/// Performance preset for different instrument types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformancePreset {
    /// High volume instruments (ES, NQ, MES, MNQ)
    HighVolume,
    /// Medium volume instruments (RTY, YM)
    MediumVolume,
    /// Low volume instruments (Agricultural, less liquid contracts)
    LowVolume,
    /// Custom preset
    Custom,
}

impl PerformancePreset {
    /// Detect preset from instrument symbol
    pub fn detect_from_symbol(symbol: &str) -> Self {
        let upper = symbol.to_uppercase();

        // High volume: E-mini and Micro futures
        if upper.starts_with("ES") || upper.starts_with("NQ")
            || upper.starts_with("MES") || upper.starts_with("MNQ")
            || upper.contains("ES.") || upper.contains("NQ.")
        {
            PerformancePreset::HighVolume
        }
        // Medium volume: Russell, Dow
        else if upper.starts_with("RTY") || upper.starts_with("YM")
            || upper.starts_with("M2K") || upper.starts_with("MYM")
        {
            PerformancePreset::MediumVolume
        }
        // Low volume: Everything else
        else {
            PerformancePreset::LowVolume
        }
    }

    /// Get recommended settings for this preset
    pub fn settings(&self) -> PresetSettings {
        match self {
            PerformancePreset::HighVolume => PresetSettings {
                max_trade_markers: 5_000,
                default_lod: LodLevel::Medium,
                aggressive_decimation: true,
                render_budget: RenderBudget::strict(),
                trade_size_filter: 1.0, // Filter tiny trades
                sparse_threshold: 500, // Switch to dense mode earlier
                enable_progressive: true,
            },
            PerformancePreset::MediumVolume => PresetSettings {
                max_trade_markers: 10_000,
                default_lod: LodLevel::High,
                aggressive_decimation: false,
                render_budget: RenderBudget::default(),
                trade_size_filter: 0.0,
                sparse_threshold: 1_000,
                enable_progressive: true,
            },
            PerformancePreset::LowVolume => PresetSettings {
                max_trade_markers: 20_000,
                default_lod: LodLevel::High,
                aggressive_decimation: false,
                render_budget: RenderBudget::relaxed(),
                trade_size_filter: 0.0,
                sparse_threshold: 2_000,
                enable_progressive: false, // Not needed for low volume
            },
            PerformancePreset::Custom => PresetSettings::default(),
        }
    }

    /// Get all available presets
    pub fn all() -> &'static [PerformancePreset] {
        &[
            PerformancePreset::HighVolume,
            PerformancePreset::MediumVolume,
            PerformancePreset::LowVolume,
            PerformancePreset::Custom,
        ]
    }
}

impl std::fmt::Display for PerformancePreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PerformancePreset::HighVolume => write!(f, "High Volume (ES, NQ)"),
            PerformancePreset::MediumVolume => write!(f, "Medium Volume (RTY, YM)"),
            PerformancePreset::LowVolume => write!(f, "Low Volume"),
            PerformancePreset::Custom => write!(f, "Custom"),
        }
    }
}

/// Settings from a performance preset
#[derive(Debug, Clone, Copy)]
pub struct PresetSettings {
    /// Maximum trade markers to render
    pub max_trade_markers: usize,
    /// Default LOD level
    pub default_lod: LodLevel,
    /// Use aggressive decimation
    pub aggressive_decimation: bool,
    /// Render budget
    pub render_budget: RenderBudget,
    /// Minimum trade size to display
    pub trade_size_filter: f32,
    /// Threshold for switching to dense mode
    pub sparse_threshold: usize,
    /// Enable progressive rendering
    pub enable_progressive: bool,
}

impl Default for PresetSettings {
    fn default() -> Self {
        PerformancePreset::MediumVolume.settings()
    }
}

/// Basis-specific optimizations
pub struct BasisOptimizer;

impl BasisOptimizer {
    /// Get recommended cell width for basis
    pub fn recommended_cell_width(basis: ChartBasis) -> f32 {
        match basis {
            ChartBasis::Time(timeframe) => {
                use data::Timeframe;
                match timeframe {
                    Timeframe::M1s | Timeframe::M5s | Timeframe::M10s | Timeframe::M30s => 2.0,
                    Timeframe::M1 | Timeframe::M3 | Timeframe::M5 => 4.0,
                    Timeframe::M15 | Timeframe::M30 => 6.0,
                    Timeframe::H1 | Timeframe::H4 => 8.0,
                    Timeframe::D1 => 12.0,
                }
            }
            ChartBasis::Tick(count) => {
                // Tick charts: wider cells for larger tick counts
                if count <= 10 {
                    3.0
                } else if count <= 50 {
                    4.0
                } else if count <= 100 {
                    5.0
                } else {
                    6.0
                }
            }
        }
    }

    /// Get recommended max visible candles for basis
    pub fn recommended_max_visible(basis: ChartBasis) -> usize {
        match basis {
            ChartBasis::Time(timeframe) => {
                use data::Timeframe;
                match timeframe {
                    Timeframe::M1s | Timeframe::M5s | Timeframe::M10s => 200,
                    Timeframe::M30s => 150,
                    Timeframe::M1 | Timeframe::M3 => 120,
                    Timeframe::M5 => 100,
                    Timeframe::M15 => 80,
                    Timeframe::M30 => 60,
                    Timeframe::H1 => 48,
                    Timeframe::H4 => 24,
                    Timeframe::D1 => 20,
                }
            }
            ChartBasis::Tick(_) => 100, // Consistent for tick charts
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_detection() {
        assert_eq!(
            PerformancePreset::detect_from_symbol("ESH5"),
            PerformancePreset::HighVolume
        );
        assert_eq!(
            PerformancePreset::detect_from_symbol("NQ.c.0"),
            PerformancePreset::HighVolume
        );
        assert_eq!(
            PerformancePreset::detect_from_symbol("RTY"),
            PerformancePreset::MediumVolume
        );
        assert_eq!(
            PerformancePreset::detect_from_symbol("ZC"),
            PerformancePreset::LowVolume
        );
    }

    #[test]
    fn test_high_volume_settings() {
        let settings = PerformancePreset::HighVolume.settings();
        assert!(settings.max_trade_markers < 10_000); // Stricter limits
        assert!(settings.aggressive_decimation); // More aggressive
        assert!(settings.trade_size_filter > 0.0); // Filters small trades
    }

    #[test]
    fn test_basis_cell_width() {
        let width_m1 = BasisOptimizer::recommended_cell_width(
            ChartBasis::Time(data::Timeframe::M1)
        );
        let width_d1 = BasisOptimizer::recommended_cell_width(
            ChartBasis::Time(data::Timeframe::D1)
        );

        assert!(width_d1 > width_m1); // Daily charts need wider cells
    }
}
