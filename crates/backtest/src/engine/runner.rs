use std::sync::Arc;

use kairos_data::{Candle, Price, Side, Timestamp, Trade, TradeRepository, Volume};
use uuid::Uuid;

use crate::config::backtest::BacktestConfig;
use crate::core::input::{OpenPositionView, PartialCandleView, SessionState, StrategyInput};
use crate::core::signal::Signal;
use crate::core::strategy::{BacktestError, BacktestStrategy};
use crate::domain::metrics::PerformanceMetrics;
use crate::domain::progress::BacktestProgressEvent;
use crate::domain::result::BacktestResult;
use crate::domain::trade_record::{ExitReason, TradeRecord};
use crate::engine::execution::{FillDirection, SimulatedBroker};
use crate::engine::feed::HistoricalFeed;
use crate::engine::session::{SessionClock, SessionEvent, SessionState as SState};
use crate::portfolio::equity::EquityCurve;
use crate::portfolio::position::OpenPosition;

/// Orchestrates a full backtest run over historical tick data.
pub struct BacktestRunner {
    trade_repo: Arc<dyn TradeRepository + Send + Sync>,
}

impl BacktestRunner {
    pub fn new(trade_repo: Arc<dyn TradeRepository + Send + Sync>) -> Self {
        Self { trade_repo }
    }

