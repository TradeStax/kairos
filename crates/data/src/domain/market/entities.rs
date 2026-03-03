//! Core market data entities.
//!
//! The [`Depth`] type uses `BTreeMap<i64, f32>` (raw price units to quantity)
//! for performance, with typed accessors that accept/return [`Price`].

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::domain::core::types::{Price, Quantity, Side, Timestamp, Volume};

// ── Trade ───────────────────────────────────────────────────────────────

/// A single trade execution.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    /// Execution timestamp
    pub time: Timestamp,
    /// Execution price
    pub price: Price,
    /// Trade quantity
    pub quantity: Quantity,
    /// Aggressor side
    pub side: Side,
}

impl Trade {
    /// Create a new trade
    #[must_use]
    pub fn new(time: Timestamp, price: Price, quantity: Quantity, side: Side) -> Self {
        Self {
            time,
            price,
            quantity,
            side,
        }
    }

    /// Create from raw numeric values (convenience for adapter layers)
    #[must_use]
    pub fn from_raw(time_millis: u64, price_f64: f64, quantity_f32: f32, is_sell: bool) -> Self {
        Self {
            time: Timestamp::from_millis(time_millis),
            price: Price::from_f64(price_f64),
            quantity: Quantity(quantity_f32 as f64),
            side: if is_sell { Side::Sell } else { Side::Buy },
        }
    }

    /// Return `true` if the aggressor was a buyer
    #[must_use]
    pub fn is_buy(&self) -> bool {
        self.side.is_buy()
    }

    /// Return `true` if the aggressor was a seller
    #[must_use]
    pub fn is_sell(&self) -> bool {
        self.side.is_sell()
    }

    /// Return `true` if this trade occurred on the given calendar date (UTC)
    #[must_use]
    pub fn is_on_date(&self, date: NaiveDate) -> bool {
        self.time.to_date() == date
    }
}

// ── Candle ──────────────────────────────────────────────────────────────

/// OHLCV candle with separate buy and sell volume.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Candle {
    /// Candle open timestamp
    pub time: Timestamp,
    /// Open price
    pub open: Price,
    /// High price
    pub high: Price,
    /// Low price
    pub low: Price,
    /// Close price
    pub close: Price,
    /// Volume from buy-side aggression
    pub buy_volume: Volume,
    /// Volume from sell-side aggression
    pub sell_volume: Volume,
}

impl Candle {
    /// Return total volume as `f32`
    #[must_use]
    pub fn volume(&self) -> f32 {
        (self.buy_volume.0 + self.sell_volume.0) as f32
    }

    /// Create a validated candle.
    ///
    /// Returns an error if `high < open|close` or `low > open|close`.
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

    /// Return combined buy + sell volume
    #[must_use]
    pub fn total_volume(&self) -> Volume {
        self.buy_volume + self.sell_volume
    }

    /// Return buy volume minus sell volume
    #[must_use]
    pub fn volume_delta(&self) -> f64 {
        self.buy_volume.value() - self.sell_volume.value()
    }

    /// Return absolute difference between open and close
    #[must_use]
    pub fn body_size(&self) -> Price {
        if self.close >= self.open {
            self.close - self.open
        } else {
            self.open - self.close
        }
    }

    /// Return high minus low
    #[must_use]
    pub fn range(&self) -> Price {
        self.high - self.low
    }

    /// Return `true` if close > open
    #[must_use]
    pub fn is_bullish(&self) -> bool {
        self.close > self.open
    }

    /// Return `true` if close < open
    #[must_use]
    pub fn is_bearish(&self) -> bool {
        self.close < self.open
    }

    /// Convert to a raw tuple `(time_ms, open, high, low, close, (buy_vol, sell_vol))`
    #[must_use]
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
/// Uses `BTreeMap<i64, f32>` (raw price units to quantity) for performance.
/// Typed accessors accept/return [`Price`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Depth {
    /// Snapshot timestamp in milliseconds since epoch
    pub time: u64,
    /// Bid levels: price_units -> quantity
    pub bids: BTreeMap<i64, f32>,
    /// Ask levels: price_units -> quantity
    pub asks: BTreeMap<i64, f32>,
}

