use kairos_data::{Price, Side, Timestamp};
use serde::{Deserialize, Serialize};

/// Reason a position was closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExitReason {
    StopLoss,
    TakeProfit,
    TrailingStop,
    SessionClose,
    Manual,
}

impl std::fmt::Display for ExitReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StopLoss => write!(f, "Stop Loss"),
            Self::TakeProfit => write!(f, "Take Profit"),
            Self::TrailingStop => write!(f, "Trailing Stop"),
            Self::SessionClose => write!(f, "Session Close"),
            Self::Manual => write!(f, "Manual"),
        }
    }
}

/// A single completed round-trip trade record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    /// 1-indexed trade sequence number within the run.
    pub index: usize,
    pub entry_time: Timestamp,
    pub exit_time: Timestamp,
    pub side: Side,
    pub quantity: f64,
    pub entry_price: Price,
    pub exit_price: Price,
    /// Stop-loss price at time of entry.
    pub initial_stop_loss: Price,
    /// Take-profit price at time of entry (if any).
    pub initial_take_profit: Option<Price>,
    /// PnL in ticks. Positive = profit. Long: (exit-entry)/tick. Short: (entry-exit)/tick.
    pub pnl_ticks: i64,
    /// Gross PnL in USD before commission.
    pub pnl_gross_usd: f64,
    pub commission_usd: f64,
    /// Net PnL = gross - commission.
    pub pnl_net_usd: f64,
    /// Risk-reward ratio = pnl_ticks / stop_distance_ticks.
    pub rr_ratio: f64,
    /// Max Adverse Excursion in ticks from entry (always >= 0).
    pub mae_ticks: i64,
    /// Max Favorable Excursion in ticks from entry (always >= 0).
    pub mfe_ticks: i64,
    pub exit_reason: ExitReason,
    pub label: Option<String>,
}