    /// Execute the backtest and return a complete `BacktestResult`.
    pub async fn run(
        &self,
        config: BacktestConfig,
        mut strategy: Box<dyn BacktestStrategy>,
    ) -> Result<BacktestResult, BacktestError> {
        config.validate().map_err(BacktestError::Engine)?;

        let strategy_name = strategy.metadata().name.clone();
        let run_started_at_ms = system_time_ms();

        // Apply strategy parameter overrides from config
        for (key, value) in &config.strategy_params {
            strategy.set_parameter(key, value.clone())?;
        }

        // Load historical trades
        let raw_trades = self
            .trade_repo
            .get_trades(&config.ticker, &config.date_range)
            .await
            .map_err(|e| BacktestError::Data(e.to_string()))?;

        let total_data_trades = raw_trades.len();
        let initial_capital = config.initial_capital_usd;

        if raw_trades.is_empty() {
            let equity_curve = EquityCurve::new(initial_capital);
            let metrics = PerformanceMetrics::compute(
                &[],
                initial_capital,
                0,
                config.risk.risk_free_annual,
                &equity_curve,
            );
            return Ok(BacktestResult {
                id: Uuid::new_v4(),
                config,
                strategy_name,
                run_started_at_ms,
                run_duration_ms: system_time_ms().saturating_sub(run_started_at_ms),
                total_data_trades: 0,
                trades: vec![],
                metrics,
                equity_curve,
                sessions_processed: 0,
                sessions_skipped: 0,
                warnings: vec!["No data found for the configured date range.".to_string()],
            });
        }

        // Contract specifications
        let tick_size = get_tick_size(&config);
        let contract_multiplier = get_contract_multiplier(&config);
        let tick_value = tick_size.to_f64() * contract_multiplier;

        // Engine components
        let mut session_clock = SessionClock::new(
            config.timezone_offset_hours,
            config.rth_open_hhmm,
            config.rth_close_hhmm,
        );
        let mut broker = SimulatedBroker::new(config.slippage.clone(), tick_size);
        let mut candle_agg = CandleAggregator::new(config.timeframe.to_milliseconds());

        // State
        let mut open_position: Option<OpenPosition> = None;
        let mut completed_trades: Vec<TradeRecord> = Vec::new();
        let mut equity_curve = EquityCurve::new(initial_capital);
        let mut realized_equity = initial_capital;
        let mut completed_candles: Vec<Candle> = Vec::new();
        let mut sessions_processed: usize = 0;
        let mut warnings: Vec<String> = Vec::new();
        let mut trade_index: usize = 0;
        let mut tick_count: u32 = 0;
        let mut last_trade: Option<Trade> = None;

        let feed = HistoricalFeed::new(raw_trades);

        for trade in feed {
            // --- 1. Advance session clock ---
            let session_event = session_clock.advance(trade.time);

            // --- 2. Handle session boundary ---
            if let Some(event) = session_event {
                match event {
                    SessionEvent::Open { .. } => {
                        sessions_processed += 1;
                        broker.mark_new_session();
                        let input = build_input(
                            &trade,
                            &completed_candles,
                            candle_agg.partial_view(),
                            &open_position,
                            tick_size,
                            contract_multiplier as f32,
                            &session_clock,
                        );
                        let signals = strategy.on_session_open(&input);
                        process_signals(
                            signals,
                            &trade,
                            &mut open_position,
                            &mut completed_trades,
                            &mut realized_equity,
                            &mut equity_curve,
                            &mut broker,
                            tick_size,
                            tick_value,
                            contract_multiplier,
                            config.commission_per_side_usd,
                            &mut trade_index,
                        );
                    }
                    SessionEvent::Close { .. } => {
                        broker.mark_new_session();
                        let input = build_input(
                            &trade,
                            &completed_candles,
                            candle_agg.partial_view(),
                            &open_position,
                            tick_size,
                            contract_multiplier as f32,
                            &session_clock,
                        );
                        let signals = strategy.on_session_close(&input);
                        process_signals(
                            signals,
                            &trade,
                            &mut open_position,
                            &mut completed_trades,
                            &mut realized_equity,
                            &mut equity_curve,
                            &mut broker,
                            tick_size,
                            tick_value,
                            contract_multiplier,
                            config.commission_per_side_usd,
                            &mut trade_index,
                        );
                    }
                }
            }

            // --- 3. Candle aggregation ---
            if let Some(closed_candle) = candle_agg.update(&trade) {
                completed_candles.push(closed_candle);
                let input = build_input(
                    &trade,
                    &completed_candles,
                    candle_agg.partial_view(),
                    &open_position,
                    tick_size,
                    contract_multiplier as f32,
                    &session_clock,
                );
                let signals = strategy.on_candle_close(&closed_candle, &input);
                process_signals(
                    signals,
                    &trade,
                    &mut open_position,
                    &mut completed_trades,
                    &mut realized_equity,
                    &mut equity_curve,
                    &mut broker,
                    tick_size,
                    tick_value,
                    contract_multiplier,
                    config.commission_per_side_usd,
                    &mut trade_index,
                );
            }

            // --- 4. SL/TP fill check ---
            if let Some(pos) = &open_position {
                if let Some(fill) = broker.check_fills(&trade, pos) {
                    let record = close_position(
                        pos,
                        &trade,
                        fill.fill_price,
                        fill.exit_reason,
                        tick_size,
                        tick_value,
                        contract_multiplier,
                        config.commission_per_side_usd,
                        &mut trade_index,
                    );
                    realized_equity += record.pnl_net_usd;
                    equity_curve.record(trade.time, realized_equity, 0.0);
                    completed_trades.push(record);
                    open_position = None;
                }
            }

            // --- 5. Update position extremes ---
            if let Some(pos) = &mut open_position {
                pos.update_extremes(trade.price);
            }

            // --- 6. on_tick (RTH only) ---
            if session_clock.session_state == SState::Open {
                let input = build_input(
                    &trade,
                    &completed_candles,
                    candle_agg.partial_view(),
                    &open_position,
                    tick_size,
                    contract_multiplier as f32,
                    &session_clock,
                );
                let signals = strategy.on_tick(&input);
                process_signals(
                    signals,
                    &trade,
                    &mut open_position,
                    &mut completed_trades,
                    &mut realized_equity,
                    &mut equity_curve,
                    &mut broker,
                    tick_size,
                    tick_value,
                    contract_multiplier,
                    config.commission_per_side_usd,
                    &mut trade_index,
                );

                // Periodic equity curve sampling (every 100 ticks)
                tick_count += 1;
                if tick_count >= 100 {
                    tick_count = 0;
                    let unr = open_position
                        .as_ref()
                        .map(|p| p.unrealized_pnl(trade.price, tick_size, tick_value))
                        .unwrap_or(0.0);
                    equity_curve.record(trade.time, realized_equity, unr);
                }
            }

            last_trade = Some(trade);
        }

        // Force-close any remaining open position at end of data
        if let Some(pos) = open_position.take() {
            let last_price =
                last_trade.map(|t| t.price).unwrap_or(pos.entry_price);
            let fake_trade = Trade::new(
                last_trade.map(|t| t.time).unwrap_or(pos.entry_time),
                last_price,
                kairos_data::Quantity(0.0),
                pos.side,
            );
            let record = close_position(
                &pos,
                &fake_trade,
                last_price,
                ExitReason::SessionClose,
                tick_size,
                tick_value,
                contract_multiplier,
                config.commission_per_side_usd,
                &mut trade_index,
            );
            realized_equity += record.pnl_net_usd;
            equity_curve.record(fake_trade.time, realized_equity, 0.0);
            completed_trades.push(record);
            warnings
                .push("Position still open at end of data — closed at last available price."
                    .to_string());
        }

        let trading_days = sessions_processed;
        let metrics = PerformanceMetrics::compute(
            &completed_trades,
            initial_capital,
            trading_days,
            config.risk.risk_free_annual,
            &equity_curve,
        );

        let run_duration_ms = system_time_ms().saturating_sub(run_started_at_ms);

        Ok(BacktestResult {
            id: Uuid::new_v4(),
            config,
            strategy_name,
            run_started_at_ms,
            run_duration_ms,
            total_data_trades,
            trades: completed_trades,
            metrics,
            equity_curve,
            sessions_processed,
            sessions_skipped: 0,
            warnings,
        })
    }

