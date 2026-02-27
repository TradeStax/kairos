//! Strongly-typed value objects that enforce business rules and constraints.
//!
//! All monetary values use the fixed-point [`Price`] type (i64 with 10^-8
//! precision) to avoid floating-point rounding errors.

use std::ops::{Add, Div, Mul, Sub};

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::Error;

/// Unique identifier for a data feed / connection.
pub type FeedId = uuid::Uuid;

// ── Price ──────────────────────────────────────────────────────────────

/// Fixed-point price with 10^-8 precision.
///
/// Stored as an `i64` count of atomic units where each unit equals 10^-8
/// of the display price. This avoids floating-point rounding errors that
/// accumulate in order-book and P&L arithmetic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Price {
    /// Price in 10^-8 atomic units.
    units: i64,
}

impl Price {
    /// Number of atomic units per whole price unit (10^8).
    pub const PRECISION: i64 = 100_000_000;

    /// Pre-computed f64 form of [`Self::PRECISION`] to avoid repeated casts.
    const PRECISION_F64: f64 = Self::PRECISION as f64;

    /// Number of decimal places of the atomic unit (10^-8).
    pub const PRICE_SCALE: i32 = 8;

    /// Create from raw atomic units (internal representation)
    #[must_use]
    pub fn from_units(units: i64) -> Self {
        Self { units }
    }

    /// Create from `f32`, rounding to the nearest atomic unit
    #[must_use]
    pub fn from_f32(value: f32) -> Self {
        Self {
            units: (value as f64 * Self::PRECISION_F64).round() as i64,
        }
    }

    /// Create from `f64`, rounding to the nearest atomic unit
    #[must_use]
    pub fn from_f64(value: f64) -> Self {
        Self {
            units: (value * Self::PRECISION_F64).round() as i64,
        }
    }

    /// Convert to `f32` (lossy)
    #[must_use]
    pub fn to_f32(self) -> f32 {
        (self.units as f64 / Self::PRECISION_F64) as f32
    }

    /// Convert to `f64`
    #[must_use]
    pub fn to_f64(self) -> f64 {
        self.units as f64 / Self::PRECISION_F64
    }

    /// Return the raw atomic units
    #[must_use]
    pub fn units(self) -> i64 {
        self.units
    }

    /// Round to the nearest multiple of `tick_size`
    #[must_use]
    pub fn round_to_tick(self, tick_size: Price) -> Self {
        if tick_size.units == 0 {
            return self;
        }
        let ticks = (self.units + tick_size.units / 2) / tick_size.units;
        Self {
            units: ticks * tick_size.units,
        }
    }

    /// Round to step (alias for [`round_to_tick`](Self::round_to_tick))
    #[must_use]
    pub fn round_to_step(self, step: Price) -> Self {
        self.round_to_tick(step)
    }

    /// Round based on side: sells/bids floor, buys/asks ceil
    #[must_use]
    pub fn round_to_side_step(self, is_sell_or_bid: bool, step: Price) -> Self {
        if step.units <= 1 {
            return self;
        }

        if is_sell_or_bid {
            let floored = (self.units.div_euclid(step.units)) * step.units;
            Self { units: floored }
        } else {
            let added = self.units.saturating_add(step.units - 1);
            let ceiled = (added.div_euclid(step.units)) * step.units;
            Self { units: ceiled }
        }
    }

    /// Add `steps` tick increments to this price (saturating)
    #[must_use]
    pub fn add_steps(self, steps: i64, step: Price) -> Self {
        Self {
            units: self.units.saturating_add(steps.saturating_mul(step.units)),
        }
    }

    /// Return the number of tick steps between `low` and `high` (inclusive).
    ///
    /// Returns `None` if `high < low` or `step <= 0`.
    #[must_use]
    pub fn steps_between_inclusive(low: Price, high: Price, step: Price) -> Option<usize> {
        if high.units < low.units || step.units <= 0 {
            return None;
        }
        let span = high.units.checked_sub(low.units)?;
        Some((span / step.units) as usize + 1)
    }

    /// Checked addition — returns `None` on overflow
    #[must_use]
    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        self.units.checked_add(rhs.units).map(Self::from_units)
    }

    /// Checked subtraction — returns `None` on overflow
    #[must_use]
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.units.checked_sub(rhs.units).map(Self::from_units)
    }

    /// The additive identity (zero price)
    #[must_use]
    pub const fn zero() -> Self {
        Self { units: 0 }
    }
}

