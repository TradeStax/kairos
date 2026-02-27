//! Built-in strategy implementations.
//!
//! These strategies ship with the backtest engine and are registered
//! automatically via
//! [`StrategyRegistry::with_built_ins`](super::registry::StrategyRegistry::with_built_ins).
//!
//! - [`MomentumBreakoutStrategy`] — Donchian channel breakout with
//!   ATR-scaled stops
//! - [`OrbStrategy`] — Opening Range Breakout
//! - [`VwapReversionStrategy`] — VWAP standard-deviation band fades

pub mod momentum_breakout;
pub mod orb;
pub mod vwap_reversion;

pub use momentum_breakout::MomentumBreakoutStrategy;
pub use orb::OrbStrategy;
pub use vwap_reversion::VwapReversionStrategy;
