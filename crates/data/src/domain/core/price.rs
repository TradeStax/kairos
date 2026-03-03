//! Price arithmetic utilities — formatting, stepping, and precision helpers.
//!
//! Builds on [`Price`] from the `types` module with display formatting via
//! [`PriceExt`], tick-step conversion via [`PriceStep`], and configurable
//! precision via the const-generic [`Power10`] type.

use serde::{Deserialize, Serialize};

use crate::domain::core::types::Price;

// ── Type Aliases ────────────────────────────────────────────────────────

/// Contract size expressed as a power of 10 in the range 10^-4 .. 10^6.
pub type ContractSize = Power10<-4, 6>;

/// Minimum tick size expressed as a power of 10 in the range 10^-8 .. 10^2.
pub type MinTicksize = Power10<-8, 2>;

/// Minimum quantity size expressed as a power of 10 in the range 10^-6 .. 10^8.
pub type MinQtySize = Power10<-6, 8>;

// ── Power10 ─────────────────────────────────────────────────────────────

/// A power-of-10 value clamped to the compile-time range `[MIN, MAX]`.
///
/// Used to represent precision and sizing parameters that are always
/// integral powers of 10 (e.g. tick sizes, contract sizes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Power10<const MIN: i8, const MAX: i8> {
    /// The exponent, clamped to `[MIN, MAX]`.
    pub power: i8,
}

impl<const MIN: i8, const MAX: i8> Power10<MIN, MAX> {
    /// Create with `power` clamped to `[MIN, MAX]`
    #[inline]
    #[must_use]
    pub fn new(power: i8) -> Self {
        Self {
            power: power.clamp(MIN, MAX),
        }
    }

    /// Convert to `f32` (10^power)
    #[inline]
    #[must_use]
    pub fn as_f32(self) -> f32 {
        10f32.powi(self.power as i32)
    }
}

impl<const MIN: i8, const MAX: i8> From<Power10<MIN, MAX>> for f32 {
    fn from(v: Power10<MIN, MAX>) -> Self {
        v.as_f32()
    }
}

impl<const MIN: i8, const MAX: i8> From<f32> for Power10<MIN, MAX> {
    fn from(value: f32) -> Self {
        if value <= 0.0 {
            return Self { power: 0 };
        }
        let log10 = value.abs().log10();
        let rounded = log10.round() as i8;
        let power = rounded.clamp(MIN, MAX);
        Self { power }
    }
}

impl<const MIN: i8, const MAX: i8> Serialize for Power10<MIN, MAX> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let v: f32 = (*self).into();
        serializer.serialize_f32(v)
    }
}

impl<'de, const MIN: i8, const MAX: i8> Deserialize<'de> for Power10<MIN, MAX> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = f32::deserialize(deserializer)?;
        Ok(Self::from(v))
    }
}

// ── PriceStep ───────────────────────────────────────────────────────────

/// A tick step expressed in atomic price units (10^-PRICE_SCALE).
///
/// Used for converting between display tick sizes (e.g. 0.25) and the
/// internal fixed-point representation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PriceStep {
    /// Step size in atomic units (10^-PRICE_SCALE).
    pub units: i64,
}

impl PriceStep {
    /// Convert to `f32` (lossy) for UI display
    #[must_use]
    pub fn to_f32_lossy(self) -> f32 {
        let scale = 10f32.powi(Price::PRICE_SCALE);
        (self.units as f32) / scale
    }

    /// Create from an `f32` step, rounding to the nearest atomic unit.
    ///
    /// Returns `None` if `step` is not positive or too small for the
    /// current `PRICE_SCALE`.
    #[must_use]
    pub fn from_f32_lossy(step: f32) -> Option<Self> {
        if step <= 0.0 {
            return None;
        }
        let scale = 10f32.powi(Price::PRICE_SCALE);
        let units = (step * scale).round() as i64;
        if units <= 0 {
            return None;
        }
        Some(Self { units })
    }

    /// Create from an `f32` step. **Panics** on non-positive or too-small input.
    ///
    /// # Panics
    /// Panics if `step <= 0.0` or if `step` is too small to be representable
    /// at the current `PRICE_SCALE`.
    ///
    /// Prefer [`from_f32_lossy`](Self::from_f32_lossy) at API boundaries where
    /// the value may come from user input. This convenience method is intended
    /// for use with known-good constants (e.g. tick sizes from
    /// `FuturesTickerInfo`).
    #[must_use]
    pub fn from_f32(step: f32) -> Self {
        Self::from_f32_lossy(step)
            .expect("PriceStep::from_f32: step must be positive and representable")
    }

