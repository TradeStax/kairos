//! Domain Entities
//!
//! Core business entities that represent market data.
//! These are immutable data structures with business logic methods.

use super::types::{Price, Quantity, Side, Timestamp, Volume};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Single trade execution
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    /// Trade execution time (milliseconds since epoch)
    pub time: Timestamp,
    /// Trade price
    pub price: Price,
    /// Trade quantity
    pub quantity: Quantity,
    /// Trade side (buy or sell aggressor)
    pub side: Side,
}

impl Trade {
    pub fn new(time: Timestamp, price: Price, quantity: Quantity, side: Side) -> Self {
        Self {
            time,
            price,
            quantity,
            side,
        }
    }

    /// Create from raw values (for compatibility)
    pub fn from_raw(time_millis: u64, price_f32: f32, quantity_f32: f32, is_sell: bool) -> Self {
        Self {
            time: Timestamp::from_millis(time_millis),
            price: Price::from_f32(price_f32),
            quantity: Quantity(quantity_f32 as f64),
            side: if is_sell { Side::Sell } else { Side::Buy },
        }
    }

    pub fn is_buy(&self) -> bool {
        self.side.is_buy()
    }

    pub fn is_sell(&self) -> bool {
        self.side.is_sell()
    }

    /// Check if trade occurred on a specific date (UTC)
    pub fn is_on_date(&self, date: NaiveDate) -> bool {
        self.time.to_date() == date
    }
}

/// OHLCV Candle (time-based or tick-based aggregation)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Candle {
    /// Candle timestamp (start of period)
    pub time: Timestamp,
    /// Open price (first trade)
    pub open: Price,
    /// High price (maximum)
    pub high: Price,
    /// Low price (minimum)
    pub low: Price,
    /// Close price (last trade)
    pub close: Price,
    /// Buy volume
    pub buy_volume: Volume,
    /// Sell volume
    pub sell_volume: Volume,
}

impl Candle {
    /// Get total volume (buy + sell)
    pub fn volume(&self) -> f32 {
        (self.buy_volume.0 + self.sell_volume.0) as f32
    }

    pub fn new(
        time: Timestamp,
        open: Price,
        high: Price,
        low: Price,
        close: Price,
        buy_volume: Volume,
        sell_volume: Volume,
    ) -> Self {
        assert!(
            high >= open && high >= close,
            "High must be >= open and close"
        );
        assert!(low <= open && low <= close, "Low must be <= open and close");

        Self {
            time,
            open,
            high,
            low,
            close,
            buy_volume,
            sell_volume,
        }
    }

    /// Total volume (buy + sell)
    pub fn total_volume(&self) -> Volume {
        self.buy_volume + self.sell_volume
    }

    /// Volume delta (buy - sell)
    pub fn volume_delta(&self) -> f64 {
        self.buy_volume.value() - self.sell_volume.value()
    }

    /// Candle body size (|close - open|)
    pub fn body_size(&self) -> Price {
        if self.close >= self.open {
            self.close - self.open
        } else {
            self.open - self.close
        }
    }

    /// Candle range (high - low)
    pub fn range(&self) -> Price {
        self.high - self.low
    }

    /// Is bullish candle (close > open)
    pub fn is_bullish(&self) -> bool {
        self.close > self.open
    }

    /// Is bearish candle (close < open)
    pub fn is_bearish(&self) -> bool {
        self.close < self.open
    }

    /// Convert to raw tuple (for compatibility with existing code)
    pub fn to_raw_tuple(&self) -> (u64, f32, f32, f32, f32, (f32, f32)) {
        (
            self.time.to_millis(),
            self.open.to_f32(),
            self.high.to_f32(),
            self.low.to_f32(),
            self.close.to_f32(),
            (
                self.buy_volume.value() as f32,
                self.sell_volume.value() as f32,
            ),
        )
    }
}

/// Order book depth snapshot
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DepthSnapshot {
    /// Snapshot timestamp
    pub time: Timestamp,
    /// Bid levels (price -> quantity)
    pub bids: BTreeMap<Price, Quantity>,
    /// Ask levels (price -> quantity)
    pub asks: BTreeMap<Price, Quantity>,
}

