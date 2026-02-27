//! VWAP Reversion strategy.
//!
//! Computes session VWAP with an online weighted variance estimator.
//! Enters when price deviates beyond `deviation_bands` standard
//! deviations from VWAP. Exits when price crosses back to VWAP (if
//! `exit_at_vwap` is enabled) or at the fixed stop distance.
//!
//! An optional slope filter skips entries when the recent price
//! trajectory is too steep, avoiding entries during strong trends.

use crate::order::request::{BracketOrder, NewOrder, OrderRequest};
use crate::order::types::OrderSide;
use crate::order::types::OrderType;
use crate::output::trade_record::ExitReason;
use crate::strategy::Strategy;
use crate::strategy::context::{SessionState, StrategyContext};
use crate::strategy::metadata::{StrategyCategory, StrategyMetadata};
use kairos_data::{Candle, FuturesTicker, Price, Timeframe};
use kairos_study::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue, StudyConfig,
    Visibility,
};

/// Online volume-weighted mean and variance tracker.
///
/// Maintains running VWAP and variance using an online weighted
/// algorithm. Used to compute standard deviation bands for entry
/// signals.
#[derive(Debug, Clone, Default)]
struct VwapState {
    /// Current volume-weighted average price.
    vwap: f64,
    /// Accumulated weighted sum of squared deviations.
    m2: f64,
    /// Total accumulated volume.
    total_volume: f64,
    /// Current variance (m2 / total_volume).
    variance: f64,
    /// Running sum of price * volume for VWAP numerator.
    pv_sum: f64,
}

impl VwapState {
    /// Resets all accumulators to zero.
    fn reset(&mut self) {
        *self = Self::default();
    }

    /// Updates the VWAP and variance with a new price/volume
    /// observation.
    fn update(&mut self, price: f64, volume: f64) {
        if volume <= 0.0 {
            return;
        }
        self.pv_sum += price * volume;
        self.total_volume += volume;
        self.vwap = self.pv_sum / self.total_volume;

        let delta = price - self.vwap;
        self.m2 += volume * delta * delta;
        self.variance = if self.total_volume > 0.0 {
            self.m2 / self.total_volume
        } else {
            0.0
        };
    }

    /// Returns the current standard deviation.
    fn std_dev(&self) -> f64 {
        self.variance.sqrt()
    }

    /// Returns the upper band at the given number of standard
    /// deviations.
    fn upper_band(&self, bands: f64) -> f64 {
        self.vwap + bands * self.std_dev()
    }

    /// Returns the lower band at the given number of standard
    /// deviations.
    fn lower_band(&self, bands: f64) -> f64 {
        self.vwap - bands * self.std_dev()
    }
}

/// Computes the percentage price change over the last `periods`
/// candle closes.
///
/// Returns 0.0 if there are fewer candles than `periods` or
/// `periods < 2`.
fn compute_slope_pct(candles: &[Candle], periods: usize) -> f64 {
    if periods < 2 || candles.len() < periods {
        return 0.0;
    }
    let slice = &candles[(candles.len() - periods)..];
    let first = slice[0].close.to_f64();
    let end = slice[periods - 1].close.to_f64();
    if first == 0.0 {
        return 0.0;
    }
    (end - first) / first * 100.0
}

/// VWAP Reversion strategy.
///
/// Fades price deviations from session VWAP at configurable
/// standard-deviation bands. See the [module docs](self) for full
/// details.
pub struct VwapReversionStrategy {
    config: StudyConfig,
    params: Vec<ParameterDef>,
    /// Online VWAP and variance tracker for the current session.
    vwap_state: VwapState,
    /// Number of trades taken in the current session.
    trades_taken: usize,
    /// Most recent candle close price (for state tracking).
    last_candle_close_price: Option<f64>,
}

