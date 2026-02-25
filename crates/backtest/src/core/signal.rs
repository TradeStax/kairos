use crate::domain::trade_record::ExitReason;
use kairos_data::{Price, Side};
use serde::{Deserialize, Serialize};

/// Signal returned by strategy callbacks to direct the engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Signal {
    /// Open a new position.
    Open {
        side: Side,
        /// Number of contracts. Must be > 0.
        quantity: f64,
        /// Optional override (ignored — use `quantity` instead; kept for API symmetry).
        quantity_override: Option<f64>,
        /// Stop-loss price. Required for position sizing when using RiskPercent mode.
        stop_loss: Price,
        /// Optional take-profit price.
        take_profit: Option<Price>,
        /// Optional label for this trade (visible in results).
        label: Option<String>,
    },
    /// Close the current position with the given reason.
    Close { reason: ExitReason },
    /// Close all open positions with the given reason.
    CloseAll { reason: ExitReason },
    /// Update the trailing stop on the open position.
    UpdateStop { new_stop: Price },
    /// Do nothing this tick.
    Hold,
}