impl DepthSnapshot {
    pub fn new(
        time: Timestamp,
        bids: BTreeMap<Price, Quantity>,
        asks: BTreeMap<Price, Quantity>,
    ) -> Self {
        Self { time, bids, asks }
    }

    /// Get best bid (highest bid price)
    pub fn best_bid(&self) -> Option<(Price, Quantity)> {
        self.bids.last_key_value().map(|(p, q)| (*p, *q))
    }

    /// Get best ask (lowest ask price)
    pub fn best_ask(&self) -> Option<(Price, Quantity)> {
        self.asks.first_key_value().map(|(p, q)| (*p, *q))
    }

    /// Get mid price
    pub fn mid_price(&self) -> Option<Price> {
        match (self.best_ask(), self.best_bid()) {
            (Some((ask, _)), Some((bid, _))) => Some((ask + bid) / 2),
            _ => None,
        }
    }

    /// Get spread
    pub fn spread(&self) -> Option<Price> {
        match (self.best_ask(), self.best_bid()) {
            (Some((ask, _)), Some((bid, _))) => Some(ask - bid),
            _ => None,
        }
    }

    /// Total bid volume
    pub fn total_bid_volume(&self) -> Quantity {
        self.bids.values().fold(Quantity::zero(), |acc, q| acc + *q)
    }

    /// Total ask volume
    pub fn total_ask_volume(&self) -> Quantity {
        self.asks.values().fold(Quantity::zero(), |acc, q| acc + *q)
    }

    /// Check if snapshot occurred on a specific date (UTC)
    pub fn is_on_date(&self, date: NaiveDate) -> bool {
        self.time.to_date() == date
    }
}

/// Market data type (union of all possible data types)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MarketData {
    Trade(Trade),
    Candle(Candle),
    Depth(DepthSnapshot),
}

impl MarketData {
    pub fn timestamp(&self) -> Timestamp {
        match self {
            MarketData::Trade(t) => t.time,
            MarketData::Candle(c) => c.time,
            MarketData::Depth(d) => d.time,
        }
    }

    pub fn as_trade(&self) -> Option<&Trade> {
        match self {
            MarketData::Trade(t) => Some(t),
            _ => None,
        }
    }

    pub fn as_candle(&self) -> Option<&Candle> {
        match self {
            MarketData::Candle(c) => Some(c),
            _ => None,
        }
    }

    pub fn as_depth(&self) -> Option<&DepthSnapshot> {
        match self {
            MarketData::Depth(d) => Some(d),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trade_creation() {
        let trade = Trade::new(
            Timestamp::from_millis(1000),
            Price::from_f32(100.25),
            Quantity(10.0),
            Side::Buy,
        );

        assert!(trade.is_buy());
        assert!(!trade.is_sell());
        assert_eq!(trade.price.to_f32(), 100.25);
    }

    #[test]
    fn test_candle_creation() {
        let candle = Candle::new(
            Timestamp::from_millis(1000),
            Price::from_f32(100.0),
            Price::from_f32(105.0),
            Price::from_f32(99.0),
            Price::from_f32(102.0),
            Volume(100.0),
            Volume(80.0),
        );

        assert_eq!(candle.total_volume().value(), 180.0);
        assert_eq!(candle.volume_delta(), 20.0);
        assert!(candle.is_bullish());
    }

    #[test]
    fn test_depth_snapshot() {
        let mut bids = BTreeMap::new();
        bids.insert(Price::from_f32(100.0), Quantity(10.0));
        bids.insert(Price::from_f32(99.5), Quantity(20.0));

        let mut asks = BTreeMap::new();
        asks.insert(Price::from_f32(100.5), Quantity(15.0));
        asks.insert(Price::from_f32(101.0), Quantity(25.0));

        let depth = DepthSnapshot::new(Timestamp::from_millis(1000), bids, asks);

        assert_eq!(depth.best_bid().unwrap().0.to_f32(), 100.0);
        assert_eq!(depth.best_ask().unwrap().0.to_f32(), 100.5);
        assert_eq!(depth.mid_price().unwrap().to_f32(), 100.25);
        assert_eq!(depth.spread().unwrap().to_f32(), 0.5);
    }
}
