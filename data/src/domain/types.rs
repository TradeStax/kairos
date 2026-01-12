//! Domain Value Objects
//!
//! Strongly-typed value objects that enforce business rules and constraints.
//! These types are immutable and self-validating.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::ops::{Add, Div, Mul, Sub};

/// Price with fixed precision (10^-8 for compatibility with exchange layer)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Price {
    units: i64, // Price in 10^-8 units
}

impl Price {
    pub const PRECISION: i64 = 100_000_000; // 10^8
    const PRECISION_F64: f64 = Self::PRECISION as f64;

    /// Create from units (internal representation)
    pub fn from_units(units: i64) -> Self {
        Self { units }
    }

    /// Create from f32
    pub fn from_f32(value: f32) -> Self {
        Self {
            units: (value as f64 * Self::PRECISION_F64).round() as i64,
        }
    }

    /// Create from f64
    pub fn from_f64(value: f64) -> Self {
        Self {
            units: (value * Self::PRECISION_F64).round() as i64,
        }
    }

    /// Convert to f32
    pub fn to_f32(self) -> f32 {
        (self.units as f64 / Self::PRECISION_F64) as f32
    }

    /// Convert to f64
    pub fn to_f64(self) -> f64 {
        self.units as f64 / Self::PRECISION_F64
    }

    /// Get internal units
    pub fn units(self) -> i64 {
        self.units
    }

    /// Alias for units() - for compatibility
    pub fn to_units(self) -> i64 {
        self.units
    }

    /// Round to tick size
    pub fn round_to_tick(self, tick_size: Price) -> Self {
        let ticks = (self.units + tick_size.units / 2) / tick_size.units;
        Self {
            units: ticks * tick_size.units,
        }
    }

    /// Round to step (tick size)
    pub fn round_to_step(self, step: Price) -> Self {
        self.round_to_tick(step)
    }

    /// Round based on side (sells floor, buys ceil)
    pub fn round_to_side_step(self, is_sell_or_bid: bool, step: Price) -> Self {
        if step.units <= 1 {
            return self;
        }

        if is_sell_or_bid {
            // Floor for sells/bids
            let floored = (self.units.div_euclid(step.units)) * step.units;
            Self { units: floored }
        } else {
            // Ceil for buys/asks
            let added = self.units.saturating_add(step.units - 1);
            let ceiled = (added.div_euclid(step.units)) * step.units;
            Self { units: ceiled }
        }
    }

    /// Add N steps to price
    pub fn add_steps(self, steps: i64, step: Price) -> Self {
        Self {
            units: self.units.saturating_add(steps.saturating_mul(step.units)),
        }
    }

    /// Number of steps between two prices
    pub fn steps_between_inclusive(low: Price, high: Price, step: Price) -> Option<usize> {
        if high.units < low.units || step.units <= 0 {
            return None;
        }
        let span = high.units.checked_sub(low.units)?;
        Some((span / step.units) as usize + 1)
    }

    /// Zero price
    pub const fn zero() -> Self {
        Self { units: 0 }
    }
}

impl Add for Price {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            units: self.units + other.units,
        }
    }
}

impl Sub for Price {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self {
            units: self.units - other.units,
        }
    }
}

impl Mul<f64> for Price {
    type Output = Self;
    fn mul(self, scalar: f64) -> Self {
        Self {
            units: (self.units as f64 * scalar).round() as i64,
        }
    }
}

impl Div<i64> for Price {
    type Output = Self;
    fn div(self, divisor: i64) -> Self {
        Self {
            units: self.units / divisor,
        }
    }
}

impl std::fmt::Display for Price {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.8}", self.to_f64())
    }
}

/// Volume (quantity traded)
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Volume(pub f64);

impl Volume {
    pub fn zero() -> Self {
        Self(0.0)
    }

    pub fn value(&self) -> f64 {
        self.0
    }
}

impl Add for Volume {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl std::fmt::Display for Volume {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2}", self.0)
    }
}

/// Quantity (position size, order size)
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Quantity(pub f64);

impl Quantity {
    pub fn zero() -> Self {
        Self(0.0)
    }

    pub fn value(&self) -> f64 {
        self.0
    }
}

impl Add for Quantity {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

/// Timestamp in milliseconds since epoch
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp(pub u64);

impl Timestamp {
    pub fn from_millis(millis: u64) -> Self {
        Self(millis)
    }

