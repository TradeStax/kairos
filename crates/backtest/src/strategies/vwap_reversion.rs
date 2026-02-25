//! VWAP Reversion strategy.
//!
//! Computes session VWAP with Welford online variance. Enters on price deviation
//! beyond `deviation_bands` standard deviations. Exits at VWAP cross (or opposite band).

use crate::core::input::{SessionState, StrategyInput};
use crate::core::metadata::{StrategyCategory, StrategyMetadata};
use crate::core::signal::Signal;
use crate::core::strategy::BacktestStrategy;
use crate::domain::trade_record::ExitReason;
use kairos_data::{Candle, Price, Side};
use kairos_study::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue, StudyConfig,
    Visibility,
};

/// Welford online algorithm for volume-weighted mean and variance.
#[derive(Debug, Clone, Default)]
struct VwapState {
    vwap: f64,
    /// M2 accumulator for variance (Welford)
    m2: f64,
    total_volume: f64,
    /// Variance = m2 / total_volume when total_volume > 0
    variance: f64,
    /// Sum of price*volume used for VWAP numerator
    pv_sum: f64,
}

impl VwapState {
    fn reset(&mut self) {
        *self = Self::default();
    }

    /// Update with a new price/volume data point.
    fn update(&mut self, price: f64, volume: f64) {
        if volume <= 0.0 {
            return;
        }
        self.pv_sum += price * volume;
        self.total_volume += volume;
        self.vwap = self.pv_sum / self.total_volume;

        // Welford online weighted variance
        let delta = price - self.vwap;
        // After updating the mean, recompute delta2 using the new mean
        self.m2 += volume * delta * delta;
        self.variance = if self.total_volume > 0.0 { self.m2 / self.total_volume } else { 0.0 };
    }

    fn std_dev(&self) -> f64 {
        self.variance.sqrt()
    }

    fn upper_band(&self, bands: f64) -> f64 {
        self.vwap + bands * self.std_dev()
    }

    fn lower_band(&self, bands: f64) -> f64 {
        self.vwap - bands * self.std_dev()
    }
}

/// Slope of the last `n` candle closes (linear regression slope as pct change).
fn compute_slope_pct(candles: &[Candle], periods: usize) -> f64 {
    if periods < 2 || candles.len() < periods {
        return 0.0;
    }
    let last = candles.len();
    let slice = &candles[(last - periods)..];
    let first = slice[0].close.to_f64();
    let end = slice[periods - 1].close.to_f64();
    if first == 0.0 {
        return 0.0;
    }
    (end - first) / first * 100.0
}

pub struct VwapReversionStrategy {
    config: StudyConfig,
    params: Vec<ParameterDef>,
    vwap_state: VwapState,
    trades_taken: usize,
    last_candle_close_price: Option<f64>,
}

