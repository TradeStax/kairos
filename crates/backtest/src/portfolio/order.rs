use crate::domain::trade_record::ExitReason;
use kairos_data::Price;

/// Type of automatic exit order tracked by the broker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingOrderType {
    StopLoss,
    TakeProfit,
    TrailingStop,
}

impl PendingOrderType {
    pub fn to_exit_reason(self) -> ExitReason {
        match self {
            Self::StopLoss => ExitReason::StopLoss,
            Self::TakeProfit => ExitReason::TakeProfit,
            Self::TrailingStop => ExitReason::TrailingStop,
        }
    }
}

/// Result of the broker's fill-check on a tick — price at which the position closes.
#[derive(Debug, Clone)]
pub struct FillEvent {
    pub fill_price: Price,
    pub exit_reason: ExitReason,
    /// True when the fill gapped through the stop price at session open.
    pub is_gap_fill: bool,
}
