use crate::config::risk::{RiskConfig, SlippageModel};
use kairos_data::{DateRange, FuturesTicker, FuturesVenue, Timeframe};
use kairos_study::ParameterValue;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Full configuration for a single backtest run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    /// The futures ticker to backtest (e.g. "ES.c.0").
    pub ticker: FuturesTicker,
    /// Date range for historical data (inclusive).
    pub date_range: DateRange,
    /// Candle timeframe used for on_candle_close callbacks.
    pub timeframe: Timeframe,
    /// Starting equity in USD. Must be > 0.
    pub initial_capital_usd: f64,
    /// Risk / position sizing configuration.
    pub risk: RiskConfig,
    /// Slippage model applied on every fill.
    pub slippage: SlippageModel,
    /// Commission per side per contract in USD (e.g. 2.50 for ES).
    /// Total per trade = commission_per_side_usd * 2 * quantity.
    pub commission_per_side_usd: f64,
    /// UTC offset in hours. ET standard = -5, ET daylight = -4.
    pub timezone_offset_hours: i32,
    /// RTH open time as HHMM integer (e.g. 930 = 09:30 local).
    pub rth_open_hhmm: u32,
    /// RTH close time as HHMM integer (e.g. 1600 = 16:00 local).
    pub rth_close_hhmm: u32,
    /// Strategy ID — must exist in StrategyRegistry.
    pub strategy_id: String,
    /// Strategy parameter overrides validated against the strategy's ParameterDef list.
    pub strategy_params: HashMap<String, ParameterValue>,
}

impl BacktestConfig {
    /// Reasonable default configuration for the ES front-month.
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
        }
    }

    /// Validate this configuration, returning a human-readable error message if invalid.
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
        if self.strategy_id.is_empty() {
            return Err("strategy_id must not be empty".to_string());
        }
        Ok(())
    }
}
