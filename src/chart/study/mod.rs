//! Chart Study System
//!
//! Studies are analytical overlays that highlight patterns in market data:
//! - `POC` - Point of Control (price with highest volume)
//! - `NPoC` - Naked Point of Control (unvisited POCs)
//! - `ValueArea` - Value Area High/Low (70% of volume)
//! - `Imbalance` - Bid/Ask imbalance markers
//! - `VolumeProfile` - Volume distribution by price

mod imbalance;
mod npoc;
mod poc;
mod value_area;
mod volume_profile;

pub use imbalance::{ImbalanceConfig, draw_imbalance_markers};
pub use npoc::{NpocConfig, draw_npocs};
pub use poc::{PocConfig, find_poc};
pub use value_area::{ValueAreaConfig, calculate_value_area};
pub use volume_profile::{VolumeProfileConfig, build_volume_profile};

use data::FootprintStudy;
use exchange::util::Price;
use std::collections::BTreeMap;

/// Trade group with buy/sell quantities at a price level
#[derive(Default, Clone, Debug)]
pub struct TradeGroup {
    pub buy_qty: f32,
    pub sell_qty: f32,
}

impl TradeGroup {
    /// Create a new trade group
    pub fn new(buy_qty: f32, sell_qty: f32) -> Self {
        Self { buy_qty, sell_qty }
    }

    /// Total quantity (buy + sell)
    pub fn total_qty(&self) -> f32 {
        self.buy_qty + self.sell_qty
    }

    /// Delta (buy - sell)
    pub fn delta_qty(&self) -> f32 {
        self.buy_qty - self.sell_qty
    }
}

/// Footprint data for a single candle
pub type Footprint = BTreeMap<Price, TradeGroup>;

/// Study trait for chart studies
pub trait Study {
    /// Unique identifier for this study type
    fn id(&self) -> &'static str;

    /// Display name for the study
    fn name(&self) -> &str;

    /// Whether this study is currently enabled
    fn is_enabled(&self) -> bool;

    /// Enable or disable the study
    fn set_enabled(&mut self, enabled: bool);
}

/// Registry for managing chart studies
pub struct StudyRegistry {
    studies: Vec<FootprintStudy>,
}

impl StudyRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            studies: Vec::new(),
        }
    }

    /// Create a registry with the given studies
    pub fn with_studies(studies: Vec<FootprintStudy>) -> Self {
        Self { studies }
    }

    /// Get all enabled studies
    pub fn enabled(&self) -> &[FootprintStudy] {
        &self.studies
    }

    /// Toggle a study on/off
    pub fn toggle(&mut self, study: FootprintStudy) {
        if let Some(pos) = self.studies.iter().position(|s| s.is_same_type(&study)) {
            self.studies.remove(pos);
        } else {
            self.studies.push(study);
        }
    }

    /// Check if a study type is enabled
    pub fn is_enabled(&self, study: &FootprintStudy) -> bool {
        self.studies.iter().any(|s| s.is_same_type(study))
    }

    /// Update study configuration
    pub fn update_config(&mut self, study: FootprintStudy) {
        if let Some(existing) = self.studies.iter_mut().find(|s| s.is_same_type(&study)) {
            *existing = study;
        }
    }

    /// Get NPoC lookback if enabled
    pub fn npoc_lookback(&self) -> Option<usize> {
        self.studies.iter().find_map(|s| {
            if let FootprintStudy::NPoC { lookback } = s {
                Some(*lookback)
            } else {
                None
            }
        })
    }

    /// Get imbalance config if enabled
    pub fn imbalance_config(&self) -> Option<(usize, bool, bool)> {
        self.studies.iter().find_map(|s| {
            if let FootprintStudy::Imbalance {
                threshold,
                color_scale,
                ignore_zeros,
            } = s
            {
                Some((*threshold as usize, *color_scale, *ignore_zeros))
            } else {
                None
            }
        })
    }
}

impl Default for StudyRegistry {
    fn default() -> Self {
        Self::new()
    }
}
