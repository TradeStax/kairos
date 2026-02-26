//! Domain Entities
//!
//! Core business entities. The `Depth` type uses `BTreeMap<i64, f32>` (raw
//! price units → quantity) for performance, with typed accessors.

use crate::Error;
use crate::domain::core::types::{Price, Quantity, Side, Timestamp, Volume};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ── Trade ───────────────────────────────────────────────────────────────

/// Single trade execution
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    pub time: Timestamp,
    pub price: Price,
    pub quantity: Quantity,
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
    pub fn from_raw(time_millis: u64, price_f64: f64, quantity_f32: f32, is_sell: bool) -> Self {
        Self {
            time: Timestamp::from_millis(time_millis),
            price: Price::from_f64(price_f64),
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

    pub fn is_on_date(&self, date: NaiveDate) -> bool {
        self.time.to_date() == date
    }
}

// ── Candle ──────────────────────────────────────────────────────────────

/// OHLCV Candle
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Candle {
    pub time: Timestamp,
    pub open: Price,
    pub high: Price,
    pub low: Price,
    pub close: Price,
    pub buy_volume: Volume,
    pub sell_volume: Volume,
}

impl Candle {
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
    ) -> Result<Self, Error> {
        if high < open || high < close {
            return Err(Error::Validation(format!(
                "Candle high ({}) must be >= open ({}) and close ({})",
                high, open, close
            )));
        }
        if low > open || low > close {
            return Err(Error::Validation(format!(
                "Candle low ({}) must be <= open ({}) and close ({})",
                low, open, close
            )));
        }
        Ok(Self {
            time,
            open,
            high,
            low,
            close,
            buy_volume,
            sell_volume,
        })
    }

    pub fn total_volume(&self) -> Volume {
        self.buy_volume + self.sell_volume
    }

    pub fn volume_delta(&self) -> f64 {
        self.buy_volume.value() - self.sell_volume.value()
    }

    pub fn body_size(&self) -> Price {
        if self.close >= self.open {
            self.close - self.open
        } else {
            self.open - self.close
        }
    }

    pub fn range(&self) -> Price {
        self.high - self.low
    }

    pub fn is_bullish(&self) -> bool {
        self.close > self.open
    }

    pub fn is_bearish(&self) -> bool {
        self.close < self.open
    }

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

// ── Depth ───────────────────────────────────────────────────────────────

/// Order book depth snapshot.
///
/// Uses `BTreeMap<i64, f32>` (raw price units → quantity) for performance.
/// Typed accessors accept/return `Price`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Depth {
    pub time: u64,                // millis since epoch
    pub bids: BTreeMap<i64, f32>, // price_units → quantity
    pub asks: BTreeMap<i64, f32>,
}

impl Depth {
    pub fn new(time: u64) -> Self {
        Self {
            time,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    pub fn best_bid(&self) -> Option<(Price, f32)> {
        self.bids
            .iter()
            .next_back()
            .map(|(pu, qty)| (Price::from_units(*pu), *qty))
    }

    pub fn best_ask(&self) -> Option<(Price, f32)> {
        self.asks
            .iter()
            .next()
            .map(|(pu, qty)| (Price::from_units(*pu), *qty))
    }

    pub fn get_bid_qty(&self, price: Price) -> f32 {
        self.bids.get(&price.units()).copied().unwrap_or(0.0)
    }

    pub fn get_ask_qty(&self, price: Price) -> f32 {
        self.asks.get(&price.units()).copied().unwrap_or(0.0)
    }

    pub fn update_bid(&mut self, price: Price, qty: f32) {
        debug_assert!(qty >= 0.0, "bid qty must be >= 0, got {}", qty);
        if qty > 0.0 {
            self.bids.insert(price.units(), qty);
        } else {
            self.bids.remove(&price.units());
        }
    }

    pub fn update_ask(&mut self, price: Price, qty: f32) {
        debug_assert!(qty >= 0.0, "ask qty must be >= 0, got {}", qty);
        if qty > 0.0 {
            self.asks.insert(price.units(), qty);
        } else {
            self.asks.remove(&price.units());
        }
    }

    /// Top N bid levels (highest price first)
    pub fn top_bids(&self, n: usize) -> Vec<(Price, f32)> {
        self.bids
            .iter()
            .rev()
            .take(n)
            .map(|(pu, qty)| (Price::from_units(*pu), *qty))
            .collect()
    }

    /// Top N ask levels (lowest price first)
    pub fn top_asks(&self, n: usize) -> Vec<(Price, f32)> {
        self.asks
            .iter()
            .take(n)
            .map(|(pu, qty)| (Price::from_units(*pu), *qty))
            .collect()
    }

    /// Mid price
    pub fn mid_price(&self) -> Option<Price> {
        match (self.best_ask(), self.best_bid()) {
            (Some((ask, _)), Some((bid, _))) => Some((ask + bid) / 2),
            _ => None,
        }
    }

    /// Spread
    pub fn spread(&self) -> Option<Price> {
        match (self.best_ask(), self.best_bid()) {
            (Some((ask, _)), Some((bid, _))) => Some(ask - bid),
            _ => None,
        }
    }
}

impl Default for Depth {
    fn default() -> Self {
        Self::new(0)
    }
}

// ── MarketData ──────────────────────────────────────────────────────────

/// Market data type (union of all possible data types)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MarketData {
    Trade(Trade),
    Candle(Candle),
    Depth(Depth),
}

impl MarketData {
    pub fn timestamp(&self) -> Timestamp {
        match self {
            MarketData::Trade(t) => t.time,
            MarketData::Candle(c) => c.time,
            MarketData::Depth(d) => Timestamp::from_millis(d.time),
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

    pub fn as_depth(&self) -> Option<&Depth> {
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
        )
        .expect("invariant: valid OHLC values");

        assert_eq!(candle.total_volume().value(), 180.0);
        assert_eq!(candle.volume_delta(), 20.0);
        assert!(candle.is_bullish());
    }

    #[test]
    fn test_depth() {
        let mut depth = Depth::new(1000);
        depth.update_bid(Price::from_f32(100.0), 10.0);
        depth.update_bid(Price::from_f32(99.5), 20.0);
        depth.update_ask(Price::from_f32(100.5), 15.0);
        depth.update_ask(Price::from_f32(101.0), 25.0);

        assert_eq!(depth.best_bid().unwrap().0.to_f32(), 100.0);
        assert_eq!(depth.best_ask().unwrap().0.to_f32(), 100.5);
        assert_eq!(depth.mid_price().unwrap().to_f32(), 100.25);
        assert_eq!(depth.spread().unwrap().to_f32(), 0.5);

        let top = depth.top_bids(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].0.to_f32(), 100.0);
        assert_eq!(top[1].0.to_f32(), 99.5);
    }

    #[test]
    fn test_depth_remove_zero_qty() {
        let mut depth = Depth::new(0);
        depth.update_bid(Price::from_f32(100.0), 10.0);
        assert_eq!(depth.bids.len(), 1);

        depth.update_bid(Price::from_f32(100.0), 0.0);
        assert!(depth.bids.is_empty());
    }
}