impl Depth {
    /// Create an empty depth snapshot at the given timestamp
    #[must_use]
    pub fn new(time: u64) -> Self {
        Self {
            time,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    /// Return the best (highest) bid price and quantity
    #[must_use]
    pub fn best_bid(&self) -> Option<(Price, f32)> {
        self.bids
            .iter()
            .next_back()
            .map(|(pu, qty)| (Price::from_units(*pu), *qty))
    }

    /// Return the best (lowest) ask price and quantity
    #[must_use]
    pub fn best_ask(&self) -> Option<(Price, f32)> {
        self.asks
            .iter()
            .next()
            .map(|(pu, qty)| (Price::from_units(*pu), *qty))
    }

    /// Return bid quantity at `price`, or 0.0 if absent
    #[must_use]
    pub fn get_bid_qty(&self, price: Price) -> f32 {
        self.bids.get(&price.units()).copied().unwrap_or(0.0)
    }

    /// Return ask quantity at `price`, or 0.0 if absent
    #[must_use]
    pub fn get_ask_qty(&self, price: Price) -> f32 {
        self.asks.get(&price.units()).copied().unwrap_or(0.0)
    }

    /// Insert or remove a bid level (removes if `qty` is zero)
    pub fn update_bid(&mut self, price: Price, qty: f32) {
        debug_assert!(qty >= 0.0, "bid qty must be >= 0, got {}", qty);
        if qty > 0.0 {
            self.bids.insert(price.units(), qty);
        } else {
            self.bids.remove(&price.units());
        }
    }

    /// Insert or remove an ask level (removes if `qty` is zero)
    pub fn update_ask(&mut self, price: Price, qty: f32) {
        debug_assert!(qty >= 0.0, "ask qty must be >= 0, got {}", qty);
        if qty > 0.0 {
            self.asks.insert(price.units(), qty);
        } else {
            self.asks.remove(&price.units());
        }
    }

    /// Return the top `n` bid levels (highest price first)
    #[must_use]
    pub fn top_bids(&self, n: usize) -> Vec<(Price, f32)> {
        self.bids
            .iter()
            .rev()
            .take(n)
            .map(|(pu, qty)| (Price::from_units(*pu), *qty))
            .collect()
    }

    /// Return the top `n` ask levels (lowest price first)
    #[must_use]
    pub fn top_asks(&self, n: usize) -> Vec<(Price, f32)> {
        self.asks
            .iter()
            .take(n)
            .map(|(pu, qty)| (Price::from_units(*pu), *qty))
            .collect()
    }

    /// Return the mid price `(best_ask + best_bid) / 2`
    #[must_use]
    pub fn mid_price(&self) -> Option<Price> {
        match (self.best_ask(), self.best_bid()) {
            (Some((ask, _)), Some((bid, _))) => Some((ask + bid) / 2),
            _ => None,
        }
    }

    /// Return the spread `best_ask - best_bid`
    #[must_use]
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

/// Tagged union of all market data types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MarketData {
    /// A single trade execution
    Trade(Trade),
    /// An OHLCV candle
    Candle(Candle),
    /// An order book snapshot
    Depth(Depth),
}

impl MarketData {
    /// Return the timestamp of the contained data
    #[must_use]
    pub fn timestamp(&self) -> Timestamp {
        match self {
            MarketData::Trade(t) => t.time,
            MarketData::Candle(c) => c.time,
            MarketData::Depth(d) => Timestamp::from_millis(d.time),
        }
    }

    /// Try to extract the inner [`Trade`]
    #[must_use]
    pub fn as_trade(&self) -> Option<&Trade> {
        match self {
            MarketData::Trade(t) => Some(t),
            _ => None,
        }
    }

    /// Try to extract the inner [`Candle`]
    #[must_use]
    pub fn as_candle(&self) -> Option<&Candle> {
        match self {
            MarketData::Candle(c) => Some(c),
            _ => None,
        }
    }

    /// Try to extract the inner [`Depth`]
    #[must_use]
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
    fn trade_from_raw_buy() {
        let trade = Trade::from_raw(5000, 4523.75, 3.0, false);
        assert!(trade.is_buy());
        assert_eq!(trade.time.to_millis(), 5000);
        assert!((trade.price.to_f64() - 4523.75).abs() < 0.01);
        assert!((trade.quantity.0 - 3.0).abs() < 0.01);
    }

    #[test]
    fn trade_from_raw_sell() {
        let trade = Trade::from_raw(1000, 100.0, 10.0, true);
        assert!(trade.is_sell());
        assert!(!trade.is_buy());
    }

    #[test]
    fn trade_is_on_date() {
        // 2025-01-15 00:00:00 UTC = 1736899200000 ms
        let trade = Trade::from_raw(1736899200000, 100.0, 1.0, false);
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
        assert!(trade.is_on_date(date));
        let other_date = NaiveDate::from_ymd_opt(2025, 1, 16).unwrap();
        assert!(!trade.is_on_date(other_date));
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
    fn candle_new_high_below_open_returns_error() {
        let result = Candle::new(
            Timestamp::from_millis(1000),
            Price::from_f32(105.0),
            Price::from_f32(104.0), // high < open
            Price::from_f32(99.0),
            Price::from_f32(102.0),
            Volume(10.0),
            Volume(10.0),
        );
        assert!(result.is_err());
    }

    #[test]
    fn candle_new_high_below_close_returns_error() {
        let result = Candle::new(
            Timestamp::from_millis(1000),
            Price::from_f32(100.0),
            Price::from_f32(101.0), // high < close
            Price::from_f32(99.0),
            Price::from_f32(102.0),
            Volume(10.0),
            Volume(10.0),
        );
        assert!(result.is_err());
    }

    #[test]
    fn candle_new_low_above_open_returns_error() {
        let result = Candle::new(
            Timestamp::from_millis(1000),
            Price::from_f32(100.0),
            Price::from_f32(105.0),
            Price::from_f32(101.0), // low > open
            Price::from_f32(103.0),
            Volume(10.0),
            Volume(10.0),
        );
        assert!(result.is_err());
    }

    #[test]
    fn candle_new_low_above_close_returns_error() {
        let result = Candle::new(
            Timestamp::from_millis(1000),
            Price::from_f32(103.0),
            Price::from_f32(105.0),
            Price::from_f32(102.0), // low > close at 101
            Price::from_f32(101.0),
            Volume(10.0),
            Volume(10.0),
        );
        assert!(result.is_err());
    }

    #[test]
    fn candle_bearish_and_body_size() {
        let candle = Candle::new(
            Timestamp::from_millis(1000),
            Price::from_f32(105.0),
            Price::from_f32(108.0),
            Price::from_f32(99.0),
            Price::from_f32(100.0),
            Volume(20.0),
            Volume(80.0),
        )
        .unwrap();

        assert!(candle.is_bearish());
        assert!(!candle.is_bullish());
        assert_eq!(candle.body_size().to_f32(), 5.0);
        assert_eq!(candle.range().to_f32(), 9.0);
        assert!((candle.volume_delta() - (-60.0)).abs() < 0.01);
    }

    #[test]
    fn candle_to_raw_tuple() {
        let candle = Candle::new(
            Timestamp::from_millis(5000),
            Price::from_f32(100.0),
            Price::from_f32(102.0),
            Price::from_f32(99.0),
            Price::from_f32(101.0),
            Volume(30.0),
            Volume(20.0),
        )
        .unwrap();

        let (t, o, h, l, c, (bv, sv)) = candle.to_raw_tuple();
        assert_eq!(t, 5000);
        assert_eq!(o, 100.0);
        assert_eq!(h, 102.0);
        assert_eq!(l, 99.0);
        assert_eq!(c, 101.0);
        assert!((bv - 30.0).abs() < 0.01);
        assert!((sv - 20.0).abs() < 0.01);
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

    #[test]
    fn depth_empty_book_returns_none() {
        let depth = Depth::new(0);
        assert!(depth.best_bid().is_none());
        assert!(depth.best_ask().is_none());
        assert!(depth.mid_price().is_none());
        assert!(depth.spread().is_none());
        assert!(depth.top_bids(5).is_empty());
        assert!(depth.top_asks(5).is_empty());
    }

    #[test]
    fn depth_bid_only_no_mid_or_spread() {
        let mut depth = Depth::new(0);
        depth.update_bid(Price::from_f32(100.0), 10.0);
        assert!(depth.best_bid().is_some());
        assert!(depth.best_ask().is_none());
        assert!(depth.mid_price().is_none());
        assert!(depth.spread().is_none());
    }

    #[test]
    fn depth_get_qty_at_missing_price_returns_zero() {
        let depth = Depth::new(0);
        assert_eq!(depth.get_bid_qty(Price::from_f32(100.0)), 0.0);
        assert_eq!(depth.get_ask_qty(Price::from_f32(100.0)), 0.0);
    }

    #[test]
    fn depth_top_asks_sorted_ascending() {
        let mut depth = Depth::new(0);
        depth.update_ask(Price::from_f32(105.0), 5.0);
        depth.update_ask(Price::from_f32(101.0), 15.0);
        depth.update_ask(Price::from_f32(103.0), 10.0);

        let top = depth.top_asks(3);
        assert_eq!(top.len(), 3);
        assert_eq!(top[0].0.to_f32(), 101.0); // lowest ask first
        assert_eq!(top[1].0.to_f32(), 103.0);
        assert_eq!(top[2].0.to_f32(), 105.0);
    }

    #[test]
    fn market_data_timestamp() {
        let trade = MarketData::Trade(Trade::from_raw(5000, 100.0, 1.0, false));
        assert_eq!(trade.timestamp().to_millis(), 5000);
        assert!(trade.as_trade().is_some());
        assert!(trade.as_candle().is_none());
        assert!(trade.as_depth().is_none());
    }
}