    /// Execute the backtest, emitting progress events to a shared buffer.
    /// The returned `BacktestResult.id` matches the supplied `run_id`.
    pub async fn run_with_progress(
        &self,
        config: BacktestConfig,
        mut strategy: Box<dyn BacktestStrategy>,
        run_id: Uuid,
        sender: &'static tokio::sync::mpsc::UnboundedSender<BacktestProgressEvent>,
    ) -> Result<BacktestResult, BacktestError> {
        config.validate().map_err(BacktestError::Engine)?;

        let strategy_name = strategy.metadata().name.clone();
        let run_started_at_ms = system_time_ms();

        for (key, value) in &config.strategy_params {
            strategy.set_parameter(key, value.clone())?;
        }

        let raw_trades = self
            .trade_repo
            .get_trades(&config.ticker, &config.date_range)
            .await
            .map_err(|e| BacktestError::Data(e.to_string()))?;

        let total_data_trades = raw_trades.len();
        let initial_capital = config.initial_capital_usd;

        if raw_trades.is_empty() {
            let equity_curve = EquityCurve::new(initial_capital);
            let metrics = PerformanceMetrics::compute(
                &[],
                initial_capital,
                0,
                config.risk.risk_free_annual,
                &equity_curve,
            );
            return Ok(BacktestResult {
                id: run_id,
                config,
                strategy_name,
                run_started_at_ms,
                run_duration_ms: system_time_ms()
                    .saturating_sub(run_started_at_ms),
                total_data_trades: 0,
                trades: vec![],
                metrics,
                equity_curve,
                sessions_processed: 0,
                sessions_skipped: 0,
                warnings: vec![
                    "No data found for the configured date range."
                        .to_string(),
                ],
            });
        }

        // Estimate total sessions from date range span
        let total_estimated_sessions = config
            .date_range
            .num_days()
            .max(1) as usize;

        let tick_size = get_tick_size(&config);
        let contract_multiplier = get_contract_multiplier(&config);
        let tick_value = tick_size.to_f64() * contract_multiplier;

        let mut session_clock = SessionClock::new(
            config.timezone_offset_hours,
            config.rth_open_hhmm,
            config.rth_close_hhmm,
        );
        let mut broker =
            SimulatedBroker::new(config.slippage.clone(), tick_size);
        let mut candle_agg =
            CandleAggregator::new(config.timeframe.to_milliseconds());

        let mut open_position: Option<OpenPosition> = None;
        let mut completed_trades: Vec<TradeRecord> = Vec::new();
        let mut equity_curve = EquityCurve::new(initial_capital);
        let mut realized_equity = initial_capital;
        let mut completed_candles: Vec<Candle> = Vec::new();
        let mut sessions_processed: usize = 0;
        let mut warnings: Vec<String> = Vec::new();
        let mut trade_index: usize = 0;
        let mut tick_count: u32 = 0;
        let mut last_trade: Option<Trade> = None;

        let feed = HistoricalFeed::new(raw_trades);

        for trade in feed {
            let session_event = session_clock.advance(trade.time);

            if let Some(event) = session_event {
                match event {
                    SessionEvent::Open { .. } => {
                        sessions_processed += 1;
                        emit(
                            sender,
                            BacktestProgressEvent::SessionProcessed {
                                run_id,
                                index: sessions_processed,
                                total_estimated: total_estimated_sessions,
                            },
                        );
                        broker.mark_new_session();
                        let input = build_input(
                            &trade,
                            &completed_candles,
                            candle_agg.partial_view(),
                            &open_position,
                            tick_size,
                            contract_multiplier as f32,
                            &session_clock,
                        );
                        let signals =
                            strategy.on_session_open(&input);
                        process_signals_with_progress(
                            signals,
                            &trade,
                            &mut open_position,
                            &mut completed_trades,
                            &mut realized_equity,
                            &mut equity_curve,
                            &mut broker,
                            tick_size,
                            tick_value,
                            contract_multiplier,
                            config.commission_per_side_usd,
                            &mut trade_index,
                            run_id,
                            sender,
                        );
                    }
                    SessionEvent::Close { .. } => {
                        broker.mark_new_session();
                        let input = build_input(
                            &trade,
                            &completed_candles,
                            candle_agg.partial_view(),
                            &open_position,
                            tick_size,
                            contract_multiplier as f32,
                            &session_clock,
                        );
                        let signals =
                            strategy.on_session_close(&input);
                        process_signals_with_progress(
                            signals,
                            &trade,
                            &mut open_position,
                            &mut completed_trades,
                            &mut realized_equity,
                            &mut equity_curve,
                            &mut broker,
                            tick_size,
                            tick_value,
                            contract_multiplier,
                            config.commission_per_side_usd,
                            &mut trade_index,
                            run_id,
                            sender,
                        );
                    }
                }
            }

            if let Some(closed_candle) = candle_agg.update(&trade) {
                completed_candles.push(closed_candle);
                let input = build_input(
                    &trade,
                    &completed_candles,
                    candle_agg.partial_view(),
                    &open_position,
                    tick_size,
                    contract_multiplier as f32,
                    &session_clock,
                );
                let signals =
                    strategy.on_candle_close(&closed_candle, &input);
                process_signals_with_progress(
                    signals,
                    &trade,
                    &mut open_position,
                    &mut completed_trades,
                    &mut realized_equity,
                    &mut equity_curve,
                    &mut broker,
                    tick_size,
                    tick_value,
                    contract_multiplier,
                    config.commission_per_side_usd,
                    &mut trade_index,
                    run_id,
                    sender,
                );
            }

            if let Some(pos) = &open_position {
                if let Some(fill) = broker.check_fills(&trade, pos) {
                    let record = close_position(
                        pos,
                        &trade,
                        fill.fill_price,
                        fill.exit_reason,
                        tick_size,
                        tick_value,
                        contract_multiplier,
                        config.commission_per_side_usd,
                        &mut trade_index,
                    );
                    realized_equity += record.pnl_net_usd;
                    equity_curve
                        .record(trade.time, realized_equity, 0.0);
                    emit(
                        sender,
                        BacktestProgressEvent::TradeCompleted {
                            run_id,
                            trade: record.clone(),
                        },
                    );
                    completed_trades.push(record);
                    open_position = None;
                }
            }

            if let Some(pos) = &mut open_position {
                pos.update_extremes(trade.price);
            }

            if session_clock.session_state == SState::Open {
                let input = build_input(
                    &trade,
                    &completed_candles,
                    candle_agg.partial_view(),
                    &open_position,
                    tick_size,
                    contract_multiplier as f32,
                    &session_clock,
                );
                let signals = strategy.on_tick(&input);
                process_signals_with_progress(
                    signals,
                    &trade,
                    &mut open_position,
                    &mut completed_trades,
                    &mut realized_equity,
                    &mut equity_curve,
                    &mut broker,
                    tick_size,
                    tick_value,
                    contract_multiplier,
                    config.commission_per_side_usd,
                    &mut trade_index,
                    run_id,
                    sender,
                );

                tick_count += 1;
                if tick_count >= 100 {
                    tick_count = 0;
                    let unr = open_position
                        .as_ref()
                        .map(|p| {
                            p.unrealized_pnl(
                                trade.price,
                                tick_size,
                                tick_value,
                            )
                        })
                        .unwrap_or(0.0);
                    equity_curve
                        .record(trade.time, realized_equity, unr);
                    emit(
                        sender,
                        BacktestProgressEvent::EquityUpdate {
                            run_id,
                            point: crate::portfolio::equity::EquityPoint {
                                timestamp: trade.time,
                                realized_equity_usd: realized_equity,
                                unrealized_pnl_usd: unr,
                                total_equity_usd: realized_equity + unr,
                            },
                        },
                    );
                }
            }

            last_trade = Some(trade);
        }

        // Force-close any remaining open position
        if let Some(pos) = open_position.take() {
            let last_price =
                last_trade.map(|t| t.price).unwrap_or(pos.entry_price);
            let fake_trade = Trade::new(
                last_trade
                    .map(|t| t.time)
                    .unwrap_or(pos.entry_time),
                last_price,
                kairos_data::Quantity(0.0),
                pos.side,
            );
            let record = close_position(
                &pos,
                &fake_trade,
                last_price,
                ExitReason::SessionClose,
                tick_size,
                tick_value,
                contract_multiplier,
                config.commission_per_side_usd,
                &mut trade_index,
            );
            realized_equity += record.pnl_net_usd;
            equity_curve
                .record(fake_trade.time, realized_equity, 0.0);
            emit(
                sender,
                BacktestProgressEvent::TradeCompleted {
                    run_id,
                    trade: record.clone(),
                },
            );
            completed_trades.push(record);
            warnings.push(
                "Position still open at end of data \
                 — closed at last available price."
                    .to_string(),
            );
        }

        let trading_days = sessions_processed;
        let metrics = PerformanceMetrics::compute(
            &completed_trades,
            initial_capital,
            trading_days,
            config.risk.risk_free_annual,
            &equity_curve,
        );

        let run_duration_ms =
            system_time_ms().saturating_sub(run_started_at_ms);

        Ok(BacktestResult {
            id: run_id,
            config,
            strategy_name,
            run_started_at_ms,
            run_duration_ms,
            total_data_trades,
            trades: completed_trades,
            metrics,
            equity_curve,
            sessions_processed,
            sessions_skipped: 0,
            warnings,
        })
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn system_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Return the tick size for the configured ticker.
/// In production this would look up a registry; here we provide sensible defaults.
fn get_tick_size(config: &BacktestConfig) -> Price {
    let product = config.ticker.product();
    match product {
        "ES" => Price::from_f64(0.25),
        "NQ" => Price::from_f64(0.25),
        "YM" => Price::from_f64(1.0),
        "RTY" => Price::from_f64(0.10),
        "GC" => Price::from_f64(0.10),
        "CL" => Price::from_f64(0.01),
        "ZN" | "ZB" | "ZF" => Price::from_f64(0.015625),
        _ => Price::from_f64(0.25),
    }
}

/// Return the USD value of one tick for the configured ticker.
fn get_contract_multiplier(config: &BacktestConfig) -> f64 {
    let product = config.ticker.product();
    match product {
        "ES" => 50.0,
        "NQ" => 20.0,
        "YM" => 5.0,
        "RTY" => 50.0,
        "GC" => 100.0,
        "CL" => 1_000.0,
        "ZN" => 1_000.0,
        "ZB" => 1_000.0,
        "ZF" => 1_000.0,
        _ => 50.0,
    }
}

// ─── Candle Aggregator ──────────────────────────────────────────────────────

struct PartialCandle {
    bucket_start: u64,
    open: Price,
    high: Price,
    low: Price,
    close: Price,
    buy_volume: f64,
    sell_volume: f64,
}

struct CandleAggregator {
    timeframe_ms: u64,
    partial: Option<PartialCandle>,
}

impl CandleAggregator {
    fn new(timeframe_ms: u64) -> Self {
        Self { timeframe_ms, partial: None }
    }

    fn update(&mut self, trade: &Trade) -> Option<Candle> {
        let bucket = (trade.time.0 / self.timeframe_ms) * self.timeframe_ms;

        match &mut self.partial {
            None => {
                self.partial = Some(PartialCandle {
                    bucket_start: bucket,
                    open: trade.price,
                    high: trade.price,
                    low: trade.price,
                    close: trade.price,
                    buy_volume: if trade.side == Side::Buy {
                        trade.quantity.0
                    } else {
                        0.0
                    },
                    sell_volume: if trade.side == Side::Sell {
                        trade.quantity.0
                    } else {
                        0.0
                    },
                });
                None
            }
            Some(bar) if bar.bucket_start == bucket => {
                if trade.price > bar.high {
                    bar.high = trade.price;
                }
                if trade.price < bar.low {
                    bar.low = trade.price;
                }
                bar.close = trade.price;
                match trade.side {
                    Side::Buy => bar.buy_volume += trade.quantity.0,
                    Side::Sell => bar.sell_volume += trade.quantity.0,
                    _ => {}
                }
                None
            }
            Some(old_bar) => {
                // Close the old bar and start a new one
                let closed = Candle::new(
                    Timestamp(old_bar.bucket_start),
                    old_bar.open,
                    old_bar.high,
                    old_bar.low,
                    old_bar.close,
                    Volume(old_bar.buy_volume),
                    Volume(old_bar.sell_volume),
                )
                .ok();

                self.partial = Some(PartialCandle {
                    bucket_start: bucket,
                    open: trade.price,
                    high: trade.price,
                    low: trade.price,
                    close: trade.price,
                    buy_volume: if trade.side == Side::Buy {
                        trade.quantity.0
                    } else {
                        0.0
                    },
                    sell_volume: if trade.side == Side::Sell {
                        trade.quantity.0
                    } else {
                        0.0
                    },
                });

                closed
            }
        }
    }

    fn partial_view(&self) -> Option<PartialCandleView> {
        self.partial.as_ref().map(|p| PartialCandleView {
            open: p.open,
            high: p.high,
            low: p.low,
            close: p.close,
            buy_volume: p.buy_volume,
            sell_volume: p.sell_volume,
            bucket_start_ms: p.bucket_start,
        })
    }
}

// ─── Input Builder ──────────────────────────────────────────────────────────

fn build_input<'a>(
    trade: &'a Trade,
    candles: &'a [Candle],
    partial: Option<PartialCandleView>,
    position: &Option<OpenPosition>,
    tick_size: Price,
    contract_size: f32,
    clock: &SessionClock,
) -> StrategyInput<'a> {
    let hhmm = clock.local_hhmm(trade.time);
    let open_view = position.as_ref().map(|p| OpenPositionView {
        side: p.side,
        entry_price: p.entry_price,
        entry_time: p.entry_time,
        quantity: p.quantity,
        stop_loss: p.stop_loss,
        take_profit: p.take_profit,
        mae: p.mae,
        mfe: p.mfe,
    });
    let session_state = match clock.session_state {
        SState::Open => SessionState::Open,
        SState::Closed => SessionState::Closed,
        SState::PreMarket => SessionState::PreMarket,
    };
    StrategyInput {
        trade,
        candles,
        candle_in_progress: partial,
        tick_size,
        contract_size,
        session_state,
        local_hour: hhmm / 100,
        local_minute: hhmm % 100,
        local_hhmm: hhmm,
        session_trade_count: clock.session_trade_count,
        unrealized_pnl_usd: None,
        open_position: open_view,
    }
}