impl Add for Price {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            units: self.units.saturating_add(other.units),
        }
    }
}

impl Sub for Price {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self {
            units: self.units.saturating_sub(other.units),
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
            units: self.units.div_euclid(divisor),
        }
    }
}

impl std::fmt::Display for Price {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.8}", self.to_f64())
    }
}

// ── Volume ─────────────────────────────────────────────────────────────

/// Volume (quantity traded) as a floating-point value.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Volume(pub f64);

impl Volume {
    /// Zero volume
    #[must_use]
    pub fn zero() -> Self {
        Self(0.0)
    }

    /// Return the inner `f64` value
    #[must_use]
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

// ── Quantity ────────────────────────────────────────────────────────────

/// Quantity (position size, order size) as a floating-point value.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Quantity(pub f64);

impl Quantity {
    /// Zero quantity
    #[must_use]
    pub fn zero() -> Self {
        Self(0.0)
    }

    /// Return the inner `f64` value
    #[must_use]
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

// ── Timestamp ──────────────────────────────────────────────────────────

/// Timestamp in milliseconds since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp(pub u64);

impl Timestamp {
    /// Create from milliseconds since epoch
    #[must_use]
    pub fn from_millis(millis: u64) -> Self {
        Self(millis)
    }

    /// Return the raw millisecond value
    #[must_use]
    pub fn to_millis(self) -> u64 {
        self.0
    }

    /// Convert to a UTC `DateTime`
    #[must_use]
    pub fn to_datetime(self) -> DateTime<Utc> {
        DateTime::from_timestamp((self.0 / 1000) as i64, ((self.0 % 1000) * 1_000_000) as u32)
            .unwrap_or_else(|| DateTime::from_timestamp(0, 0).unwrap())
    }

    /// Convert to a `NaiveDate` (UTC)
    #[must_use]
    pub fn to_date(self) -> NaiveDate {
        self.to_datetime().date_naive()
    }

    /// Create from a UTC `DateTime`
    #[must_use]
    pub fn from_datetime(dt: DateTime<Utc>) -> Self {
        Self(dt.timestamp_millis() as u64)
    }
}

// ── TimeRange ──────────────────────────────────────────────────────────

/// Inclusive time range (`start..=end`) in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    /// Inclusive lower bound
    pub start: Timestamp,
    /// Inclusive upper bound
    pub end: Timestamp,
}

impl TimeRange {
    /// Create a validated time range.
    ///
    /// Returns an error if `start > end`.
    pub fn new(start: Timestamp, end: Timestamp) -> Result<Self, Error> {
        if start > end {
            return Err(Error::Validation(format!(
                "TimeRange start ({}) must be <= end ({})",
                start.0, end.0
            )));
        }
        Ok(Self { start, end })
    }

    /// Return whether `timestamp` falls within this range (inclusive)
    #[must_use]
    pub fn contains(&self, timestamp: Timestamp) -> bool {
        timestamp >= self.start && timestamp <= self.end
    }

    /// Return the duration in milliseconds
    #[must_use]
    pub fn duration_millis(&self) -> u64 {
        self.end.0 - self.start.0
    }
}

// ── DateRange ──────────────────────────────────────────────────────────

/// Inclusive date range (`start..=end`) as calendar dates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateRange {
    /// Inclusive start date
    pub start: NaiveDate,
    /// Inclusive end date
    pub end: NaiveDate,
}

impl DateRange {
    /// Create a validated date range.
    ///
    /// Returns an error if `start > end`.
    pub fn new(start: NaiveDate, end: NaiveDate) -> Result<Self, Error> {
        if start > end {
            return Err(Error::Validation(format!(
                "DateRange start ({start}) must be <= end ({end})"
            )));
        }
        Ok(Self { start, end })
    }

    /// Today's date in US Eastern Time (CME session date reference)
    #[must_use]
    pub fn today_et() -> NaiveDate {
        let et = chrono::FixedOffset::west_opt(5 * 3600).unwrap();
        chrono::Utc::now().with_timezone(&et).date_naive()
    }

    /// Create a date range covering today and the previous `n - 1` days
    #[must_use]
    pub fn last_n_days(n: i64) -> Self {
        let end = Self::today_et();
        let start = end - chrono::Duration::days(n - 1);
        Self { start, end }
    }

