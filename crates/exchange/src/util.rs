use serde::{Deserialize, Serialize};

// Re-export the canonical Price type from the data crate.
pub use kairos_data::Price;

/// Validate API key format shared across adapters.
///
/// Returns `true` when the key is non-empty and at least 10 characters long.
pub(crate) fn validate_api_key(key: &str) -> bool {
    !key.is_empty() && key.len() >= 10
}

pub type ContractSize = Power10<-4, 6>;
pub type MinTicksize = Power10<-8, 2>;
pub type MinQtySize = Power10<-6, 8>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Power10<const MIN: i8, const MAX: i8> {
    pub power: i8,
}

impl<const MIN: i8, const MAX: i8> Power10<MIN, MAX> {
    #[inline]
    pub fn new(power: i8) -> Self {
        Self {
            power: power.clamp(MIN, MAX),
        }
    }

    #[inline]
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

impl<const MIN: i8, const MAX: i8> serde::Serialize for Power10<MIN, MAX> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // serialize as a plain numeric (e.g. 0.1, 1, 10)
        let v: f32 = (*self).into();
        serializer.serialize_f32(v)
    }
}

impl<'de, const MIN: i8, const MAX: i8> serde::Deserialize<'de> for Power10<MIN, MAX> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = f32::deserialize(deserializer)?;
        Ok(Self::from(v))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PriceStep {
    /// step size in atomic units (10^-PRICE_SCALE)
    pub units: i64,
}

impl PriceStep {
    /// Lossy: f32 step for UI
    pub fn to_f32_lossy(self) -> f32 {
        let scale = 10f32.powi(Price::PRICE_SCALE);
        (self.units as f32) / scale
    }

    /// Lossy: from f32 step (rounds to nearest atomic unit)
    pub fn from_f32_lossy(step: f32) -> Self {
        assert!(step > 0.0, "step must be > 0");
        let scale = 10f32.powi(Price::PRICE_SCALE);
        let units = (step * scale).round() as i64;
        assert!(units > 0, "step too small at given PRICE_SCALE");
        Self { units }
    }

    pub fn from_f32(step: f32) -> Self {
        Self::from_f32_lossy(step)
    }

    /// Convert to a Price with the same units value
    pub fn to_price(self) -> Price {
        Price::from_units(self.units)
    }
}

impl From<PriceStep> for Price {
    fn from(step: PriceStep) -> Self {
        Price::from_units(step.units)
    }
}

/// Extension trait adding exchange-specific formatting methods to `Price`.
///
/// Provides formatting with `Power10` precision and rounding to `MinTicksize`.
/// For PriceStep-based operations (round_to_step, add_steps, etc.), use
/// `step.into()` to convert PriceStep to Price and call the inherent methods.
pub trait PriceExt {
    /// Format price as string with the given `Power10` precision
    fn fmt_with_precision<const MIN: i8, const MAX: i8>(
        self,
        precision: Power10<MIN, MAX>,
    ) -> String;

    /// Write formatted price into the given writer
    fn fmt_into<const MIN: i8, const MAX: i8, W: core::fmt::Write>(
        self,
        precision: Power10<MIN, MAX>,
        out: &mut W,
    ) -> core::fmt::Result;

    /// Round to the nearest multiple of the provided min ticksize
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

        // number of atomic units for the given decade step: 10^(PRICE_SCALE + power)
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

/// Convert a millisecond Unix timestamp to a chrono DateTime<Utc>
pub fn ms_to_datetime(ms: u64) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::from_timestamp((ms / 1000) as i64, ((ms % 1000) * 1_000_000) as u32)
}

#[cfg(test)]
mod price_overflow_tests {
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

#[cfg(test)]
mod manual_printouts {
    use super::*;

    #[test]
    fn show_min_tick_rounding() {
        let orig: f32 = 0.000051;
        let p = Price::from_f32(orig);
        let back = p.to_f32();

        let scale = 10f32.powi(Price::PRICE_SCALE);
        let expected_units = (orig * scale).round() as i64;
        let expected_back = (expected_units as f32) / scale;

        println!("orig (f32)        = {:0.9}", orig);
        println!("orig bits         = 0x{:08x}", orig.to_bits());
        println!("price units       = {}", p.units());
        println!("expected units    = {}", expected_units);
        println!("back (from units) = {:0.9}", back);
        println!("expected back     = {:0.9}", expected_back);
        println!("orig - back       = {:+.9e}", orig - back);
        println!("back == expected  = {}", back == expected_back);
    }
}