// ─── Signal Processing ──────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn process_signals(
    signals: Vec<Signal>,
    trade: &Trade,
    open_position: &mut Option<OpenPosition>,
    completed_trades: &mut Vec<TradeRecord>,
    realized_equity: &mut f64,
    equity_curve: &mut EquityCurve,
    broker: &mut SimulatedBroker,
    tick_size: Price,
    tick_value: f64,
    contract_size: f64,
    commission_per_side: f64,
    trade_index: &mut usize,
) {
    for signal in signals {
        match signal {
            Signal::Open { side, quantity, stop_loss, take_profit, label, .. } => {
                if open_position.is_none() && quantity > 0.0 {
                    let fill_price = broker.entry_fill_price(trade.price, side);
                    *open_position = Some(OpenPosition::new(
                        side,
                        fill_price,
                        trade.time,
                        quantity,
                        Some(stop_loss),
                        take_profit,
                        label,
                    ));
                }
            }
            Signal::Close { reason } | Signal::CloseAll { reason } => {
                if let Some(pos) = open_position.take() {
                    let fill_price =
                        broker.apply_slippage(trade.price, FillDirection::Exit(pos.side));
                    let record = close_position(
                        &pos,
                        trade,
                        fill_price,
                        reason,
                        tick_size,
                        tick_value,
                        contract_size,
                        commission_per_side,
                        trade_index,
                    );
                    *realized_equity += record.pnl_net_usd;
                    equity_curve.record(trade.time, *realized_equity, 0.0);
                    completed_trades.push(record);
                }
            }
            Signal::UpdateStop { new_stop } => {
                if let Some(pos) = open_position.as_mut() {
                    pos.stop_loss = Some(new_stop);
                }
            }
            Signal::Hold => {}
        }
    }
}