fn make_params() -> Vec<ParameterDef> {
    vec![
        ParameterDef {
            key: "deviation_bands".into(),
            label: "Deviation Bands".into(),
            description: "Number of standard deviations from VWAP for entry.".into(),
            kind: ParameterKind::Float { min: 0.5, max: 4.0, step: 0.5 },
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
            description: "Periods for VWAP slope filter (0 = disabled).".into(),
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
            description: "Skip entries if |slope| exceeds this percentage.".into(),
            kind: ParameterKind::Float { min: 0.0, max: 1.0, step: 0.05 },
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
            description: "Close any open position at this local time (HHMM).".into(),
            kind: ParameterKind::Integer { min: 1200, max: 1545 },
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
}

impl Default for VwapReversionStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl BacktestStrategy for VwapReversionStrategy {
    fn id(&self) -> &str {
        "vwap_reversion"
    }

    fn metadata(&self) -> StrategyMetadata {
        StrategyMetadata {
            id: "vwap_reversion".to_string(),
            name: "VWAP Reversion".to_string(),
            description: "Fades price deviations from VWAP at standard-deviation bands."
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

    fn on_session_open(&mut self, _input: &StrategyInput<'_>) -> Vec<Signal> {
        self.vwap_state.reset();
        self.trades_taken = 0;
        self.last_candle_close_price = None;
        vec![]
    }

    fn on_candle_close(&mut self, candle: &Candle, input: &StrategyInput<'_>) -> Vec<Signal> {
        // Update VWAP state with closed candle
        let volume = candle.total_volume().0;
        let price = candle.close.to_f64();
        self.vwap_state.update(price, volume);
        self.last_candle_close_price = Some(price);

        // Time exit
        if input.local_hhmm >= self.time_exit_hhmm() {
            if input.open_position.is_some() {
                self.trades_taken = self.max_trades(); // prevent re-entry
                return vec![Signal::CloseAll { reason: ExitReason::SessionClose }];
            }
            return vec![];
        }

        // Exit at VWAP when holding a position
        if self.exit_at_vwap() {
            if let Some(pos) = &input.open_position {
                let vwap = self.vwap_state.vwap;
                let close = candle.close.to_f64();
                let crossed_vwap = match pos.side {
                    Side::Buy => close >= vwap,
                    Side::Sell => close <= vwap,
                    _ => false,
                };
                if crossed_vwap {
                    return vec![Signal::Close { reason: ExitReason::Manual }];
                }
            }
        }

        // Band entry on candle close
        if input.open_position.is_none() && self.trades_taken < self.max_trades() {
            let bands = self.deviation_bands();
            let sd = self.vwap_state.std_dev();
            if sd < 1e-12 {
                return vec![];
            }

            // Slope filter
            let slope_periods = self.slope_filter_periods();
            let slope_threshold = self.slope_threshold_pct();
            if slope_periods > 0 {
                let slope = compute_slope_pct(input.candles, slope_periods);
                if slope.abs() > slope_threshold {
                    return vec![];
                }
            }

            let upper = self.vwap_state.upper_band(bands);
            let lower = self.vwap_state.lower_band(bands);
            let close = candle.close.to_f64();

            if close <= lower {
                // Price at or below lower band → fade the move, go long
                let entry = candle.close;
                let sl = entry - input.tick_size.add_steps(self.fixed_stop_ticks(), input.tick_size);
                let tp_price = Price::from_f64(self.vwap_state.vwap);
                self.trades_taken += 1;
                return vec![Signal::Open {
                    side: Side::Buy,
                    quantity: 1.0,
                    quantity_override: None,
                    stop_loss: sl,
                    take_profit: if self.exit_at_vwap() { Some(tp_price) } else { None },
                    label: Some("VWAP Fade Long".to_string()),
                }];
            }

            if close >= upper {
                // Price at or above upper band → fade the move, go short
                let entry = candle.close;
                let sl = entry + input.tick_size.add_steps(self.fixed_stop_ticks(), input.tick_size);
                let tp_price = Price::from_f64(self.vwap_state.vwap);
                self.trades_taken += 1;
                return vec![Signal::Open {
                    side: Side::Sell,
                    quantity: 1.0,
                    quantity_override: None,
                    stop_loss: sl,
                    take_profit: if self.exit_at_vwap() { Some(tp_price) } else { None },
                    label: Some("VWAP Fade Short".to_string()),
                }];
            }
        }

        vec![]
    }

    fn on_tick(&mut self, input: &StrategyInput<'_>) -> Vec<Signal> {
        if input.session_state != SessionState::Open {
            return vec![];
        }

        // Time exit
        if input.local_hhmm >= self.time_exit_hhmm() {
            if input.open_position.is_some() {
                self.trades_taken = self.max_trades();
                return vec![Signal::CloseAll { reason: ExitReason::SessionClose }];
            }
            return vec![];
        }

        // Tick-level VWAP cross exit (if exit_at_vwap enabled)
        if self.exit_at_vwap() {
            if let Some(pos) = &input.open_position {
                let vwap = self.vwap_state.vwap;
                let price = input.trade.price.to_f64();
                let crossed = match pos.side {
                    Side::Buy => price >= vwap,
                    Side::Sell => price <= vwap,
                    _ => false,
                };
                if crossed {
                    return vec![Signal::Close { reason: ExitReason::Manual }];
                }
            }
        }

        vec![]
    }

    fn on_session_close(&mut self, input: &StrategyInput<'_>) -> Vec<Signal> {
        if input.open_position.is_some() {
            return vec![Signal::CloseAll { reason: ExitReason::SessionClose }];
        }
        vec![]
    }

    fn reset(&mut self) {
        self.vwap_state.reset();
        self.trades_taken = 0;
        self.last_candle_close_price = None;
    }

    fn clone_strategy(&self) -> Box<dyn BacktestStrategy> {
        Box::new(VwapReversionStrategy {
            config: self.config.clone(),
            params: self.params.clone(),
            vwap_state: self.vwap_state.clone(),
            trades_taken: self.trades_taken,
            last_candle_close_price: self.last_candle_close_price,
        })
    }
}
