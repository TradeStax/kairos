//! Individual trade record types.
//!
//! A [`TradeRecord`] captures every detail of a single completed
//! round-trip trade: entry/exit prices and times, P&L in both USD
//! and ticks, risk parameters, and trade excursion data (MAE/MFE).
//! The [`ExitReason`] enum describes why the position was closed.

use super::snapshot::TradeSnapshot;
use kairos_data::{FuturesTicker, Price, Side, Timestamp};
use serde::{Deserialize, Serialize};

/// Reason a position was closed.
///
/// Each variant maps to a specific exit mechanism in the backtest
/// engine's order management logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExitReason {
    /// Fixed stop-loss triggered.
    StopLoss,
    /// Fixed take-profit triggered.
    TakeProfit,
    /// Trailing stop triggered.
    TrailingStop,
    /// Position flattened at the end of a trading session.
    SessionClose,
    /// Strategy-defined time-based exit (e.g. ORB time exit).
    TimeExit,
    /// Strategy issued an explicit close signal.
    Manual,
    /// Portfolio-level maximum drawdown limit breached.
    MaxDrawdown,
    /// Backtest data exhausted with position still open.
    EndOfData,
    /// Bracket order stop-loss leg filled.
    BracketSL,
    /// Bracket order take-profit leg filled.
    BracketTP,
    /// Position flattened by a flatten-all directive.
    Flatten,
}

impl std::fmt::Display for ExitReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StopLoss => write!(f, "Stop Loss"),
            Self::TakeProfit => write!(f, "Take Profit"),
            Self::TrailingStop => write!(f, "Trailing Stop"),
            Self::SessionClose => write!(f, "Session Close"),
            Self::TimeExit => write!(f, "Time Exit"),
            Self::Manual => write!(f, "Manual"),
            Self::MaxDrawdown => write!(f, "Max Drawdown"),
            Self::EndOfData => write!(f, "End of Data"),
            Self::BracketSL => write!(f, "Bracket SL"),
            Self::BracketTP => write!(f, "Bracket TP"),
            Self::Flatten => write!(f, "Flatten"),
        }
    }
}

/// A single completed round-trip trade record.
///
/// Records every detail needed for post-run analytics: timing,
/// prices, P&L, risk parameters, and trade excursion. Trade
/// records are stored in chronological order within
/// [`BacktestResult::trades`](super::result::BacktestResult::trades).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    /// 1-indexed trade sequence number within the run.
    pub index: usize,
    /// Timestamp when the entry fill occurred.
    pub entry_time: Timestamp,
    /// Timestamp when the exit fill occurred.
    pub exit_time: Timestamp,
    /// Trade direction (long or short).
    pub side: Side,
    /// Number of contracts traded.
    pub quantity: f64,
    /// Price at which the position was entered.
    pub entry_price: Price,
    /// Price at which the position was exited.
    pub exit_price: Price,
    /// Stop-loss price at time of entry.
    pub initial_stop_loss: Price,
    /// Take-profit price at time of entry, if one was set.
    pub initial_take_profit: Option<Price>,
    /// P&L in ticks (positive = profit).
    ///
    /// Long: `(exit - entry) / tick_size`.
    /// Short: `(entry - exit) / tick_size`.
    pub pnl_ticks: i64,
    /// Gross P&L in USD, before commission.
    pub pnl_gross_usd: f64,
    /// Round-trip commission in USD.
    pub commission_usd: f64,
    /// Net P&L in USD: `pnl_gross_usd - commission_usd`.
    pub pnl_net_usd: f64,
    /// Risk-reward ratio: `pnl_ticks / stop_distance_ticks`.
    pub rr_ratio: f64,
    /// Maximum Adverse Excursion in ticks from entry
    /// (always >= 0).
    ///
    /// Measures the worst unrealized loss the trade experienced
    /// before closing. Useful for evaluating stop placement.
    pub mae_ticks: i64,
    /// Maximum Favorable Excursion in ticks from entry
    /// (always >= 0).
    ///
    /// Measures the best unrealized profit the trade experienced
    /// before closing. Useful for evaluating target placement.
    pub mfe_ticks: i64,
    /// Why the position was closed.
    pub exit_reason: ExitReason,
    /// Optional human-readable label for the trade (e.g. strategy
    /// signal name).
    pub label: Option<String>,
    /// Instrument this trade was executed on, if available.
    #[serde(default)]
    pub instrument: Option<FuturesTicker>,
    /// Trade duration in milliseconds (`exit_time - entry_time`).
    #[serde(default)]
    pub duration_ms: Option<u64>,
    /// Snapshot of surrounding candle data and strategy context at
    /// trade close. `None` for older backtest results.
    #[serde(default)]
    pub snapshot: Option<TradeSnapshot>,
}