// ─── Emit helper ────────────────────────────────────────────────────────────

fn emit(
    sender: &'static tokio::sync::mpsc::UnboundedSender<BacktestProgressEvent>,
    evt: BacktestProgressEvent,
) {
    let _ = sender.send(evt);
}

// ─── Signal Processing (with progress) ─────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn process_signals_with_progress(
    signals: Vec<Signal>,
    trade: &Trade,
    open_position: &mut Option<OpenPosition>,
    completed_trades: &mut Vec<TradeRecord>,
    realized_equity: &mut f64,
    equity_curve: &mut EquityCurve,
    broker: &mut SimulatedBroker,
    tick_size: Price,
    tick_value: f64,
    contract_size: f64,
    commission_per_side: f64,
    trade_index: &mut usize,
    run_id: Uuid,
    sender: &'static tokio::sync::mpsc::UnboundedSender<BacktestProgressEvent>,
) {
    for signal in signals {
        match signal {
            Signal::Open {
                side,
                quantity,
                stop_loss,
                take_profit,
                label,
                ..
            } => {
                if open_position.is_none() && quantity > 0.0 {
                    let fill_price =
                        broker.entry_fill_price(trade.price, side);
                    *open_position = Some(OpenPosition::new(
                        side,
                        fill_price,
                        trade.time,
                        quantity,
                        Some(stop_loss),
                        take_profit,
                        label,
                    ));
                }
            }
            Signal::Close { reason }
            | Signal::CloseAll { reason } => {
                if let Some(pos) = open_position.take() {
                    let fill_price = broker.apply_slippage(
                        trade.price,
                        FillDirection::Exit(pos.side),
                    );
                    let record = close_position(
                        &pos,
                        trade,
                        fill_price,
                        reason,
                        tick_size,
                        tick_value,
                        contract_size,
                        commission_per_side,
                        trade_index,
                    );
                    *realized_equity += record.pnl_net_usd;
                    equity_curve
                        .record(trade.time, *realized_equity, 0.0);
                    emit(
                        sender,
                        BacktestProgressEvent::TradeCompleted {
                            run_id,
                            trade: record.clone(),
                        },
                    );
                    completed_trades.push(record);
                }
            }
            Signal::UpdateStop { new_stop } => {
                if let Some(pos) = open_position.as_mut() {
                    pos.stop_loss = Some(new_stop);
                }
            }
            Signal::Hold => {}
        }
    }
}