    /// Previous trading week (Monday through Friday before the current week)
    #[must_use]
    pub fn last_week() -> Self {
        use chrono::Weekday;
        let today = Self::today_et();
        let days_since_monday = today.weekday().num_days_from_monday() as i64;
        let prev_monday = today - chrono::Duration::days(days_since_monday + 7);
        let prev_friday = prev_monday + chrono::Duration::days(4);
        debug_assert_eq!(prev_monday.weekday(), Weekday::Mon);
        debug_assert_eq!(prev_friday.weekday(), Weekday::Fri);
        Self {
            start: prev_monday,
            end: prev_friday,
        }
    }

    /// Iterate over all dates in the range (inclusive)
    pub fn dates(&self) -> impl Iterator<Item = NaiveDate> {
        let start = self.start;
        let end = self.end;
        (0..=(end - start).num_days()).map(move |days| start + chrono::Duration::days(days))
    }

    /// Return the number of days in the range (inclusive)
    #[must_use]
    pub fn num_days(&self) -> i64 {
        (self.end - self.start).num_days() + 1
    }

    /// Return the duration in hours (days * 24)
    #[must_use]
    pub fn duration_hours(&self) -> i64 {
        self.num_days() * 24
    }

    /// Return whether `date` falls within this range (inclusive)
    #[must_use]
    pub fn contains(&self, date: NaiveDate) -> bool {
        date >= self.start && date <= self.end
    }

    /// Start of range as milliseconds since epoch (midnight UTC)
    #[must_use]
    pub fn start_timestamp_ms(&self) -> u64 {
        self.start
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis() as u64
    }

    /// End of range as milliseconds since epoch (end of day UTC)
    #[must_use]
    pub fn end_timestamp_ms(&self) -> u64 {
        self.end
            .and_hms_opt(23, 59, 59)
            .unwrap()
            .and_utc()
            .timestamp_millis() as u64
    }
}

impl Default for DateRange {
    /// Default date range: last 30 days ending today
    fn default() -> Self {
        Self::last_n_days(30)
    }
}

// ── Side ───────────────────────────────────────────────────────────────

/// Trade side (buy or sell) or book side (bid or ask).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
    Bid,
    Ask,
}

impl Side {
    /// Return `true` for `Buy` or `Bid`
    #[must_use]
    pub fn is_buy(&self) -> bool {
        matches!(self, Side::Buy | Side::Bid)
    }

    /// Return `true` for `Sell` or `Ask`
    #[must_use]
    pub fn is_sell(&self) -> bool {
        matches!(self, Side::Sell | Side::Ask)
    }

    /// Return index: 0 for Bid/Buy, 1 for Ask/Sell
    #[must_use]
    pub fn idx(&self) -> usize {
        match self {
            Side::Bid | Side::Buy => 0,
            Side::Ask | Side::Sell => 1,
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
    fn test_price_saturating_add() {
        let max = Price::from_units(i64::MAX);
        let one = Price::from_units(1);
        assert_eq!((max + one).units(), i64::MAX);
    }

    #[test]
    fn test_price_saturating_sub() {
        let min = Price::from_units(i64::MIN);
        let one = Price::from_units(1);
        assert_eq!((min - one).units(), i64::MIN);
    }

    #[test]
    fn test_price_checked_add_overflow() {
        let max = Price::from_units(i64::MAX);
        let one = Price::from_units(1);
        assert!(max.checked_add(one).is_none());
    }

    #[test]
    fn test_price_checked_sub_overflow() {
        let min = Price::from_units(i64::MIN);
        let one = Price::from_units(1);
        assert!(min.checked_sub(one).is_none());
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
        let range = DateRange::new(start, end).expect("invariant: start <= end");

        let dates: Vec<_> = range.dates().collect();
        assert_eq!(dates.len(), 3);
        assert_eq!(dates[0], start);
        assert_eq!(dates[2], end);
    }

    #[test]
    fn test_time_range_contains() {
        let range =
            TimeRange::new(Timestamp(1000), Timestamp(2000)).expect("invariant: start <= end");
        assert!(range.contains(Timestamp(1500)));
        assert!(!range.contains(Timestamp(500)));
        assert!(!range.contains(Timestamp(2500)));
    }
}
