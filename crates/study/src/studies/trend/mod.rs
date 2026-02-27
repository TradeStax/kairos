//! Trend-following studies.
//!
//! Trend indicators smooth price data to reveal the prevailing direction
//! of the market. They are typically rendered as overlay lines on the
//! price chart. Traders use them to identify trend direction, filter
//! entries in the direction of the trend, and define dynamic support and
//! resistance levels.
//!
//! # Available studies
//!
//! - [`SmaStudy`] -- **Simple Moving Average**. An equal-weight average of
//!   the last *N* candle values. The most basic trend filter; widely used
//!   to gauge long-term direction (e.g. 200-period) or short-term momentum
//!   (e.g. 20-period). All values in the window contribute equally, so the
//!   SMA reacts slowly to sudden price changes.
//!
//! - [`EmaStudy`] -- **Exponential Moving Average**. A recency-weighted
//!   average that applies a decaying multiplier `k = 2 / (period + 1)` to
//!   each new value. Recent prices carry more weight, so the EMA responds
//!   faster to changes than the SMA. Favored for crossover systems and
//!   short-term trend detection (common periods: 9, 12, 21, 26).
//!
//! - [`VwapStudy`] -- **Volume Weighted Average Price**. The cumulative
//!   ratio of price-times-volume to total volume across the session.
//!   Institutional traders treat VWAP as a benchmark for execution quality.
//!   Price above VWAP suggests bullish conviction; below suggests bearish.
//!   Optional standard-deviation bands highlight extremes relative to the
//!   session mean.

pub mod ema;
pub mod sma;
pub mod vwap;

pub use ema::EmaStudy;
pub use sma::SmaStudy;
pub use vwap::VwapStudy;