// ─── Close Position ─────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn close_position(
    pos: &OpenPosition,
    trade: &Trade,
    fill_price: Price,
    reason: ExitReason,
    tick_size: Price,
    tick_value: f64,
    _contract_size: f64,
    commission_per_side: f64,
    trade_index: &mut usize,
) -> TradeRecord {
    *trade_index += 1;

    let tick_units = tick_size.units().max(1);

    let price_diff = match pos.side {
        Side::Buy => fill_price.units() - pos.entry_price.units(),
        Side::Sell => pos.entry_price.units() - fill_price.units(),
        _ => 0,
    };

    let pnl_ticks = price_diff / tick_units;
    let pnl_gross_usd = pnl_ticks as f64 * tick_value * pos.quantity;
    let commission_usd = commission_per_side * 2.0 * pos.quantity;
    let pnl_net_usd = pnl_gross_usd - commission_usd;

    let stop_dist_ticks = pos
        .stop_loss
        .map(|sl| {
            let d = match pos.side {
                Side::Buy => pos.entry_price.units() - sl.units(),
                Side::Sell => sl.units() - pos.entry_price.units(),
                _ => 0,
            };
            d / tick_units
        })
        .unwrap_or(0);

    let rr_ratio = if stop_dist_ticks != 0 {
        pnl_ticks as f64 / stop_dist_ticks as f64
    } else {
        0.0
    };

    // MAE/MFE in ticks (always non-negative)
    let mae_ticks = match pos.side {
        Side::Buy => {
            let diff = pos.entry_price.units() - pos.mae.units();
            if diff > 0 { diff / tick_units } else { 0 }
        }
        Side::Sell => {
            let diff = pos.mae.units() - pos.entry_price.units();
            if diff > 0 { diff / tick_units } else { 0 }
        }
        _ => 0,
    };

    let mfe_ticks = match pos.side {
        Side::Buy => {
            let diff = pos.mfe.units() - pos.entry_price.units();
            if diff > 0 { diff / tick_units } else { 0 }
        }
        Side::Sell => {
            let diff = pos.entry_price.units() - pos.mfe.units();
            if diff > 0 { diff / tick_units } else { 0 }
        }
        _ => 0,
    };

    TradeRecord {
        index: *trade_index,
        entry_time: pos.entry_time,
        exit_time: trade.time,
        side: pos.side,
        quantity: pos.quantity,
        entry_price: pos.entry_price,
        exit_price: fill_price,
        initial_stop_loss: pos.stop_loss.unwrap_or(pos.entry_price),
        initial_take_profit: pos.take_profit,
        pnl_ticks,
        pnl_gross_usd,
        commission_usd,
        pnl_net_usd,
        rr_ratio,
        mae_ticks,
        mfe_ticks,
        exit_reason: reason,
        label: pos.label.clone(),
    }
}
