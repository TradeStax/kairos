//! Core backtest engine — orchestrates deterministic event-driven
//! simulation.
//!
//! The engine module contains:
//! - [`kernel::Engine`] — the main simulation loop that processes
//!   trades, manages candle aggregation, study computation, order
//!   matching, and portfolio updates.
//! - [`runner::BacktestRunner`] — high-level facade that loads data
//!   and delegates to `Engine::run`.
//! - [`clock`] — simulation clock and RTH session boundary tracking.
//!
//! Internal submodules (not re-exported):
//! - `context` — `StrategyContext` construction from cached state.
//! - `processing` — order submission, fill detection, and position
//!   flattening.

pub mod clock;
pub mod kernel;
pub mod runner;

mod context;
mod processing;

/// Returns the current wall-clock time as milliseconds since the
/// Unix epoch.
///
/// Used for benchmarking backtest run duration — not for simulation
/// time (which is driven by [`clock::EngineClock`]).
pub(crate) fn system_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
