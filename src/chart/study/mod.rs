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
