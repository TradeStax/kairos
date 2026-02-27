//! Simulation clocks for the backtest engine.
//!
//! Two clocks work together:
//! - [`EngineClock`] — monotonic simulation time tracking.
//! - [`SessionClock`] — RTH (Regular Trading Hours) session
//!   boundary detection using a fixed UTC offset.

pub mod session;
pub mod trading;

pub use session::{SessionClock, SessionCloseReason, SessionEvent};
pub use trading::EngineClock;
