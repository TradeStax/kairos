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
use crate::portfolio::equity::{DailyEquityTracker, EquityCurve};
use crate::portfolio::manager::Portfolio;
use crate::strategy::context::SessionState;
use crate::strategy::study_bank::StudyBank;
use crate::strategy::{BacktestError, Strategy};
use kairos_data::{Candle, Depth, FuturesTicker, Price, Timeframe, Trade};
use std::collections::HashMap;
use uuid::Uuid;

/// Deterministic event-driven backtest engine.
pub struct Engine {
    pub(crate) config: BacktestConfig,
    pub(crate) clock: EngineClock,
    pub(crate) session_clock: SessionClock,
    pub(crate) portfolio: Portfolio,
    pub(crate) order_book: OrderBook,
    pub(crate) fill_simulator: Box<dyn FillSimulator>,
    pub(crate) aggregator: MultiTimeframeAggregator,
    pub(crate) study_bank: StudyBank,
    pub(crate) instruments: HashMap<FuturesTicker, InstrumentSpec>,
    pub(crate) latest_depth: HashMap<FuturesTicker, Depth>,
    pub(crate) latest_prices: HashMap<FuturesTicker, Price>,

    // Context cache — rebuilt before each build_context call
    pub(crate) cached_candles: HashMap<(FuturesTicker, Timeframe), Vec<Candle>>,
    pub(crate) cached_partials: HashMap<(FuturesTicker, Timeframe), PartialCandle>,
    pub(crate) cache_generation: u64,

    // Accumulated results
    pub(crate) completed_trades: Vec<TradeRecord>,
    pub(crate) equity_curve: EquityCurve,
    pub(crate) daily_tracker: DailyEquityTracker,
    pub(crate) sessions_processed: usize,
    pub(crate) tick_count: u32,
    pub(crate) warm_up_candles: usize,
    pub(crate) is_warmup: bool,
    pub(crate) warnings: Vec<String>,

    // Benchmark
    pub(crate) first_trade_price: Option<Price>,
}

impl Engine {
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

    /// Run the full backtest with the given strategy and data feed.
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

        // Register timeframes
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

        // Initialize studies
        let requests = strategy.required_studies();
        if !requests.is_empty() {
            let registry = kairos_study::StudyRegistry::default();
            self.study_bank.initialize(&requests, &registry);

            // Warn if warm-up period may be too short
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

        // Main event loop
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

        // Force-close remaining positions
        self.close_all_positions(ExitReason::EndOfData);

        let trading_days = self.sessions_processed;
        let mut metrics = PerformanceMetrics::compute(
            &self.completed_trades,
            self.config.initial_capital_usd,
            trading_days,
            self.config.risk.risk_free_annual,
            &self.equity_curve,
        );

        let run_duration_ms = crate::engine::system_time_ms().saturating_sub(run_started_at_ms);

        let benchmark_pnl_usd = self.first_trade_price.and_then(|first| {
            let last = self.latest_prices.get(&primary)?;
            let multiplier = self
                .instruments
                .get(&primary)
                .map(|i| i.multiplier)
                .unwrap_or(50.0);
            let diff = last.to_f64() - first.to_f64();
            Some(diff * multiplier)
        });

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

        // 1. Session clock
        let session_event = self.session_clock.advance(trade.time);

        if let Some(event) = session_event {
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
                            let ctx = self.build_context(primary, &trade);
                            strategy.on_session_open(&ctx)
                        };
                        self.process_order_requests(requests, &trade, run_id, sender);
                    }
                }
                SessionEvent::Close { .. } => {
                    if !self.is_warmup {
                        self.rebuild_context_cache(primary);
                        let requests = {
                            let ctx = self.build_context(primary, &trade);
                            strategy.on_session_close(&ctx)
                        };
                        self.process_order_requests(requests, &trade, run_id, sender);
                    }
                    self.order_book.expire_day_orders(trade.time);
                }
            }
        }

        // 2. Candle aggregation
        let closed_candles = self.aggregator.update(instrument, &trade);
        for (key, candle) in &closed_candles {
            // Check warm-up completion
            if self.is_warmup && key.instrument == primary && key.timeframe == self.config.timeframe
            {
                let count = self
                    .aggregator
                    .candles(primary, self.config.timeframe)
                    .len();
                if count >= self.warm_up_candles {
                    self.is_warmup = false;
                    self.rebuild_context_cache(primary);
                    {
                        let ctx = self.build_context(primary, &trade);
                        strategy.on_warmup_complete(&ctx);
                    }
                    if let Some(s) = sender {
                        let _ = s.send(BacktestProgressEvent::WarmUpComplete {
                            run_id,
                            candles_processed: count,
                        });
                    }
                }
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
                    let ctx = self.build_context(primary, &trade);
                    strategy.on_candle(key.instrument, key.timeframe, candle, &ctx)
                };
                self.process_order_requests(requests, &trade, run_id, sender);
            }
        }

        // 3. Fill checks on active orders
        self.check_fills(&trade, run_id, sender, strategy);

        // 4. Update position marks
        self.portfolio.mark_to_market(&self.latest_prices);

        // 4b. Max drawdown circuit breaker
        if let Some(max_dd) = self.config.risk.max_drawdown_pct {
            let current_dd = self.portfolio.current_drawdown_pct();
            if current_dd >= max_dd * 100.0 {
                self.close_all_positions(ExitReason::MaxDrawdown);
                self.order_book.cancel_all(None, trade.time);
                self.warnings.push(format!(
                    "Max drawdown limit reached ({current_dd:.1}% \
                     >= {:.1}%). All positions closed.",
                    max_dd * 100.0
                ));
                return;
            }
        }

        // 5. on_tick (RTH only, not during warmup)
        if self.session_clock.session_state == SessionState::Open && !self.is_warmup {
            self.rebuild_context_cache(primary);
            let requests = {
                let ctx = self.build_context(primary, &trade);
                strategy.on_tick(&ctx)
            };
            self.process_order_requests(requests, &trade, run_id, sender);

            // Periodic equity sampling
            self.tick_count += 1;
            if self.tick_count >= 100 {
                self.tick_count = 0;
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
                        point: crate::portfolio::equity::EquityPoint {
                            timestamp: trade.time,
                            realized_equity_usd: realized,
                            unrealized_pnl_usd: unrealized,
                            total_equity_usd: realized + unrealized,
                        },
                    });
                }
            }
        }

        // 6. Daily equity tracking
        self.daily_tracker.maybe_record(
            trade.time.0,
            self.portfolio.total_equity(),
            self.portfolio.realized_pnl(),
            self.portfolio.positions().len(),
        );
    }
}

/// Convenience: create default InstrumentSpec for a ticker.
pub fn default_instrument(ticker: FuturesTicker) -> InstrumentSpec {
    InstrumentSpec::from_ticker(ticker)
}
