//! Shared helpers for candle data extraction and statistics.
//!
//! - [`candle`] — Extract OHLC values and compute x-coordinates from candles.
//! - [`math`] — Mean, variance, and standard deviation.

pub mod candle;
pub mod math;
#[cfg(test)]
pub(crate) mod test_helpers;

pub use candle::{candle_key, source_value};