    /// Convert to a [`Price`] with the same atomic units
    #[must_use]
    pub fn to_price(self) -> Price {
        Price::from_units(self.units)
    }
}

impl From<PriceStep> for Price {
    fn from(step: PriceStep) -> Self {
        Price::from_units(step.units)
    }
}

// ── PriceExt ────────────────────────────────────────────────────────────

/// Extension trait adding precision-aware formatting methods to [`Price`].
pub trait PriceExt {
    /// Format price as a string with the given `Power10` precision
    fn fmt_with_precision<const MIN: i8, const MAX: i8>(
        self,
        precision: Power10<MIN, MAX>,
    ) -> String;

    /// Write the formatted price into the given writer
    fn fmt_into<const MIN: i8, const MAX: i8, W: core::fmt::Write>(
        self,
        precision: Power10<MIN, MAX>,
        out: &mut W,
    ) -> core::fmt::Result;

    /// Round to the nearest multiple of the provided min tick size
    fn round_to_min_tick(self, min_tick: MinTicksize) -> Self;
}

impl PriceExt for Price {
    #[inline]
    fn fmt_with_precision<const MIN: i8, const MAX: i8>(
        self,
        precision: Power10<MIN, MAX>,
    ) -> String {
        let mut out = String::with_capacity(24);
        self.fmt_into(precision, &mut out).unwrap();
        out
    }

    #[inline]
    fn fmt_into<const MIN: i8, const MAX: i8, W: core::fmt::Write>(
        self,
        precision: Power10<MIN, MAX>,
        out: &mut W,
    ) -> core::fmt::Result {
        let scale_u = Price::PRICE_SCALE as u32;

        let exp = (Price::PRICE_SCALE + precision.power as i32) as u32;
        debug_assert!(Price::PRICE_SCALE + precision.power as i32 >= 0);
        let unit = 10i64
            .checked_pow(exp)
            .expect("Price::fmt_into unit overflow");

        let u = self.units();
        let half = unit / 2;
        let rounded_units = if u >= 0 {
            ((u + half).div_euclid(unit)) * unit
        } else {
            ((u - half).div_euclid(unit)) * unit
        };

        let decimals: u32 = if precision.power < 0 {
            ((-precision.power) as u32).min(scale_u)
        } else {
            0
        };

        if rounded_units < 0 {
            core::fmt::Write::write_char(out, '-')?;
        }
        let abs_u = (rounded_units as i128).unsigned_abs();

        let scale_pow = 10u128.pow(scale_u);
        let int_part = abs_u / scale_pow;
        write!(out, "{}", int_part)?;

        if decimals == 0 {
            return Ok(());
        }

        let frac_div = 10u128.pow(scale_u - decimals);
        let frac_part = (abs_u % scale_pow) / frac_div;
        write!(out, ".{:0width$}", frac_part, width = decimals as usize)
    }

    fn round_to_min_tick(self, min_tick: MinTicksize) -> Self {
        let exp = Price::PRICE_SCALE + (min_tick.power as i32);
        assert!(exp >= 0, "PRICE_SCALE must be >= -min_tick.power");
        let unit = 10i64
            .checked_pow(exp as u32)
            .expect("min_tick_units overflowed");
        if unit <= 1 {
            return self;
        }
        let half = unit / 2;
        let rounded = ((self.units() + half).div_euclid(unit)) * unit;
        Price::from_units(rounded)
    }
}

/// Convert a millisecond Unix timestamp to a `chrono::DateTime<Utc>`
#[must_use]
pub fn ms_to_datetime(ms: u64) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::from_timestamp((ms / 1000) as i64, ((ms % 1000) * 1_000_000) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_saturates_on_overflow() {
        let max = Price::from_units(i64::MAX);
        let one = Price::from_units(1);
        assert_eq!((max + one).units(), i64::MAX);
    }

    #[test]
    fn sub_saturates_on_overflow() {
        let min = Price::from_units(i64::MIN);
        let one = Price::from_units(1);
        assert_eq!((min - one).units(), i64::MIN);
    }

    #[test]
    fn checked_add_returns_none_on_overflow() {
        let max = Price::from_units(i64::MAX);
        let one = Price::from_units(1);
        assert!(max.checked_add(one).is_none());
    }

    #[test]
    fn checked_sub_returns_none_on_overflow() {
        let min = Price::from_units(i64::MIN);
        let one = Price::from_units(1);
        assert!(min.checked_sub(one).is_none());
    }

    #[test]
    fn add_steps_saturates_on_overflow() {
        let max = Price::from_units(i64::MAX);
        let step = PriceStep { units: 100 };
        let result = max.add_steps(1, step.into());
        assert_eq!(result.units(), i64::MAX);
    }
}