/// Builds the parameter definitions for this strategy.
fn make_params() -> Vec<ParameterDef> {
    vec![
        ParameterDef {
            key: "deviation_bands".into(),
            label: "Deviation Bands".into(),
            description: "Number of standard deviations from VWAP \
                 for entry."
                .into(),
            kind: ParameterKind::Float {
                min: 0.5,
                max: 4.0,
                step: 0.5,
            },
            default: ParameterValue::Float(2.0),
            tab: ParameterTab::Parameters,
            section: None,
            order: 0,
            format: DisplayFormat::Float { decimals: 1 },
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "exit_at_vwap".into(),
            label: "Exit at VWAP".into(),
            description: "Exit when price crosses back to VWAP.".into(),
            kind: ParameterKind::Boolean,
            default: ParameterValue::Boolean(true),
            tab: ParameterTab::Parameters,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "fixed_stop_ticks".into(),
            label: "Fixed Stop (ticks)".into(),
            description: "Stop loss distance in ticks from entry.".into(),
            kind: ParameterKind::Integer { min: 5, max: 200 },
            default: ParameterValue::Integer(20),
            tab: ParameterTab::Parameters,
            section: None,
            order: 2,
            format: DisplayFormat::Integer { suffix: " ticks" },
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "slope_filter_periods".into(),
            label: "Slope Filter Periods".into(),
            description: "Periods for slope filter (0 = disabled).".into(),
            kind: ParameterKind::Integer { min: 0, max: 50 },
            default: ParameterValue::Integer(10),
            tab: ParameterTab::Parameters,
            section: None,
            order: 3,
            format: DisplayFormat::Integer { suffix: " bars" },
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "slope_threshold_pct".into(),
            label: "Slope Threshold %".into(),
            description: "Skip entries if |slope| exceeds this \
                 percentage."
                .into(),
            kind: ParameterKind::Float {
                min: 0.0,
                max: 1.0,
                step: 0.05,
            },
            default: ParameterValue::Float(0.1),
            tab: ParameterTab::Parameters,
            section: None,
            order: 4,
            format: DisplayFormat::Percent,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "time_exit_hhmm".into(),
            label: "Time Exit".into(),
            description: "Close any open position at this local time \
                 (HHMM)."
                .into(),
            kind: ParameterKind::Integer {
                min: 1200,
                max: 1545,
            },
            default: ParameterValue::Integer(1530),
            tab: ParameterTab::Parameters,
            section: None,
            order: 5,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "max_trades".into(),
            label: "Max Trades".into(),
            description: "Maximum trades per session.".into(),
            kind: ParameterKind::Integer { min: 1, max: 10 },
            default: ParameterValue::Integer(3),
            tab: ParameterTab::Parameters,
            section: None,
            order: 6,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
    ]
}

impl VwapReversionStrategy {
    /// Creates a new instance with default parameter values.
    #[must_use]
    pub fn new() -> Self {
        let params = make_params();
        let mut config = StudyConfig::new("vwap_reversion");
        for p in &params {
            config.set(p.key.clone(), p.default.clone());
        }
        Self {
            config,
            params,
            vwap_state: VwapState::default(),
            trades_taken: 0,
            last_candle_close_price: None,
        }
    }

    fn deviation_bands(&self) -> f64 {
        self.config.get_float("deviation_bands", 2.0)
    }

    fn exit_at_vwap(&self) -> bool {
        self.config.get_bool("exit_at_vwap", true)
    }

    fn fixed_stop_ticks(&self) -> i64 {
        self.config.get_int("fixed_stop_ticks", 20)
    }

    fn slope_filter_periods(&self) -> usize {
        self.config.get_int("slope_filter_periods", 10) as usize
    }

    fn slope_threshold_pct(&self) -> f64 {
        self.config.get_float("slope_threshold_pct", 0.1)
    }

    fn time_exit_hhmm(&self) -> u32 {
        self.config.get_int("time_exit_hhmm", 1530) as u32
    }

    fn max_trades(&self) -> usize {
        self.config.get_int("max_trades", 3) as usize
    }

    /// Checks if it is past the configured time exit and flattens
    /// any open position. Returns `Some(orders)` if time exit was
    /// triggered, `None` otherwise.
    fn check_time_exit(&mut self, ctx: &StrategyContext) -> Option<Vec<OrderRequest>> {
        if ctx.local_hhmm < self.time_exit_hhmm() {
            return None;
        }
        if ctx.primary_position().is_some() {
            self.trades_taken = self.max_trades();
            Some(vec![OrderRequest::Flatten {
                instrument: ctx.primary_instrument,
                reason: ExitReason::SessionClose,
            }])
        } else {
            Some(vec![])
        }
    }

    /// Checks if the current position should be exited because
    /// price has crossed back to VWAP.
    fn check_vwap_exit(&self, price: f64, ctx: &StrategyContext) -> Option<Vec<OrderRequest>> {
        if !self.exit_at_vwap() {
            return None;
        }
        let pos = ctx.primary_position()?;
        let vwap = self.vwap_state.vwap;
        let crossed = match pos.side {
            OrderSide::Buy => price >= vwap,
            OrderSide::Sell => price <= vwap,
        };
        if crossed {
            Some(vec![OrderRequest::Flatten {
                instrument: ctx.primary_instrument,
                reason: ExitReason::Manual,
            }])
        } else {
            None
        }
    }
}

impl Default for VwapReversionStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl Strategy for VwapReversionStrategy {
    fn id(&self) -> &str {
        "vwap_reversion"
    }

    fn metadata(&self) -> StrategyMetadata {
        StrategyMetadata {
            id: "vwap_reversion".to_string(),
            name: "VWAP Reversion".to_string(),
            description: "Fades price deviations from VWAP at \
                          standard-deviation bands."
                .to_string(),
            category: StrategyCategory::MeanReversion,
            version: "1.0.0",
        }
    }

    fn parameters(&self) -> &[ParameterDef] {
        &self.params
    }

    fn config(&self) -> &StudyConfig {
        &self.config
    }

    fn config_mut(&mut self) -> &mut StudyConfig {
        &mut self.config
    }

    fn on_session_open(&mut self, _ctx: &StrategyContext) -> Vec<OrderRequest> {
        self.vwap_state.reset();
        self.trades_taken = 0;
        self.last_candle_close_price = None;
        vec![]
    }

    fn on_candle(
        &mut self,
        instrument: FuturesTicker,
        timeframe: Timeframe,
        candle: &Candle,
        ctx: &StrategyContext,
    ) -> Vec<OrderRequest> {
        if instrument != ctx.primary_instrument {
            return vec![];
        }

        // Update VWAP state with closed candle
        let volume = candle.total_volume().0;
        let close = candle.close.to_f64();
        self.vwap_state.update(close, volume);
        self.last_candle_close_price = Some(close);

        let tick_size = ctx.tick_size();

        // Time exit
        if let Some(orders) = self.check_time_exit(ctx) {
            return orders;
        }

        // Exit at VWAP when holding a position
        if let Some(orders) = self.check_vwap_exit(close, ctx) {
            return orders;
        }

        // Band entry on candle close
        if ctx.primary_position().is_some() || self.trades_taken >= self.max_trades() {
            return vec![];
        }

        let bands = self.deviation_bands();
        let sd = self.vwap_state.std_dev();
        if sd < 1e-12 {
            return vec![];
        }

        // Slope filter
        let slope_periods = self.slope_filter_periods();
        if slope_periods > 0 {
            let candles = ctx.primary_candles(timeframe);
            let slope = compute_slope_pct(candles, slope_periods);
            if slope.abs() > self.slope_threshold_pct() {
                return vec![];
            }
        }

        let upper = self.vwap_state.upper_band(bands);
        let lower = self.vwap_state.lower_band(bands);
        let stop_offset = tick_size.add_steps(self.fixed_stop_ticks(), tick_size);
        let tp_price = if self.exit_at_vwap() {
            Some(Price::from_f64(self.vwap_state.vwap))
        } else {
            None
        };

        if close <= lower {
            // Price at or below lower band -> fade long
            let sl = candle.close - stop_offset;
            self.trades_taken += 1;
            return vec![OrderRequest::SubmitBracket(BracketOrder {
                entry: NewOrder {
                    instrument: ctx.primary_instrument,
                    side: OrderSide::Buy,
                    order_type: OrderType::Market,
                    quantity: 1.0,
                    time_in_force: Default::default(),
                    label: Some("VWAP Fade Long".to_string()),
                    reduce_only: false,
                },
                stop_loss: sl,
                take_profit: tp_price,
            })];
        }

        if close >= upper {
            // Price at or above upper band -> fade short
            let sl = candle.close + stop_offset;
            self.trades_taken += 1;
            return vec![OrderRequest::SubmitBracket(BracketOrder {
                entry: NewOrder {
                    instrument: ctx.primary_instrument,
                    side: OrderSide::Sell,
                    order_type: OrderType::Market,
                    quantity: 1.0,
                    time_in_force: Default::default(),
                    label: Some("VWAP Fade Short".to_string()),
                    reduce_only: false,
                },
                stop_loss: sl,
                take_profit: tp_price,
            })];
        }

        vec![]
    }

    fn on_tick(&mut self, ctx: &StrategyContext) -> Vec<OrderRequest> {
        if ctx.session_state != SessionState::Open {
            return vec![];
        }

        // Time exit
        if let Some(orders) = self.check_time_exit(ctx) {
            return orders;
        }

        // Tick-level VWAP cross exit
        let price = ctx.trade.price.to_f64();
        if let Some(orders) = self.check_vwap_exit(price, ctx) {
            return orders;
        }

        vec![]
    }

    fn on_session_close(&mut self, ctx: &StrategyContext) -> Vec<OrderRequest> {
        if ctx.primary_position().is_some() {
            return vec![OrderRequest::Flatten {
                instrument: ctx.primary_instrument,
                reason: ExitReason::SessionClose,
            }];
        }
        vec![]
    }

    fn reset(&mut self) {
        self.vwap_state.reset();
        self.trades_taken = 0;
        self.last_candle_close_price = None;
    }

    fn clone_strategy(&self) -> Box<dyn Strategy> {
        Box::new(VwapReversionStrategy {
            config: self.config.clone(),
            params: self.params.clone(),
            vwap_state: self.vwap_state.clone(),
            trades_taken: self.trades_taken,
            last_candle_close_price: self.last_candle_close_price,
        })
    }
}
