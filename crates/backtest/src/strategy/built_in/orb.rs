//! Opening Range Breakout (ORB) strategy.
//!
//! Accumulates the high/low of the first `or_minutes` of RTH session,
//! then watches for a breakout above or below that range.

use crate::order::request::{BracketOrder, NewOrder, OrderRequest};
use crate::order::types::{OrderSide, OrderType};
use crate::output::trade_record::ExitReason;
use crate::strategy::Strategy;
use crate::strategy::context::{SessionState, StrategyContext};
use crate::strategy::metadata::{StrategyCategory, StrategyMetadata};
use kairos_data::{Candle, FuturesTicker, Price, Timeframe};
use kairos_study::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue, StudyConfig,
    Visibility,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrbState {
    /// Waiting for first trade of RTH session.
    WaitingForOpen,
    /// Accumulating the opening range.
    AccumulatingOR,
    /// OR complete, watching for breakout.
    WatchingForBreakout,
    /// In a trade.
    InTrade,
    /// Max trades reached or time exit triggered.
    Done,
}

/// Opening Range Breakout strategy.
pub struct OrbStrategy {
    config: StudyConfig,
    params: Vec<ParameterDef>,
    state: OrbState,
    or_high: Option<Price>,
    or_low: Option<Price>,
    or_start_ms: u64,
    trades_taken: usize,
}

