//! Fundamental value objects and error handling.
//!
//! - [`types`] — `Price` (i64, 10^-8 precision), `Timestamp`, `Volume`, `Quantity`,
//!   `Side`, `DateRange`, `TimeRange`, `FeedId`
//! - [`price`] — `PriceStep`, `PriceExt` formatting, `Power10` generic,
//!   `MinTicksize`, `ContractSize`, `MinQtySize`
//! - [`color`] — `Rgba` / `SerializableColor`, hex conversion
//! - [`error`] — `AppError` trait, `ErrorSeverity` enum

pub mod color;
pub mod error;
pub mod price;
pub mod types;

// Re-export commonly used types
pub use color::{Rgba, SerializableColor, hex_to_rgba, rgba_to_hex};
pub use error::{AppError, ErrorSeverity};
pub use price::{
    ContractSize, MinQtySize, MinTicksize, Power10, PriceExt, PriceStep, ms_to_datetime,
};
pub use types::{DateRange, FeedId, Price, Quantity, Side, TimeRange, Timestamp, Volume};
