//! Top-level backtest configuration.
//!
//! [`BacktestConfig`] is the single entry point that fully describes
//! a backtest run: which instrument, date range, timeframe, capital,
//! risk parameters, fees, session times, and strategy selection.

use crate::config::margin::MarginConfig;
use crate::config::risk::{RiskConfig, SlippageModel};
use kairos_data::{DateRange, FuturesTicker, FuturesVenue, Timeframe};
use kairos_study::ParameterValue;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Full configuration for a single backtest run.
///
/// Create one via [`BacktestConfig::default_es`] for quick prototyping,
/// or construct manually for full control. Always call [`validate`]
/// before passing to the engine.
///
/// [`validate`]: BacktestConfig::validate
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BacktestConfig {
    /// The futures ticker to backtest (e.g. `"ES.c.0"`).
    pub ticker: FuturesTicker,
    /// Date range for historical data (inclusive on both ends).
    pub date_range: DateRange,
    /// Candle timeframe used for `on_candle_close` callbacks.
    pub timeframe: Timeframe,
    /// Starting equity in USD. Must be positive.
    pub initial_capital_usd: f64,
    /// Risk and position-sizing configuration.
    pub risk: RiskConfig,
    /// Slippage model applied on every simulated fill.
    pub slippage: SlippageModel,
    /// Commission per side per contract in USD (e.g. 2.50 for ES).
    ///
    /// Total round-trip cost = `commission_per_side_usd * 2 * quantity`.
    pub commission_per_side_usd: f64,
    /// UTC offset in hours for session time interpretation.
    ///
    /// Eastern Standard = -5, Eastern Daylight = -4.
    pub timezone_offset_hours: i32,
    /// RTH (Regular Trading Hours) open time as an HHMM integer.
    ///
    /// Example: `930` represents 09:30 local time.
    pub rth_open_hhmm: u32,
    /// RTH close time as an HHMM integer.
    ///
    /// Example: `1600` represents 16:00 local time.
    pub rth_close_hhmm: u32,
    /// Strategy identifier that must exist in the `StrategyRegistry`.
    pub strategy_id: String,
    /// Strategy parameter overrides, validated against the strategy's
    /// `ParameterDef` list at engine startup.
    pub strategy_params: HashMap<String, ParameterValue>,
    /// Additional instruments for multi-instrument backtests.
    #[serde(default)]
    pub additional_instruments: Vec<FuturesTicker>,
    /// Additional timeframes for multi-timeframe strategies.
    #[serde(default)]
    pub additional_timeframes: Vec<Timeframe>,
    /// Number of candle periods to process before the strategy goes
    /// live (warm-up / look-back window).
    #[serde(default)]
    pub warm_up_periods: usize,
    /// Whether to load and use depth-of-book data for fill
    /// simulation.
    #[serde(default)]
    pub use_depth_data: bool,
    /// Margin enforcement configuration.
    #[serde(default)]
    pub margin: MarginConfig,
    /// Simulated order-to-fill latency in milliseconds.
    ///
    /// `0` means instant fills (no latency simulation).
    #[serde(default)]
    pub simulated_latency_ms: u64,
}

impl BacktestConfig {
    /// Creates a reasonable default configuration for the ES
    /// front-month contract.
    ///
    /// Defaults: 30-minute candles, $100k capital, no slippage,
    /// $2.50/side commission, ET standard timezone, RTH 09:30-16:00.
    #[must_use]
    pub fn default_es(strategy_id: impl Into<String>) -> Self {
        Self {
            ticker: FuturesTicker::new("ES.c.0", FuturesVenue::CMEGlobex),
            date_range: DateRange::default(),
            timeframe: Timeframe::M30,
            initial_capital_usd: 100_000.0,
            risk: RiskConfig::default(),
            slippage: SlippageModel::None,
            commission_per_side_usd: 2.50,
            timezone_offset_hours: -5,
            rth_open_hhmm: 930,
            rth_close_hhmm: 1600,
            strategy_id: strategy_id.into(),
            strategy_params: HashMap::new(),
            additional_instruments: Vec::new(),
            additional_timeframes: Vec::new(),
            warm_up_periods: 0,
            use_depth_data: false,
            margin: MarginConfig::default(),
            simulated_latency_ms: 0,
        }
    }

    /// Validates this configuration, returning a human-readable
    /// error message if any field is invalid.
    ///
    /// Checks performed:
    /// - `initial_capital_usd > 0`
    /// - `commission_per_side_usd >= 0`
    /// - `rth_open_hhmm < rth_close_hhmm` and both are valid HHMM
    /// - Slippage parameters are within bounds
    /// - `strategy_id` is non-empty
    pub fn validate(&self) -> Result<(), String> {
        if self.initial_capital_usd <= 0.0 {
            return Err("initial_capital_usd must be > 0".to_string());
        }
        if self.commission_per_side_usd < 0.0 {
            return Err("commission_per_side_usd must be >= 0".to_string());
        }
        if self.rth_open_hhmm >= self.rth_close_hhmm {
            return Err("rth_open_hhmm must be < rth_close_hhmm".to_string());
        }
        if !validate_hhmm(self.rth_open_hhmm) {
            return Err("rth_open_hhmm is not a valid HHMM time".to_string());
        }
        if !validate_hhmm(self.rth_close_hhmm) {
            return Err("rth_close_hhmm is not a valid HHMM time".to_string());
        }
        if let SlippageModel::Percentage(pct) = &self.slippage
            && (*pct < 0.0 || *pct > 0.10)
        {
            return Err("slippage percentage must be between 0.0 and 0.10 \
                 (10%)"
                .to_string());
        }
        if let SlippageModel::FixedTick(n) = &self.slippage
            && *n < 0
        {
            return Err("slippage fixed ticks must be >= 0".to_string());
        }
        if self.strategy_id.is_empty() {
            return Err("strategy_id must not be empty".to_string());
        }
        Ok(())
    }
}

/// Validates that an HHMM integer represents a valid 24-hour time.
///
/// Hours must be 0..=23 and minutes must be 0..=59.
fn validate_hhmm(hhmm: u32) -> bool {
    let hours = hhmm / 100;
    let minutes = hhmm % 100;
    hours <= 23 && minutes <= 59
}
