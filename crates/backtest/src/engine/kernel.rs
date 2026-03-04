//! Core backtest engine — deterministic event-driven simulation.
//!
//! [`Engine`] is the heart of the backtest system. It processes a
//! stream of [`FeedEvent`]s in timestamp order, driving candle
//! aggregation, study computation, strategy callbacks, order
//! matching, and portfolio accounting.
//!
//! The engine is created by [`BacktestRunner`](super::runner::BacktestRunner)
//! which handles data loading and configuration validation.

use crate::config::backtest::BacktestConfig;
use crate::config::instrument::InstrumentSpec;
use crate::engine::clock::EngineClock;
use crate::engine::clock::{SessionClock, SessionEvent};
use crate::feed::aggregation::candle::PartialCandle;
use crate::feed::aggregation::multi_timeframe::MultiTimeframeAggregator;
use crate::feed::data_feed::{DataFeed, FeedEvent};
use crate::fill::FillSimulator;
use crate::order::book::OrderBook;
use crate::output::metrics::PerformanceMetrics;
use crate::output::progress::BacktestProgressEvent;
use crate::output::result::BacktestResult;
use crate::output::trade_record::{ExitReason, TradeRecord};
use crate::portfolio::equity::{DailyEquityTracker, EquityCurve, EquityPoint};
use crate::portfolio::manager::Portfolio;
use crate::strategy::context::SessionState;
use crate::strategy::study_bank::StudyBank;
use crate::strategy::{BacktestError, Strategy};
use kairos_data::{Candle, Depth, FuturesTicker, Price, Timeframe, Trade};
use std::collections::HashMap;
use uuid::Uuid;

/// Number of trade ticks between periodic equity curve samples.
const EQUITY_SAMPLE_INTERVAL: u32 = 100;

/// Deterministic event-driven backtest engine.
///
/// Processes a chronological stream of market events (trades +
/// depth) and orchestrates the full simulation pipeline:
///
/// 1. Clock advancement and session boundary detection
/// 2. Candle aggregation across multiple timeframes
/// 3. Study (indicator) recomputation on candle closes
/// 4. Strategy callbacks (`on_tick`, `on_candle`,
///    `on_session_open/close`)
/// 5. Order submission, fill simulation, and portfolio updates
/// 6. Equity curve sampling and daily tracking
///
/// The engine is deterministic: given the same config, strategy,
/// and data feed, it will always produce the same result.
pub struct Engine {
    /// Backtest configuration (ticker, timeframe, risk, etc.).
    pub(crate) config: BacktestConfig,
    /// Monotonic simulation clock.
    pub(crate) clock: EngineClock,
    /// RTH session boundary tracker.
    pub(crate) session_clock: SessionClock,
    /// Portfolio state (cash, positions, margin).
    pub(crate) portfolio: Portfolio,
    /// Active and historical orders.
    pub(crate) order_book: OrderBook,
    /// Pluggable fill simulation (market/depth-based).
    pub(crate) fill_simulator: Box<dyn FillSimulator>,
    /// Multi-timeframe candle aggregator.
    pub(crate) aggregator: MultiTimeframeAggregator,
    /// Technical indicator computation bank.
    pub(crate) study_bank: StudyBank,
    /// Instrument specifications keyed by ticker.
    pub(crate) instruments: HashMap<FuturesTicker, InstrumentSpec>,
    /// Most recent depth snapshot per instrument.
    pub(crate) latest_depth: HashMap<FuturesTicker, Depth>,
    /// Most recent trade price per instrument.
    pub(crate) latest_prices: HashMap<FuturesTicker, Price>,

    // ── Context cache ───────────────────────────────────────────
    // Rebuilt before each `build_context` call when the aggregator
    // generation changes.
    /// Completed candles per (instrument, timeframe) pair.
    pub(crate) cached_candles: HashMap<(FuturesTicker, Timeframe), Vec<Candle>>,
    /// In-progress partial candles per (instrument, timeframe).
    pub(crate) cached_partials: HashMap<(FuturesTicker, Timeframe), PartialCandle>,
    /// Aggregator generation at last cache rebuild.
    pub(crate) cache_generation: u64,

