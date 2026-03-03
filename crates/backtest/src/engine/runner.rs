//! High-level backtest runner facade.
//!
//! [`BacktestRunner`] wraps data loading and engine initialization
//! into a simple `run()` API. It validates configuration, loads
//! historical trades from a [`TradeProvider`], builds the data feed
//! and instrument map, and delegates to [`Engine::run`].

use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use crate::config::backtest::BacktestConfig;
use crate::config::instrument::InstrumentSpec;
use crate::engine::kernel::Engine;
use crate::feed::data_feed::DataFeed;
use crate::feed::provider::TradeProvider;
use crate::fill::FillSimulator;
use crate::fill::depth::DepthBasedFillSimulator;
use crate::fill::market::StandardFillSimulator;
use crate::output::metrics::PerformanceMetrics;
use crate::output::progress::BacktestProgressEvent;
use crate::output::result::BacktestResult;
use crate::portfolio::equity::EquityCurve;
use crate::strategy::{BacktestError, Strategy};

/// High-level facade over the event-driven [`Engine`].
///
/// Loads historical data from a [`TradeProvider`], builds a
/// [`DataFeed`] and instrument spec map, then delegates to
/// [`Engine::run`]. This is the primary entry point for running
/// backtests from application code.
pub struct BacktestRunner {
    /// Data source for historical trades.
    trade_provider: Arc<dyn TradeProvider>,
}

impl BacktestRunner {
    /// Creates a new runner with the given trade data provider.
    #[must_use]
    pub fn new(trade_provider: Arc<dyn TradeProvider>) -> Self {
        Self { trade_provider }
    }

    /// Executes the backtest and returns a complete
    /// [`BacktestResult`].
    ///
    /// Validates the config, applies strategy parameters, loads
    /// trade data, and runs the engine. Returns an empty result
    /// (with a warning) if no data is found for the configured
    /// date range.
    pub async fn run(
        &self,
        config: BacktestConfig,
        mut strategy: Box<dyn Strategy>,
    ) -> Result<BacktestResult, BacktestError> {
        config.validate().map_err(BacktestError::Validation)?;

        for (key, value) in &config.strategy_params {
            strategy.set_parameter(key, value.clone())?;
        }

        let raw_trades = self
            .trade_provider
            .get_trades(&config.ticker, &config.date_range)
            .await
            .map_err(BacktestError::Data)?;

        if raw_trades.is_empty() {
            return Ok(empty_result(
                config,
                strategy.metadata().name,
                Uuid::new_v4(),
            ));
        }

        log_unimplemented_warnings(&config);
        let instruments = build_instruments(&config);
        let mut feed = DataFeed::new();
        feed.add_trades(config.ticker, raw_trades);
        let fill_sim = build_fill_simulator(&config);

        let run_id = Uuid::new_v4();
        let mut engine = Engine::new(config, instruments, fill_sim);
        engine.run(strategy, feed, run_id, None)
    }

    /// Executes the backtest, emitting progress events to a channel.
    ///
    /// Identical to [`run`](Self::run) but sends
    /// [`BacktestProgressEvent`]s to the provided sender for
    /// real-time UI updates. The returned result's `id` matches
    /// the supplied `run_id`.
    pub async fn run_with_progress(
        &self,
        config: BacktestConfig,
        mut strategy: Box<dyn Strategy>,
        run_id: Uuid,
        sender: &'static tokio::sync::mpsc::UnboundedSender<BacktestProgressEvent>,
    ) -> Result<BacktestResult, BacktestError> {
        config.validate().map_err(BacktestError::Validation)?;

        for (key, value) in &config.strategy_params {
            strategy.set_parameter(key, value.clone())?;
        }

        let raw_trades = self
            .trade_provider
            .get_trades(&config.ticker, &config.date_range)
            .await
            .map_err(BacktestError::Data)?;

        if raw_trades.is_empty() {
            return Ok(empty_result(config, strategy.metadata().name, run_id));
        }

        log_unimplemented_warnings(&config);
        let instruments = build_instruments(&config);
        let mut feed = DataFeed::new();
        feed.add_trades(config.ticker, raw_trades);
        let fill_sim = build_fill_simulator(&config);

        let mut engine = Engine::new(config, instruments, fill_sim);
        engine.run(strategy, feed, run_id, Some(sender))
    }
}

// ─── Helpers ────────────────────────────────────────────────────

/// Selects the appropriate fill simulator based on the slippage
/// model configured in the backtest.
fn build_fill_simulator(config: &BacktestConfig) -> Box<dyn FillSimulator> {
    use crate::config::risk::SlippageModel;
    match &config.slippage {
        SlippageModel::DepthBased => {
            Box::new(DepthBasedFillSimulator::new(config.slippage.clone()))
        }
        SlippageModel::None
        | SlippageModel::FixedTick(_)
        | SlippageModel::Percentage(_)
        | SlippageModel::VolumeImpact { .. } => {
            Box::new(StandardFillSimulator::new(config.slippage.clone()))
        }
    }
}

/// Logs warnings for configured features that are not yet
/// implemented (latency simulation, dynamic position sizing).
fn log_unimplemented_warnings(config: &BacktestConfig) {
    use crate::config::risk::PositionSizeMode;
    if config.simulated_latency_ms > 0 {
        log::warn!(
            "simulated_latency_ms={} is configured but not yet \
             implemented; fills will be instant",
            config.simulated_latency_ms
        );
    }
    if !matches!(config.risk.position_size_mode, PositionSizeMode::Fixed(_)) {
        log::warn!(
            "position_size_mode={:?} is configured but not yet \
             implemented; strategies must size orders manually",
            config.risk.position_size_mode
        );
    }
}

/// Builds the instrument spec map from the backtest config.
fn build_instruments(
    config: &BacktestConfig,
) -> HashMap<kairos_data::FuturesTicker, InstrumentSpec> {
    let mut instruments = HashMap::new();
    instruments.insert(config.ticker, InstrumentSpec::from_ticker(config.ticker));
    for inst in &config.additional_instruments {
        instruments.insert(*inst, InstrumentSpec::from_ticker(*inst));
    }
    instruments
}

/// Creates an empty backtest result when no data is available.
fn empty_result(config: BacktestConfig, strategy_name: String, run_id: Uuid) -> BacktestResult {
    let initial_capital = config.initial_capital_usd;
    let equity_curve = EquityCurve::new(initial_capital);
    let metrics = PerformanceMetrics::compute(
        &[],
        initial_capital,
        0,
        config.risk.risk_free_annual,
        &equity_curve,
    );
    let run_started_at_ms = crate::engine::system_time_ms();
    BacktestResult {
        id: run_id,
        config,
        strategy_name,
        run_started_at_ms,
        run_duration_ms: 0,
        total_data_trades: 0,
        trades: vec![],
        metrics,
        equity_curve,
        sessions_processed: 0,
        sessions_skipped: 0,
        warnings: vec!["No data found for the configured date range.".to_string()],
        daily_snapshots: vec![],
        benchmark_pnl_usd: None,
    }
}
