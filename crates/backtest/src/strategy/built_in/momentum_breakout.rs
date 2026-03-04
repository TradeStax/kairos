//! Momentum Breakout strategy.
//!
//! Uses a Donchian channel for entry: goes long when price exceeds
//! the highest high of the last `entry_periods` candles, goes short
//! when below the lowest low. Stop loss is placed at entry price
//! +/- ATR * `atr_stop_multiplier`. Trailing exit closes the
//! position when price crosses back through the `exit_periods`
//! Donchian channel.

use crate::order::request::{BracketOrder, NewOrder, OrderRequest};
use crate::order::types::{OrderSide, OrderType, TimeInForce};
use crate::output::snapshot::ContextValue;
use crate::output::trade_record::ExitReason;
use crate::strategy::Strategy;
use crate::strategy::context::StrategyContext;
use crate::strategy::metadata::{StrategyCategory, StrategyMetadata};
use kairos_data::{Candle, FuturesTicker, Price, Side, Timeframe};
use kairos_study::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue, StudyConfig,
    Visibility,
};

/// Computes ATR(n) from the last `period` completed candles.
///
/// Returns 0.0 if there are fewer than 2 candles or `period` is 0.
fn compute_atr(candles: &[Candle], period: usize) -> f64 {
    if candles.len() < 2 || period == 0 {
        return 0.0;
    }
    let n = candles.len().min(period + 1);
    let slice = &candles[(candles.len() - n)..];
    let mut tr_sum = 0.0;
    let mut count = 0usize;
    for i in 1..slice.len() {
        let high = slice[i].high.to_f64();
        let low = slice[i].low.to_f64();
        let prev_close = slice[i - 1].close.to_f64();
        let tr = (high - low)
            .max((high - prev_close).abs())
            .max((low - prev_close).abs());
        tr_sum += tr;
        count += 1;
    }
    if count > 0 {
        tr_sum / count as f64
    } else {
        0.0
    }
}

/// Computes the Donchian channel (highest high, lowest low) over
/// the most recent `periods` candles.
///
/// Returns `None` if there are fewer candles than `periods` or
/// `periods` is 0.
fn donchian(candles: &[Candle], periods: usize) -> Option<(f64, f64)> {
    if candles.len() < periods || periods == 0 {
        return None;
    }
    let slice = &candles[(candles.len() - periods)..];
    let high = slice
        .iter()
        .map(|c| c.high.to_f64())
        .fold(f64::NEG_INFINITY, f64::max);
    let low = slice
        .iter()
        .map(|c| c.low.to_f64())
        .fold(f64::INFINITY, f64::min);
    Some((high, low))
}

/// Momentum Breakout strategy.
///
/// Enters on Donchian channel breakouts and uses ATR-scaled initial
/// stops with trailing exit via a shorter-period Donchian channel.
/// See the [module docs](self) for full details.
pub struct MomentumBreakoutStrategy {
    config: StudyConfig,
    params: Vec<ParameterDef>,
    /// Number of trades taken in the current session.
    trades_taken: usize,
    /// Trailing exit Donchian high (updated each candle close).
    trailing_exit_high: Option<f64>,
    /// Trailing exit Donchian low (updated each candle close).
    trailing_exit_low: Option<f64>,
    /// Direction of the currently open trade for trailing logic.
    open_side: Option<Side>,
}