fn make_params() -> Vec<ParameterDef> {
    vec![
        ParameterDef {
            key: "or_minutes".into(),
            label: "OR Minutes".into(),
            description: "Number of minutes to accumulate the \
                          opening range."
                .into(),
            kind: ParameterKind::Integer { min: 5, max: 120 },
            default: ParameterValue::Integer(30),
            tab: ParameterTab::Parameters,
            section: None,
            order: 0,
            format: DisplayFormat::Integer { suffix: " min" },
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "tp_multiple".into(),
            label: "TP Multiple".into(),
            description: "Take-profit distance as a multiple of \
                          the OR range."
                .into(),
            kind: ParameterKind::Float {
                min: 0.5,
                max: 5.0,
                step: 0.25,
            },
            default: ParameterValue::Float(1.5),
            tab: ParameterTab::Parameters,
            section: None,
            order: 1,
            format: DisplayFormat::Float { decimals: 2 },
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "max_trades".into(),
            label: "Max Trades".into(),
            description: "Maximum trades per session (1-3).".into(),
            kind: ParameterKind::Integer { min: 1, max: 3 },
            default: ParameterValue::Integer(1),
            tab: ParameterTab::Parameters,
            section: None,
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "wick_filter".into(),
            label: "Wick Filter".into(),
            description: "Require a candle close beyond the OR \
                          level (reduces false breakouts)."
                .into(),
            kind: ParameterKind::Boolean,
            default: ParameterValue::Boolean(true),
            tab: ParameterTab::Parameters,
            section: None,
            order: 3,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "time_exit_hhmm".into(),
            label: "Time Exit".into(),
            description: "Close any open position at this local \
                          time (HHMM)."
                .into(),
            kind: ParameterKind::Integer {
                min: 1200,
                max: 1545,
            },
            default: ParameterValue::Integer(1500),
            tab: ParameterTab::Parameters,
            section: None,
            order: 4,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "gap_skip".into(),
            label: "Skip Gap Days".into(),
            description: "Skip sessions that open with a gap > \
                          1 OR range."
                .into(),
            kind: ParameterKind::Boolean,
            default: ParameterValue::Boolean(true),
            tab: ParameterTab::Parameters,
            section: None,
            order: 5,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
    ]
}

impl OrbStrategy {
    pub fn new() -> Self {
        let params = make_params();
        let mut config = StudyConfig::new("orb");
        for p in &params {
            config.set(p.key.clone(), p.default.clone());
        }
        Self {
            config,
            params,
            state: OrbState::WaitingForOpen,
            or_high: None,
            or_low: None,
            or_start_ms: 0,
            trades_taken: 0,
        }
    }

    fn or_minutes(&self) -> u64 {
        self.config.get_int("or_minutes", 30) as u64
    }

    fn tp_multiple(&self) -> f64 {
        self.config.get_float("tp_multiple", 1.5)
    }

    fn max_trades(&self) -> usize {
        self.config.get_int("max_trades", 1) as usize
    }

    fn wick_filter(&self) -> bool {
        self.config.get_bool("wick_filter", true)
    }

    fn time_exit_hhmm(&self) -> u32 {
        self.config.get_int("time_exit_hhmm", 1500) as u32
    }
}

impl Default for OrbStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl Strategy for OrbStrategy {
    fn id(&self) -> &str {
        "orb"
    }

    fn metadata(&self) -> StrategyMetadata {
        StrategyMetadata {
            id: "orb".to_string(),
            name: "Opening Range Breakout".to_string(),
            description: "Trades breakouts above/below the first \
                          N minutes of the RTH session."
                .to_string(),
            category: StrategyCategory::BreakoutMomentum,
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

    fn on_session_open(&mut self, ctx: &StrategyContext) -> Vec<OrderRequest> {
        self.state = OrbState::AccumulatingOR;
        self.or_high = Some(ctx.trade.price);
        self.or_low = Some(ctx.trade.price);
        self.or_start_ms = ctx.trade.time.0;
        self.trades_taken = 0;
        vec![]
    }

    fn on_candle(
        &mut self,
        instrument: FuturesTicker,
        _timeframe: Timeframe,
        candle: &Candle,
        ctx: &StrategyContext,
    ) -> Vec<OrderRequest> {
        if instrument != ctx.primary_instrument {
            return vec![];
        }

        let tick_size = ctx.tick_size();

        // Time exit check
        if ctx.local_hhmm >= self.time_exit_hhmm() {
            if ctx.primary_position().is_some() {
                self.state = OrbState::Done;
                return vec![OrderRequest::Flatten {
                    instrument: ctx.primary_instrument,
                    reason: ExitReason::SessionClose,
                }];
            }
            self.state = OrbState::Done;
            return vec![];
        }

        // Wick-filter breakout entry: triggered on candle close
        // beyond OR level
        if self.state == OrbState::WatchingForBreakout
            && self.wick_filter()
            && ctx.primary_position().is_none()
            && self.trades_taken < self.max_trades()
        {
            let or_high = match self.or_high {
                Some(h) => h,
                None => return vec![],
            };
            let or_low = match self.or_low {
                Some(l) => l,
                None => return vec![],
            };

            let or_range = or_high - or_low;
            if or_range.units() <= 0 {
                return vec![];
            }

            if candle.close > or_high {
                // Long breakout
                let entry = candle.close;
                let sl = or_low - tick_size;
                let tp = entry + Price::from_f64(or_range.to_f64() * self.tp_multiple());
                self.trades_taken += 1;
                self.state = OrbState::InTrade;
                return vec![OrderRequest::SubmitBracket(BracketOrder {
                    entry: NewOrder {
                        instrument: ctx.primary_instrument,
                        side: OrderSide::Buy,
                        order_type: OrderType::Market,
                        quantity: 1.0,
                        time_in_force: Default::default(),
                        label: Some("ORB Long".to_string()),
                        reduce_only: false,
                    },
                    stop_loss: sl,
                    take_profit: Some(tp),
                })];
            }

            if candle.close < or_low {
                // Short breakout
                let entry = candle.close;
                let sl = or_high + tick_size;
                let tp = entry - Price::from_f64(or_range.to_f64() * self.tp_multiple());
                self.trades_taken += 1;
                self.state = OrbState::InTrade;
                return vec![OrderRequest::SubmitBracket(BracketOrder {
                    entry: NewOrder {
                        instrument: ctx.primary_instrument,
                        side: OrderSide::Sell,
                        order_type: OrderType::Market,
                        quantity: 1.0,
                        time_in_force: Default::default(),
                        label: Some("ORB Short".to_string()),
                        reduce_only: false,
                    },
                    stop_loss: sl,
                    take_profit: Some(tp),
                })];
            }
        }

        vec![]
    }

    fn on_tick(&mut self, ctx: &StrategyContext) -> Vec<OrderRequest> {
        if ctx.session_state != SessionState::Open {
            return vec![];
        }

        let tick_size = ctx.tick_size();

        // Time exit check
        if ctx.local_hhmm >= self.time_exit_hhmm() {
            if ctx.primary_position().is_some() {
                self.state = OrbState::Done;
                return vec![OrderRequest::Flatten {
                    instrument: ctx.primary_instrument,
                    reason: ExitReason::SessionClose,
                }];
            }
            self.state = OrbState::Done;
            return vec![];
        }

        // Accumulate OR
        if self.state == OrbState::AccumulatingOR {
            let elapsed_ms = ctx.trade.time.0.saturating_sub(self.or_start_ms);
            let or_ms = self.or_minutes() * 60_000;

            let price = ctx.trade.price;
            if let Some(h) = &mut self.or_high
                && price > *h
            {
                *h = price;
            }
            if let Some(l) = &mut self.or_low
                && price < *l
            {
                *l = price;
            }

            if elapsed_ms >= or_ms {
                self.state = OrbState::WatchingForBreakout;
            }
            return vec![];
        }

        // Tick-based (no wick filter) breakout entry
        if self.state == OrbState::WatchingForBreakout
            && !self.wick_filter()
            && ctx.primary_position().is_none()
            && self.trades_taken < self.max_trades()
        {
            let or_high = match self.or_high {
                Some(h) => h,
                None => return vec![],
            };
            let or_low = match self.or_low {
                Some(l) => l,
                None => return vec![],
            };

            let or_range = or_high - or_low;
            if or_range.units() <= 0 {
                return vec![];
            }

            let price = ctx.trade.price;

            if price > or_high {
                let sl = or_low - tick_size;
                let tp = price + Price::from_f64(or_range.to_f64() * self.tp_multiple());
                self.trades_taken += 1;
                self.state = OrbState::InTrade;
                return vec![OrderRequest::SubmitBracket(BracketOrder {
                    entry: NewOrder {
                        instrument: ctx.primary_instrument,
                        side: OrderSide::Buy,
                        order_type: OrderType::Market,
                        quantity: 1.0,
                        time_in_force: Default::default(),
                        label: Some("ORB Long".to_string()),
                        reduce_only: false,
                    },
                    stop_loss: sl,
                    take_profit: Some(tp),
                })];
            }

            if price < or_low {
                let sl = or_high + tick_size;
                let tp = price - Price::from_f64(or_range.to_f64() * self.tp_multiple());
                self.trades_taken += 1;
                self.state = OrbState::InTrade;
                return vec![OrderRequest::SubmitBracket(BracketOrder {
                    entry: NewOrder {
                        instrument: ctx.primary_instrument,
                        side: OrderSide::Sell,
                        order_type: OrderType::Market,
                        quantity: 1.0,
                        time_in_force: Default::default(),
                        label: Some("ORB Short".to_string()),
                        reduce_only: false,
                    },
                    stop_loss: sl,
                    take_profit: Some(tp),
                })];
            }
        }

        // If trade finished, allow re-entry up to max_trades
        if self.state == OrbState::InTrade && ctx.primary_position().is_none() {
            if self.trades_taken < self.max_trades() {
                self.state = OrbState::WatchingForBreakout;
            } else {
                self.state = OrbState::Done;
            }
        }

        vec![]
    }

    fn on_session_close(&mut self, ctx: &StrategyContext) -> Vec<OrderRequest> {
        if ctx.primary_position().is_some() {
            self.state = OrbState::Done;
            return vec![OrderRequest::Flatten {
                instrument: ctx.primary_instrument,
                reason: ExitReason::SessionClose,
            }];
        }
        self.state = OrbState::Done;
        vec![]
    }

    fn reset(&mut self) {
        self.state = OrbState::WaitingForOpen;
        self.or_high = None;
        self.or_low = None;
        self.or_start_ms = 0;
        self.trades_taken = 0;
    }

    fn clone_strategy(&self) -> Box<dyn Strategy> {
        Box::new(OrbStrategy {
            config: self.config.clone(),
            params: self.params.clone(),
            state: self.state,
            or_high: self.or_high,
            or_low: self.or_low,
            or_start_ms: self.or_start_ms,
            trades_taken: self.trades_taken,
        })
    }
}
