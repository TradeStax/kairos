//! Streaming progress events emitted during a backtest run.
//!
//! The backtest engine emits [`BacktestProgressEvent`]s through a
//! channel as the run progresses. Consumers (typically the UI layer)
//! can use these events to display live trade notifications, equity
//! updates, and session progress without waiting for the run to
//! complete.

use crate::output::trade_record::TradeRecord;
use crate::portfolio::equity::EquityPoint;
use uuid::Uuid;

/// Events emitted during a streaming backtest run.
///
/// Each variant carries the `run_id` of the backtest that produced
/// it, allowing consumers to correlate events when multiple runs
/// are in flight.
#[derive(Debug, Clone)]
pub enum BacktestProgressEvent {
    /// A round-trip trade was completed (position closed).
    ///
    /// The trade record is boxed to keep the enum size uniform
    /// across variants.
    TradeCompleted {
        /// Backtest run that produced this trade.
        run_id: Uuid,
        /// The completed trade record.
        trade: Box<TradeRecord>,
    },
    /// A trading session was fully processed.
    SessionProcessed {
        /// Backtest run this session belongs to.
        run_id: Uuid,
        /// 0-indexed session number that just completed.
        index: usize,
        /// Estimated total number of sessions in the run.
        total_estimated: usize,
    },
    /// Periodic equity curve sample.
    EquityUpdate {
        /// Backtest run this sample belongs to.
        run_id: Uuid,
        /// The sampled equity point.
        point: EquityPoint,
    },
    /// An order lifecycle event occurred (e.g. bracket placed,
    /// limit filled).
    OrderEvent {
        /// Backtest run that produced this event.
        run_id: Uuid,
        /// Human-readable description of the order event.
        description: String,
    },
    /// Warm-up phase completed; the strategy is now generating
    /// live signals.
    WarmUpComplete {
        /// Backtest run that completed warm-up.
        run_id: Uuid,
        /// Number of candles processed during warm-up.
        candles_processed: usize,
    },
}
