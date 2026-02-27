//! Kline (candlestick) chart types — data points, footprint cells,
//! point-of-control, and naked POC markers.

use crate::domain::core::types::{Price, Timestamp};
use crate::domain::market::entities::Candle;

/// A candle with pre-computed total volume for rendering.
#[derive(Debug, Clone)]
pub struct KlineDataPoint {
    /// The underlying candle
    pub kline: Candle,
    /// Pre-computed total (buy + sell) volume
    pub total_volume: f32,
}

impl KlineDataPoint {
    /// Create from a candle, pre-computing total volume
    #[must_use]
    pub fn from_candle(candle: Candle) -> Self {
        Self {
            total_volume: (candle.buy_volume.0 + candle.sell_volume.0) as f32,
            kline: candle,
        }
    }
}

/// Per-candle footprint data — buy/sell volume at each traded price level.
#[derive(Debug, Clone, Default)]
pub struct KlineTrades {
    /// Volume split by price level within this candle
    pub trades: Vec<TradeCell>,
}

/// A single price level within a footprint candle.
#[derive(Debug, Clone)]
pub struct TradeCell {
    /// Price level
    pub price: Price,
    /// Buy-side volume at this level
    pub buy_volume: f32,
    /// Sell-side volume at this level
    pub sell_volume: f32,
}

/// Point of control — the price level with the highest volume in a candle.
#[derive(Debug, Clone, Copy)]
pub struct PointOfControl {
    /// POC price level
    pub price: Price,
    /// Volume at the POC
    pub volume: f32,
}

/// Naked point of control — a POC that has not been revisited by price.
#[derive(Debug, Clone, Copy)]
pub struct NPoc {
    /// Price of the naked POC
    pub price: Price,
    /// Timestamp of the candle that produced the POC
    pub time: Timestamp,
}