    pub fn to_millis(self) -> u64 {
        self.0
    }

    pub fn to_datetime(self) -> DateTime<Utc> {
        DateTime::from_timestamp((self.0 / 1000) as i64, ((self.0 % 1000) * 1_000_000) as u32)
            .unwrap_or_else(|| DateTime::from_timestamp(0, 0).unwrap())
    }

    pub fn to_date(self) -> NaiveDate {
        self.to_datetime().date_naive()
    }

    pub fn from_datetime(dt: DateTime<Utc>) -> Self {
        Self(dt.timestamp_millis() as u64)
    }
}

/// Time range (start, end) in milliseconds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: Timestamp,
    pub end: Timestamp,
}

impl TimeRange {
    pub fn new(start: Timestamp, end: Timestamp) -> Self {
        assert!(start <= end, "Invalid time range: start > end");
        Self { start, end }
    }

    pub fn contains(&self, timestamp: Timestamp) -> bool {
        timestamp >= self.start && timestamp <= self.end
    }

    pub fn duration_millis(&self) -> u64 {
        self.end.0 - self.start.0
    }
}

/// Date range (start date, end date)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateRange {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

impl DateRange {
    pub fn new(start: NaiveDate, end: NaiveDate) -> Self {
        assert!(start <= end, "Invalid date range: start > end");
        Self { start, end }
    }

    /// Create a date range for the last N days (ending today)
    pub fn last_n_days(n: i64) -> Self {
        let end = chrono::Utc::now().date_naive();
        let start = end - chrono::Duration::days(n - 1);
        Self { start, end }
    }

    /// Iterate over all dates in range (inclusive)
    pub fn dates(&self) -> impl Iterator<Item = NaiveDate> {
        let start = self.start;
        let end = self.end;
        (0..=(end - start).num_days()).map(move |days| start + chrono::Duration::days(days))
    }

    pub fn num_days(&self) -> i64 {
        (self.end - self.start).num_days() + 1
    }

    pub fn contains(&self, date: NaiveDate) -> bool {
        date >= self.start && date <= self.end
    }
}

impl Default for DateRange {
    /// Default date range: last 30 days ending today
    fn default() -> Self {
        Self::last_n_days(30)
    }
}

/// Trade side (buy or sell)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
    // Orderbook-specific sides
    Bid,
    Ask,
}

impl Side {
    pub fn is_buy(&self) -> bool {
        matches!(self, Side::Buy | Side::Bid)
    }

    pub fn is_sell(&self) -> bool {
        matches!(self, Side::Sell | Side::Ask)
    }

    /// Get index (0 for Bid, 1 for Ask)
    pub fn idx(&self) -> usize {
        match self {
            Side::Bid => 0,
            Side::Ask => 1,
            Side::Buy => 0,
            Side::Sell => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_conversion() {
        let price = Price::from_f32(100.25);
        assert_eq!(price.to_f32(), 100.25);

        let price = Price::from_f64(100.123456789);
        assert!((price.to_f64() - 100.123456789).abs() < 1e-8);
    }

    #[test]
    fn test_price_arithmetic() {
        let p1 = Price::from_f32(100.0);
        let p2 = Price::from_f32(50.0);

        assert_eq!((p1 + p2).to_f32(), 150.0);
        assert_eq!((p1 - p2).to_f32(), 50.0);
        assert_eq!((p2 * 2.0).to_f32(), 100.0);
    }

    #[test]
    fn test_price_rounding() {
        let price = Price::from_f32(100.127);
        let tick = Price::from_f32(0.25);
        let rounded = price.round_to_tick(tick);
        assert_eq!(rounded.to_f32(), 100.25);
    }

    #[test]
    fn test_date_range_iteration() {
        let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2025, 1, 3).unwrap();
        let range = DateRange::new(start, end);

        let dates: Vec<_> = range.dates().collect();
        assert_eq!(dates.len(), 3);
        assert_eq!(dates[0], start);
        assert_eq!(dates[2], end);
    }

    #[test]
    fn test_time_range_contains() {
        let range = TimeRange::new(Timestamp(1000), Timestamp(2000));
        assert!(range.contains(Timestamp(1500)));
        assert!(!range.contains(Timestamp(500)));
        assert!(!range.contains(Timestamp(2500)));
    }
}
