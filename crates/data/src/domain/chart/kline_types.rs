//! Kline (candlestick) chart types

use crate::domain::entities::Candle;
use crate::domain::types::Timestamp;

/// Kline data point (candle with metadata)
#[derive(Debug, Clone)]
pub struct KlineDataPoint {
    pub kline: Candle,
    pub total_volume: f32,
}

impl KlineDataPoint {
    pub fn from_candle(candle: Candle) -> Self {
        Self {
            total_volume: (candle.buy_volume.0 + candle.sell_volume.0) as f32,
            kline: candle,
        }
    }
}

/// Kline trades (for footprint charts)
#[derive(Debug, Clone, Default)]
pub struct KlineTrades {
    pub trades: Vec<TradeCell>,
}

#[derive(Debug, Clone)]
pub struct TradeCell {
    pub price: i64,
    pub buy_volume: f32,
    pub sell_volume: f32,
}

/// Point of control
#[derive(Debug, Clone, Copy)]
pub struct PointOfControl {
    pub price: i64,
    pub volume: f32,
}

/// Naked point of control
#[derive(Debug, Clone, Copy)]
pub struct NPoc {
    pub price: i64,
    pub time: Timestamp,
}
