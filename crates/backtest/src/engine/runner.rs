use std::collections::HashMap;
use std::sync::Arc;

use crate::feed::provider::TradeProvider;
use uuid::Uuid;

use crate::config::backtest::BacktestConfig;
use crate::config::instrument::InstrumentSpec;
use crate::engine::kernel::Engine;
use crate::feed::data_feed::DataFeed;
use crate::fill::FillSimulator;
use crate::fill::depth::DepthBasedFillSimulator;
use crate::fill::market::StandardFillSimulator;
use crate::output::metrics::PerformanceMetrics;
use crate::output::progress::BacktestProgressEvent;
use crate::output::result::BacktestResult;
use crate::portfolio::equity::EquityCurve;
use crate::strategy::{BacktestError, Strategy};

/// Backward-compatible facade over the event-driven Engine.
///
/// Loads historical data from a TradeProvider, builds a DataFeed
/// and InstrumentSpec map, then delegates to [`Engine::run`].
pub struct BacktestRunner {
    trade_provider: Arc<dyn TradeProvider>,
}

impl BacktestRunner {
    pub fn new(trade_provider: Arc<dyn TradeProvider>) -> Self {
        Self { trade_provider }
    }

    /// Execute the backtest and return a complete `BacktestResult`.
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

    /// Execute the backtest, emitting progress events to a shared
    /// buffer. The returned `BacktestResult.id` matches the
    /// supplied `run_id`.
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

fn build_fill_simulator(config: &BacktestConfig) -> Box<dyn FillSimulator> {
    use crate::config::risk::SlippageModel;
    match &config.slippage {
        SlippageModel::DepthBased => {
            Box::new(DepthBasedFillSimulator::new(config.slippage.clone()))
        }
        _ => Box::new(StandardFillSimulator::new(config.slippage.clone())),
    }
}

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
