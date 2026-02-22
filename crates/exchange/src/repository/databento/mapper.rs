//! Shared type conversions between Databento exchange types and domain types.
//!
//! Centralizes conversions used by both trade and depth repositories to
//! avoid duplication.

use crate::types::TradeSide;
use kairos_data::domain::{
    DepthSnapshot, Price, Quantity, Side, Timestamp, Trade,
};
use kairos_data::repository::RepositoryError;
use std::collections::BTreeMap;

/// Convert an exchange-layer `Trade` to a domain `Trade`.
pub fn convert_trade(trade: &crate::types::Trade) -> Trade {
    Trade {
        time: Timestamp(trade.time),
        price: Price::from_f32(trade.price),
        quantity: Quantity(trade.qty as f64),
        side: match trade.side {
            TradeSide::Buy => Side::Buy,
            TradeSide::Sell => Side::Sell,
        },
    }
}

/// Convert an exchange-layer `Depth` snapshot to a domain `DepthSnapshot`.
pub fn convert_depth_snapshot(
    time: u64,
    depth: &crate::types::Depth,
) -> DepthSnapshot {
    let bids: BTreeMap<Price, Quantity> = depth
        .bids
        .iter()
        .map(|(price_units, qty)| {
            (Price::from_units(*price_units), Quantity(*qty as f64))
        })
        .collect();

    let asks: BTreeMap<Price, Quantity> = depth
        .asks
        .iter()
        .map(|(price_units, qty)| {
            (Price::from_units(*price_units), Quantity(*qty as f64))
        })
        .collect();

    DepthSnapshot {
        time: Timestamp(time),
        bids,
        asks,
    }
}

/// Convert a `DateRange` start date to a UTC `DateTime` at midnight.
pub fn date_range_start_utc(
    date: chrono::NaiveDate,
) -> Result<chrono::DateTime<chrono::Utc>, RepositoryError> {
    date.and_hms_opt(0, 0, 0)
        .ok_or_else(|| {
            RepositoryError::InvalidData("Invalid start date".to_string())
        })?
        .and_utc()
        .pipe(Ok)
}

/// Convert a `DateRange` end date to a UTC `DateTime` at 23:59:59.
pub fn date_range_end_utc(
    date: chrono::NaiveDate,
) -> Result<chrono::DateTime<chrono::Utc>, RepositoryError> {
    date.and_hms_opt(23, 59, 59)
        .ok_or_else(|| {
            RepositoryError::InvalidData("Invalid end date".to_string())
        })?
        .and_utc()
        .pipe(Ok)
}

/// Helper trait for piping values (used by date conversion helpers).
trait Pipe: Sized {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R,
    {
        f(self)
    }
}

impl<T> Pipe for T {}
