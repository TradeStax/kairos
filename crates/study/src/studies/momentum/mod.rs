//! Momentum oscillators: RSI, MACD, and Stochastic.
//!
//! Momentum indicators measure the rate of price change rather than price
//! levels. They oscillate between fixed boundaries and help traders identify
//! overbought/oversold conditions, trend strength, and potential reversals.
//!
//! ## Indicators
//!
//! - **RSI** — Relative Strength Index. Uses Wilder's smoothed moving
//!   average to gauge whether recent price action has been predominantly
//!   bullish or bearish. Oscillates 0--100 with conventional overbought
//!   (70) and oversold (30) thresholds.
//!
//! - **MACD** — Moving Average Convergence Divergence. Tracks the spread
//!   between a fast and slow EMA, with a signal-line EMA and divergence
//!   histogram. Crossovers between the MACD line and signal line flag
//!   potential trend shifts.
//!
//! - **Stochastic** — Stochastic Oscillator. Compares the current close
//!   to the recent high-low range, producing %K (position in range) and
//!   %D (smoothed signal). The slow variant applies SMA smoothing to %K
//!   before deriving %D.
//!
//! All three studies render in a separate panel below the price chart.

pub mod macd;
pub mod rsi;
pub mod stochastic;

pub use macd::MacdStudy;
pub use rsi::RsiStudy;
pub use stochastic::StochasticStudy;
