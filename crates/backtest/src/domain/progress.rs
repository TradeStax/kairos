use crate::domain::trade_record::TradeRecord;
use crate::portfolio::equity::EquityPoint;
use uuid::Uuid;

/// Events emitted during a streaming backtest run.
#[derive(Debug, Clone)]
pub enum BacktestProgressEvent {
    /// A trade was completed (position closed).
    TradeCompleted { run_id: Uuid, trade: TradeRecord },
    /// A trading session was fully processed.
    SessionProcessed {
        run_id: Uuid,
        index: usize,
        total_estimated: usize,
    },
    /// Periodic equity curve sample.
    EquityUpdate { run_id: Uuid, point: EquityPoint },
}