/// Builds the parameter definitions for this strategy.
fn make_params() -> Vec<ParameterDef> {
    vec![
        ParameterDef {
            key: "entry_periods".into(),
            label: "Entry Periods".into(),
            description: "Donchian channel lookback for breakout entry.".into(),
            kind: ParameterKind::Integer { min: 5, max: 200 },
            default: ParameterValue::Integer(20),
            tab: ParameterTab::Parameters,
            section: None,
            order: 0,
            format: DisplayFormat::Integer { suffix: " bars" },
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "exit_periods".into(),
            label: "Exit Periods".into(),
            description: "Donchian channel lookback for trailing exit.".into(),
            kind: ParameterKind::Integer { min: 3, max: 100 },
            default: ParameterValue::Integer(10),
            tab: ParameterTab::Parameters,
            section: None,
            order: 1,
            format: DisplayFormat::Integer { suffix: " bars" },
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "atr_period".into(),
            label: "ATR Period".into(),
            description: "ATR lookback for stop calculation.".into(),
            kind: ParameterKind::Integer { min: 5, max: 100 },
            default: ParameterValue::Integer(14),
            tab: ParameterTab::Parameters,
            section: None,
            order: 2,
            format: DisplayFormat::Integer { suffix: " bars" },
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "atr_stop_multiplier".into(),
            label: "ATR Stop Multiplier".into(),
            description: "Stop distance = ATR x multiplier.".into(),
            kind: ParameterKind::Float {
                min: 0.5,
                max: 5.0,
                step: 0.25,
            },
            default: ParameterValue::Float(2.0),
            tab: ParameterTab::Parameters,
            section: None,
            order: 3,
            format: DisplayFormat::Float { decimals: 2 },
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "allow_reentry".into(),
            label: "Allow Re-entry".into(),
            description: "Allow entering new trades after the previous \
                 one closes."
                .into(),
            kind: ParameterKind::Boolean,
            default: ParameterValue::Boolean(true),
            tab: ParameterTab::Parameters,
            section: None,
            order: 4,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "max_trades".into(),
            label: "Max Trades".into(),
            description: "Maximum trades per session.".into(),
            kind: ParameterKind::Integer { min: 1, max: 20 },
            default: ParameterValue::Integer(5),
            tab: ParameterTab::Parameters,
            section: None,
            order: 5,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
    ]
}

impl MomentumBreakoutStrategy {
    /// Creates a new instance with default parameter values.
    #[must_use]
    pub fn new() -> Self {
        let params = make_params();
        let mut config = StudyConfig::new("momentum_breakout");
        for p in &params {
            config.set(p.key.clone(), p.default.clone());
        }
        Self {
            config,
            params,
            trades_taken: 0,
            trailing_exit_high: None,
            trailing_exit_low: None,
            open_side: None,
        }
    }

    fn entry_periods(&self) -> usize {
        self.config.get_int("entry_periods", 20) as usize
    }

    fn exit_periods(&self) -> usize {
        self.config.get_int("exit_periods", 10) as usize
    }

    fn atr_period(&self) -> usize {
        self.config.get_int("atr_period", 14) as usize
    }

    fn atr_stop_multiplier(&self) -> f64 {
        self.config.get_float("atr_stop_multiplier", 2.0)
    }

    fn allow_reentry(&self) -> bool {
        self.config.get_bool("allow_reentry", true)
    }

    fn max_trades(&self) -> usize {
        self.config.get_int("max_trades", 5) as usize
    }

    /// Builds a trailing stop order that cancels existing orders
    /// and submits a new stop at the given price.
    fn trailing_stop_orders(
        &self,
        instrument: FuturesTicker,
        side: OrderSide,
        trigger: Price,
        quantity: f64,
    ) -> Vec<OrderRequest> {
        vec![
            OrderRequest::CancelAll {
                instrument: Some(instrument),
            },
            OrderRequest::Submit(NewOrder {
                instrument,
                side,
                order_type: OrderType::Stop { trigger },
                quantity,
                time_in_force: TimeInForce::GTC,
                label: Some("Trailing Stop".into()),
                reduce_only: true,
            }),
        ]
    }
}

impl Default for MomentumBreakoutStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl Strategy for MomentumBreakoutStrategy {
    fn id(&self) -> &str {
        "momentum_breakout"
    }

    fn metadata(&self) -> StrategyMetadata {
        StrategyMetadata {
            id: "momentum_breakout".to_string(),
            name: "Momentum Breakout".to_string(),
            description: "Donchian channel breakout with \
                          ATR-scaled bracket orders."
                .to_string(),
            category: StrategyCategory::TrendFollowing,
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
        self.trades_taken = 0;
        self.open_side = None;
        self.trailing_exit_high = None;
        self.trailing_exit_low = None;
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

        let candles = ctx.primary_candles(timeframe);

        // --- Trailing exit on candle close ---
        if let Some(side) = self.open_side {
            if ctx.primary_position().is_some() {
                let exit_periods = self.exit_periods();
                if let Some((ex_high, ex_low)) = donchian(candles, exit_periods) {
                    self.trailing_exit_high = Some(ex_high);
                    self.trailing_exit_low = Some(ex_low);
                }

                if let Some((ex_high, ex_low)) = self.trailing_exit_high.zip(self.trailing_exit_low)
                {
                    let close = candle.close.to_f64();
                    let should_exit = match side {
                        Side::Buy => close < ex_low,
                        Side::Sell => close > ex_high,
                        Side::Bid | Side::Ask => false,
                    };
                    if should_exit {
                        self.open_side = None;
                        return vec![OrderRequest::Flatten {
                            instrument: ctx.primary_instrument,
                            reason: ExitReason::TrailingStop,
                        }];
                    }
                }

                // Update trailing stop order
                let pos_qty = ctx.primary_position().map(|p| p.quantity).unwrap_or(1.0);

                if side == Side::Buy {
                    if let Some(ex_low) = self.trailing_exit_low {
                        return self.trailing_stop_orders(
                            ctx.primary_instrument,
                            OrderSide::Sell,
                            Price::from_f64(ex_low),
                            pos_qty,
                        );
                    }
                } else if side == Side::Sell
                    && let Some(ex_high) = self.trailing_exit_high
                {
                    return self.trailing_stop_orders(
                        ctx.primary_instrument,
                        OrderSide::Buy,
                        Price::from_f64(ex_high),
                        pos_qty,
                    );
                }
            } else {
                // Position was closed externally (SL/TP)
                self.open_side = None;
                if !self.allow_reentry() || self.trades_taken >= self.max_trades() {
                    return vec![];
                }
                // Fall through to entry check below
            }
        }

        // --- Entry check ---
        if ctx.primary_position().is_some() {
            return vec![];
        }
        if self.trades_taken >= self.max_trades() {
            return vec![];
        }
        let min_candles = self.entry_periods().max(self.atr_period() + 1);
        if candles.len() < min_candles {
            return vec![];
        }

        let entry_periods = self.entry_periods();
        let (channel_high, channel_low) = match donchian(candles, entry_periods) {
            Some(v) => v,
            None => return vec![],
        };

        let atr = compute_atr(candles, self.atr_period());
        if atr == 0.0 {
            return vec![];
        }

        let close = candle.close.to_f64();
        let stop_dist = atr * self.atr_stop_multiplier();
        let exit_periods = self.exit_periods();

        // Long breakout: close above channel high
        if close > channel_high {
            let sl = Price::from_f64(close - stop_dist);
            self.trades_taken += 1;
            self.open_side = Some(Side::Buy);
            if let Some((_, ex_low)) = donchian(candles, exit_periods) {
                self.trailing_exit_low = Some(ex_low);
            }
            return vec![OrderRequest::SubmitBracket(BracketOrder {
                entry: NewOrder {
                    instrument: ctx.primary_instrument,
                    side: OrderSide::Buy,
                    order_type: OrderType::Market,
                    quantity: 1.0,
                    time_in_force: Default::default(),
                    label: Some("Breakout Long".to_string()),
                    reduce_only: false,
                },
                stop_loss: sl,
                take_profit: None,
            })];
        }

        // Short breakout: close below channel low
        if close < channel_low {
            let sl = Price::from_f64(close + stop_dist);
            self.trades_taken += 1;
            self.open_side = Some(Side::Sell);
            if let Some((ex_high, _)) = donchian(candles, exit_periods) {
                self.trailing_exit_high = Some(ex_high);
            }
            return vec![OrderRequest::SubmitBracket(BracketOrder {
                entry: NewOrder {
                    instrument: ctx.primary_instrument,
                    side: OrderSide::Sell,
                    order_type: OrderType::Market,
                    quantity: 1.0,
                    time_in_force: Default::default(),
                    label: Some("Breakout Short".to_string()),
                    reduce_only: false,
                },
                stop_loss: sl,
                take_profit: None,
            })];
        }

        vec![]
    }

    fn on_tick(&mut self, _ctx: &StrategyContext) -> Vec<OrderRequest> {
        // Momentum breakout uses candle-close logic only.
        vec![]
    }

    fn on_session_close(&mut self, ctx: &StrategyContext) -> Vec<OrderRequest> {
        if ctx.primary_position().is_some() {
            self.open_side = None;
            return vec![OrderRequest::Flatten {
                instrument: ctx.primary_instrument,
                reason: ExitReason::SessionClose,
            }];
        }
        vec![]
    }

    fn trade_context(&self, ctx: &StrategyContext) -> Vec<(String, ContextValue)> {
        let mut values = Vec::new();
        // Find the primary candle set by scanning for the primary
        // instrument's longest candle vec (the primary timeframe).
        let candles: &[Candle] = ctx
            .candles
            .iter()
            .filter(|((inst, _), _)| *inst == ctx.primary_instrument)
            .max_by_key(|(_, v)| v.len())
            .map(|(_, v)| v.as_slice())
            .unwrap_or(&[]);
        if let Some((ch, cl)) = donchian(candles, self.entry_periods()) {
            values.push(("channel_high".into(), ContextValue::Float(ch)));
            values.push(("channel_low".into(), ContextValue::Float(cl)));
        }
        if let Some(h) = self.trailing_exit_high {
            values.push(("trailing_exit_high".into(), ContextValue::Float(h)));
        }
        if let Some(l) = self.trailing_exit_low {
            values.push(("trailing_exit_low".into(), ContextValue::Float(l)));
        }
        let atr = compute_atr(candles, self.atr_period());
        values.push(("atr".into(), ContextValue::Float(atr)));
        values.push((
            "entry_periods".into(),
            ContextValue::Integer(self.entry_periods() as i64),
        ));
        values.push((
            "exit_periods".into(),
            ContextValue::Integer(self.exit_periods() as i64),
        ));
        values.push((
            "atr_period".into(),
            ContextValue::Integer(self.atr_period() as i64),
        ));
        values.push((
            "atr_stop_multiplier".into(),
            ContextValue::Float(self.atr_stop_multiplier()),
        ));
        values.push((
            "open_side".into(),
            ContextValue::Text(
                match self.open_side {
                    Some(Side::Buy) => "Long",
                    Some(Side::Sell) => "Short",
                    _ => "None",
                }
                .to_string(),
            ),
        ));
        values.push((
            "trades_taken".into(),
            ContextValue::Integer(self.trades_taken as i64),
        ));
        values
    }

    fn reset(&mut self) {
        self.trades_taken = 0;
        self.open_side = None;
        self.trailing_exit_high = None;
        self.trailing_exit_low = None;
    }

    fn clone_strategy(&self) -> Box<dyn Strategy> {
        Box::new(MomentumBreakoutStrategy {
            config: self.config.clone(),
            params: self.params.clone(),
            trades_taken: self.trades_taken,
            trailing_exit_high: self.trailing_exit_high,
            trailing_exit_low: self.trailing_exit_low,
            open_side: self.open_side,
        })
    }
}
