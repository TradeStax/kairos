use kairos_data::{DateRange, FuturesTicker, Trade};
use std::future::Future;
use std::pin::Pin;

/// Minimal trait for fetching historical trades.
///
/// Replaces the removed `data::TradeRepository` trait so the backtest
/// crate stays transport-agnostic (not coupled to `DataEngine`).
pub trait TradeProvider: Send + Sync {
    fn get_trades(
        &self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Trade>, String>> + Send + '_>>;
}
