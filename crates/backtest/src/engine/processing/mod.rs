//! Order processing and fill detection for the backtest engine.
//!
//! Split into two submodules:
//! - [`fills`] — passive fill detection against the order book.
//! - [`orders`] — order submission, bracket handling, and position
//!   flattening.

mod fills;
mod orders;
