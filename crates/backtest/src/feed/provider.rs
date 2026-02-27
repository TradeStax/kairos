//! Trade data provider trait for historical data fetching.
//!
//! [`TradeProvider`] defines the interface between the backtest
//! engine and the data layer. Implementations may read from the
//! local cache, fetch from a remote API, or generate synthetic data.

use kairos_data::{DateRange, FuturesTicker, Trade};
use std::future::Future;
use std::pin::Pin;

/// Async trait for fetching historical trades over a date range.
///
/// This trait keeps the backtest crate transport-agnostic: the
/// engine depends only on this trait, not on concrete adapters
/// like `DataEngine` or `DatabentoClient`.
///
/// # Implementors
///
/// The app layer provides an implementation backed by the
/// `DataEngine` cache and fetcher infrastructure.
pub trait TradeProvider: Send + Sync {
    /// Fetches all trades for `ticker` within `date_range`.
    ///
    /// Returns trades sorted in ascending time order. Returns an
    /// empty `Vec` if no data is available for the range.
    fn get_trades(
        &self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Trade>, String>> + Send + '_>>;
}
