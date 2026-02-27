//! Volatility studies.
//!
//! Volatility indicators measure the rate and magnitude of price
//! fluctuations. They help traders assess risk, set stop-loss levels,
//! and identify periods of expansion or contraction. Unlike trend
//! indicators, volatility measures are direction-agnostic -- they
//! quantify *how much* price is moving, not *which way*.
//!
//! # Available studies
//!
//! - [`AtrStudy`] -- **Average True Range**. Measures market volatility
//!   as the Wilder-smoothed average of the True Range (the greatest of
//!   High-Low, |High-PrevClose|, |Low-PrevClose|). ATR is the standard
//!   tool for position sizing and stop-loss placement. Rendered as a
//!   single line in a separate panel below the price chart, since its
//!   scale differs from price.
//!
//! - [`BollingerStudy`] -- **Bollinger Bands**. A volatility envelope
//!   consisting of an SMA middle line flanked by upper and lower bands
//!   at a configurable number of standard deviations. The bands widen
//!   during volatile periods and contract during calm ones. Traders
//!   watch for "squeezes" (narrow bands) as breakout precursors and
//!   use band touches to gauge overbought/oversold conditions. Rendered
//!   as an overlay on the price chart.

pub mod atr;
pub mod bollinger;

pub use atr::AtrStudy;
pub use bollinger::BollingerStudy;