    // ── Accumulated results ─────────────────────────────────────
    /// Completed round-trip trade records.
    pub(crate) completed_trades: Vec<TradeRecord>,
    /// Time-series equity curve.
    pub(crate) equity_curve: EquityCurve,
    /// Daily equity snapshots for analytics.
    pub(crate) daily_tracker: DailyEquityTracker,
    /// Number of RTH sessions processed so far.
    pub(crate) sessions_processed: usize,
    /// Trade tick counter for periodic equity sampling.
    pub(crate) tick_count: u32,
    /// Number of candles required before exiting warm-up.
    pub(crate) warm_up_candles: usize,
    /// Whether the engine is still in the warm-up phase.
    pub(crate) is_warmup: bool,
    /// Warnings accumulated during the run.
    pub(crate) warnings: Vec<String>,

    // ── Benchmark ───────────────────────────────────────────────
    /// Price of the first trade on the primary instrument, for
    /// buy-and-hold benchmark calculation.
    pub(crate) first_trade_price: Option<Price>,
}

impl Engine {
    /// Creates a new engine with the given configuration, instrument
    /// specs, and fill simulator.
    #[must_use]
    pub fn new(
        config: BacktestConfig,
        instruments: HashMap<FuturesTicker, InstrumentSpec>,
        fill_simulator: Box<dyn FillSimulator>,
    ) -> Self {
        let initial = config.initial_capital_usd;
        let commission = config.commission_per_side_usd;
        let margin_calc = if config.margin.enforce {
            Some(crate::portfolio::margin::MarginCalculator::new(
                config.margin.initial_margin_override,
                config.margin.maintenance_margin_override,
            ))
        } else {
            None
        };

        let portfolio = Portfolio::new(initial, instruments.clone(), commission, margin_calc);

        Self {
            clock: EngineClock::new(),
            session_clock: SessionClock::new(
                config.timezone_offset_hours,
                config.rth_open_hhmm,
                config.rth_close_hhmm,
            ),
            portfolio,
            order_book: OrderBook::new(),
            fill_simulator,
            aggregator: MultiTimeframeAggregator::new(),
            study_bank: StudyBank::new(),
            instruments,
            latest_depth: HashMap::new(),
            latest_prices: HashMap::new(),
            cached_candles: HashMap::new(),
            cached_partials: HashMap::new(),
            cache_generation: 0,
            completed_trades: Vec::new(),
            equity_curve: EquityCurve::new(initial),
            daily_tracker: DailyEquityTracker::new(),
            sessions_processed: 0,
            tick_count: 0,
            warm_up_candles: config.warm_up_periods,
            is_warmup: config.warm_up_periods > 0,
            warnings: Vec::new(),
            first_trade_price: None,
            config,
        }
    }

    /// Runs the full backtest with the given strategy and data feed.
    ///
    /// This is the main entry point. It:
    /// 1. Registers timeframes and initializes studies
    /// 2. Calls `strategy.on_init()`
    /// 3. Processes every event in the feed
    /// 4. Force-closes remaining positions
    /// 5. Computes performance metrics and returns a
    ///    [`BacktestResult`]
    pub fn run(
        &mut self,
        mut strategy: Box<dyn Strategy>,
        mut feed: DataFeed,
        run_id: Uuid,
        sender: Option<&'static tokio::sync::mpsc::UnboundedSender<BacktestProgressEvent>>,
    ) -> Result<BacktestResult, BacktestError> {
        let run_started_at_ms = crate::engine::system_time_ms();
        let strategy_name = strategy.metadata().name.clone();
        let total_data_trades = feed.total_events();
        let primary = self.config.ticker;

        self.register_timeframes(primary, &*strategy);
        self.initialize_studies(&*strategy);

        // Build initial context and call on_init
        self.rebuild_context_cache(primary);
        {
            let dummy = Trade::new(
                kairos_data::Timestamp(0),
                self.latest_prices
                    .get(&primary)
                    .copied()
                    .unwrap_or(Price::zero()),
                kairos_data::Quantity(0.0),
                kairos_data::Side::Buy,
            );
            let ctx = self.build_context(primary, &dummy);
            strategy.on_init(&ctx);
        }

        let total_estimated_sessions = self.config.date_range.num_days().max(1) as usize;

        // Main event loop — process every event in timestamp order
        while let Some(feed_event) = feed.next_event() {
            let timestamp = feed_event.timestamp();
            self.clock.advance(timestamp);

            match feed_event {
                FeedEvent::Trade { instrument, trade } => {
                    self.process_trade(
                        instrument,
                        trade,
                        &mut *strategy,
                        run_id,
                        sender,
                        total_estimated_sessions,
                    );
                }
                FeedEvent::Depth { instrument, depth } => {
                    self.latest_depth.insert(instrument, depth);
                }
            }
        }

        // Force-close any remaining positions at end of data
        self.close_all_positions(ExitReason::EndOfData);

        self.build_result(
            run_id,
            strategy_name,
            run_started_at_ms,
            total_data_trades,
            primary,
        )
    }

