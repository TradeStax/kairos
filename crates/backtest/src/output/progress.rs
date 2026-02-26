use crate::output::trade_record::TradeRecord;
use crate::portfolio::equity::EquityPoint;
use uuid::Uuid;

/// Events emitted during a streaming backtest run.
#[derive(Debug, Clone)]
pub enum BacktestProgressEvent {
    /// A trade was completed (position closed).
    TradeCompleted {
        run_id: Uuid,
        trade: Box<TradeRecord>,
    },
    /// A trading session was fully processed.
    SessionProcessed {
        run_id: Uuid,
        index: usize,
        total_estimated: usize,
    },
    /// Periodic equity curve sample.
    EquityUpdate { run_id: Uuid, point: EquityPoint },
    /// An order event occurred (for monitoring bracket/limit lifecycle).
    OrderEvent { run_id: Uuid, description: String },
    /// Warm-up phase completed, strategy is now live.
    WarmUpComplete {
        run_id: Uuid,
        candles_processed: usize,
    },
}