    /// Registers all required timeframes on the aggregator.
    fn register_timeframes(&mut self, primary: FuturesTicker, strategy: &dyn Strategy) {
        self.aggregator.register(primary, self.config.timeframe);
        for tf in strategy.required_timeframes() {
            self.aggregator.register(primary, tf);
        }
        for inst in &self.config.additional_instruments {
            self.aggregator.register(*inst, self.config.timeframe);
        }
        for tf in &self.config.additional_timeframes {
            self.aggregator.register(primary, *tf);
        }
    }

    /// Initializes the study bank from strategy requirements.
    ///
    /// Also checks whether the warm-up period is long enough for
    /// the requested study periods and emits a warning if not.
    fn initialize_studies(&mut self, strategy: &dyn Strategy) {
        let requests = strategy.required_studies();
        if requests.is_empty() {
            return;
        }

        let registry = kairos_study::StudyRegistry::default();
        self.study_bank.initialize(&requests, &registry);

        // Warn if warm-up may be too short for study convergence
        let max_study_period = requests
            .iter()
            .flat_map(|r| r.params.iter())
            .filter_map(|(key, val)| {
                if key == "period" || key == "slow_period" || key == "k_period" {
                    if let kairos_study::ParameterValue::Integer(n) = val {
                        Some(*n as usize)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .max()
            .unwrap_or(0);

        if max_study_period > 0 && self.warm_up_candles < max_study_period + 1 {
            self.warnings.push(format!(
                "warm_up_periods ({}) may be too short for study \
                 requirements (max period={}). Consider >= {}.",
                self.warm_up_candles,
                max_study_period,
                max_study_period + 1,
            ));
        }
    }

    /// Assembles the final [`BacktestResult`] from accumulated
    /// engine state.
    fn build_result(
        &self,
        run_id: Uuid,
        strategy_name: String,
        run_started_at_ms: u64,
        total_data_trades: usize,
        primary: FuturesTicker,
    ) -> Result<BacktestResult, BacktestError> {
        let trading_days = self.sessions_processed;
        let mut metrics = PerformanceMetrics::compute(
            &self.completed_trades,
            self.config.initial_capital_usd,
            trading_days,
            self.config.risk.risk_free_annual,
            &self.equity_curve,
        );

        let run_duration_ms = crate::engine::system_time_ms().saturating_sub(run_started_at_ms);

        // Buy-and-hold benchmark comparison
        let benchmark_pnl_usd = self.compute_benchmark_pnl(primary);

        if let Some(bench_pnl) = benchmark_pnl_usd
            && self.config.initial_capital_usd > 0.0
        {
            metrics.benchmark_return_pct = bench_pnl / self.config.initial_capital_usd * 100.0;
            metrics.alpha_pct = metrics.total_return_pct - metrics.benchmark_return_pct;
        }

        Ok(BacktestResult {
            id: run_id,
            config: self.config.clone(),
            strategy_name,
            run_started_at_ms,
            run_duration_ms,
            total_data_trades,
            trades: self.completed_trades.clone(),
            metrics,
            equity_curve: self.equity_curve.clone(),
            sessions_processed: self.sessions_processed,
            sessions_skipped: 0,
            warnings: self.warnings.clone(),
            daily_snapshots: self.daily_tracker.snapshots().to_vec(),
            benchmark_pnl_usd,
        })
    }

    /// Computes buy-and-hold P&L for the primary instrument.
    fn compute_benchmark_pnl(&self, primary: FuturesTicker) -> Option<f64> {
        let first = self.first_trade_price?;
        let last = self.latest_prices.get(&primary)?;
        let multiplier = self
            .instruments
            .get(&primary)
            .map(|i| i.multiplier)
            .unwrap_or(50.0);
        let diff = last.to_f64() - first.to_f64();
        Some(diff * multiplier)
    }

    /// Processes a single trade event through the full simulation
    /// pipeline.
    ///
    /// Pipeline stages:
    /// 1. Update latest price and record first-trade benchmark
    /// 2. Session boundary detection (open/close callbacks)
    /// 3. Candle aggregation and warm-up tracking
    /// 4. Study recomputation on primary candle close
    /// 5. Strategy `on_candle` callbacks for each closed candle
    /// 6. Fill checks on active orders
    /// 7. Position mark-to-market and drawdown circuit breaker
    /// 8. Strategy `on_tick` callback (RTH only, post-warmup)
    /// 9. Periodic equity sampling and daily tracking
    #[allow(clippy::too_many_arguments)]
    fn process_trade(
        &mut self,
        instrument: FuturesTicker,
        trade: Trade,
        strategy: &mut dyn Strategy,
        run_id: Uuid,
        sender: Option<&'static tokio::sync::mpsc::UnboundedSender<BacktestProgressEvent>>,
        total_estimated_sessions: usize,
    ) {
        let primary = self.config.ticker;
        self.latest_prices.insert(instrument, trade.price);

        if self.first_trade_price.is_none() && instrument == primary {
            self.first_trade_price = Some(trade.price);
        }

        // 1. Session boundary detection
        self.handle_session_events(&trade, strategy, run_id, sender, total_estimated_sessions);

        // 2. Candle aggregation + study recomputation + callbacks
        self.handle_candle_aggregation(instrument, &trade, strategy, run_id, sender);

        // 3. Fill checks on active orders
        self.check_fills(&trade, run_id, sender, strategy);

        // 4. Mark-to-market and drawdown circuit breaker
        self.portfolio.mark_to_market(&self.latest_prices);
        if self.check_drawdown_breaker(&trade) {
            return;
        }

        // 5. on_tick (RTH only, post-warmup) + equity sampling
        self.handle_on_tick(&trade, strategy, run_id, sender);

        // 6. Daily equity tracking
        self.daily_tracker.maybe_record(
            trade.time.0,
            self.portfolio.total_equity(),
            self.portfolio.realized_pnl(),
            self.portfolio.positions().len(),
        );
    }

    /// Detects and handles session open/close boundaries.
    fn handle_session_events(
        &mut self,
        trade: &Trade,
        strategy: &mut dyn Strategy,
        run_id: Uuid,
        sender: Option<&'static tokio::sync::mpsc::UnboundedSender<BacktestProgressEvent>>,
        total_estimated_sessions: usize,
    ) {
        let primary = self.config.ticker;
        let Some(event) = self.session_clock.advance(trade.time) else {
            return;
        };

        match event {
            SessionEvent::Open { .. } => {
                self.sessions_processed += 1;
                if let Some(s) = sender {
                    let _ = s.send(BacktestProgressEvent::SessionProcessed {
                        run_id,
                        index: self.sessions_processed,
                        total_estimated: total_estimated_sessions,
                    });
                }
                if !self.is_warmup {
                    self.rebuild_context_cache(primary);
                    let requests = {
                        let ctx = self.build_context(primary, trade);
                        strategy.on_session_open(&ctx)
                    };
                    self.process_order_requests(requests, trade, run_id, sender, &*strategy);
                }
            }
            SessionEvent::Close { .. } => {
                if !self.is_warmup {
                    self.rebuild_context_cache(primary);
                    let requests = {
                        let ctx = self.build_context(primary, trade);
                        strategy.on_session_close(&ctx)
                    };
                    self.process_order_requests(requests, trade, run_id, sender, &*strategy);
                }
                self.order_book.expire_day_orders(trade.time);
            }
        }
    }

    /// Aggregates the trade into candles, checks warm-up, recomputes
    /// studies, and invokes `on_candle` for each closed candle.
    fn handle_candle_aggregation(
        &mut self,
        instrument: FuturesTicker,
        trade: &Trade,
        strategy: &mut dyn Strategy,
        run_id: Uuid,
        sender: Option<&'static tokio::sync::mpsc::UnboundedSender<BacktestProgressEvent>>,
    ) {
        let primary = self.config.ticker;
        let closed_candles = self.aggregator.update(instrument, trade);

        for (key, candle) in &closed_candles {
            // Check warm-up completion on primary candle close
            if self.is_warmup && key.instrument == primary && key.timeframe == self.config.timeframe
            {
                self.check_warmup_complete(trade, strategy, run_id, sender);
            }

            // Recompute studies on primary candle close
            if key.instrument == primary && key.timeframe == self.config.timeframe {
                let candles = self.aggregator.candles(primary, self.config.timeframe);
                let tick_size = self
                    .instruments
                    .get(&primary)
                    .map(|i| i.tick_size)
                    .unwrap_or(Price::from_f64(0.25));
                self.study_bank
                    .recompute(candles, None, tick_size, self.config.timeframe);
            }

            if !self.is_warmup {
                self.rebuild_context_cache(primary);
                let requests = {
                    let ctx = self.build_context(primary, trade);
                    strategy.on_candle(key.instrument, key.timeframe, candle, &ctx)
                };
                self.process_order_requests(requests, trade, run_id, sender, &*strategy);
            }
        }
    }

    /// Checks if enough candles have accumulated to exit warm-up.
    fn check_warmup_complete(
        &mut self,
        trade: &Trade,
        strategy: &mut dyn Strategy,
        run_id: Uuid,
        sender: Option<&'static tokio::sync::mpsc::UnboundedSender<BacktestProgressEvent>>,
    ) {
        let primary = self.config.ticker;
        let count = self
            .aggregator
            .candles(primary, self.config.timeframe)
            .len();

        if count < self.warm_up_candles {
            return;
        }

        self.is_warmup = false;
        self.rebuild_context_cache(primary);
        {
            let ctx = self.build_context(primary, trade);
            strategy.on_warmup_complete(&ctx);
        }
        if let Some(s) = sender {
            let _ = s.send(BacktestProgressEvent::WarmUpComplete {
                run_id,
                candles_processed: count,
            });
        }
    }

    /// Checks the max-drawdown circuit breaker.
    ///
    /// Returns `true` if the breaker was tripped and the trade
    /// processing pipeline should short-circuit.
    fn check_drawdown_breaker(&mut self, trade: &Trade) -> bool {
        let Some(max_dd) = self.config.risk.max_drawdown_pct else {
            return false;
        };

        let current_dd = self.portfolio.current_drawdown_pct();
        if current_dd < max_dd * 100.0 {
            return false;
        }

        self.close_all_positions(ExitReason::MaxDrawdown);
        self.order_book.cancel_all(None, trade.time);
        self.warnings.push(format!(
            "Max drawdown limit reached ({current_dd:.1}% \
             >= {:.1}%). All positions closed.",
            max_dd * 100.0
        ));
        true
    }

    /// Invokes the strategy's `on_tick` callback during RTH and
    /// periodically samples the equity curve.
    fn handle_on_tick(
        &mut self,
        trade: &Trade,
        strategy: &mut dyn Strategy,
        run_id: Uuid,
        sender: Option<&'static tokio::sync::mpsc::UnboundedSender<BacktestProgressEvent>>,
    ) {
        if self.session_clock.session_state != SessionState::Open || self.is_warmup {
            return;
        }

        let primary = self.config.ticker;
        self.rebuild_context_cache(primary);
        let requests = {
            let ctx = self.build_context(primary, trade);
            strategy.on_tick(&ctx)
        };
        self.process_order_requests(requests, trade, run_id, sender, &*strategy);

        // Periodic equity sampling
        self.tick_count += 1;
        if self.tick_count >= EQUITY_SAMPLE_INTERVAL {
            self.tick_count = 0;
            self.sample_equity(trade, run_id, sender);
        }
    }

    /// Records an equity curve point and emits an equity update
    /// progress event.
    fn sample_equity(
        &mut self,
        trade: &Trade,
        run_id: Uuid,
        sender: Option<&'static tokio::sync::mpsc::UnboundedSender<BacktestProgressEvent>>,
    ) {
        let realized = self.portfolio.cash();
        let unrealized: f64 = self
            .portfolio
            .positions()
            .values()
            .map(|pos| {
                self.instruments
                    .get(&pos.instrument)
                    .map(|i| pos.unrealized_pnl(i.tick_size, i.tick_value))
                    .unwrap_or(0.0)
            })
            .sum();

        self.equity_curve.record(trade.time, realized, unrealized);

        if let Some(s) = sender {
            let _ = s.send(BacktestProgressEvent::EquityUpdate {
                run_id,
                point: EquityPoint {
                    timestamp: trade.time,
                    realized_equity_usd: realized,
                    unrealized_pnl_usd: unrealized,
                    total_equity_usd: realized + unrealized,
                },
            });
        }
    }
}

/// Creates a default [`InstrumentSpec`] for the given ticker.
///
/// Convenience function that delegates to
/// [`InstrumentSpec::from_ticker`].
#[must_use]
pub fn default_instrument(ticker: FuturesTicker) -> InstrumentSpec {
    InstrumentSpec::from_ticker(ticker)
}
