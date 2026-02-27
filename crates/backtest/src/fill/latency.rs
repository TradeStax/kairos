//! Latency simulation models for order-to-fill delay.
//!
//! These models define how much simulated time elapses between
//! order submission and order activation. This allows backtests to
//! approximate the real-world delay of order routing.
//!
//! **Not yet integrated into the engine** — fills are currently
//! instant. When wired in, the engine will hold submitted orders
//! in a `Pending` state for the configured latency before
//! promoting them to `Active`.

use serde::{Deserialize, Serialize};

/// Trait for simulating order-to-fill latency.
///
/// Implementations return the number of milliseconds an order
/// should be delayed after submission before it becomes active in
/// the simulated order book.
pub trait LatencyModel: Send + Sync {
    /// Simulated latency in milliseconds for this order event.
    fn latency_ms(&self) -> u64;

    /// Create a boxed clone of this latency model.
    ///
    /// Required because `LatencyModel` is object-safe and stored
    /// as `Box<dyn LatencyModel>`.
    fn clone_model(&self) -> Box<dyn LatencyModel>;
}

/// Zero-latency model — orders activate instantly.
///
/// This is the default when no latency model is configured and
/// matches the traditional "instant fill" backtesting assumption.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct ZeroLatency;

impl LatencyModel for ZeroLatency {
    fn latency_ms(&self) -> u64 {
        0
    }

    fn clone_model(&self) -> Box<dyn LatencyModel> {
        Box::new(ZeroLatency)
    }
}

/// Fixed latency model — every order incurs the same constant
/// delay.
///
/// # Example
///
/// ```ignore
/// let model = FixedLatency::new(50); // 50ms delay
/// assert_eq!(model.latency_ms(), 50);
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FixedLatency {
    /// Constant latency in milliseconds applied to every order.
    pub ms: u64,
}

impl FixedLatency {
    /// Create a new fixed-latency model with the given delay.
    #[must_use]
    pub fn new(ms: u64) -> Self {
        Self { ms }
    }
}

impl LatencyModel for FixedLatency {
    fn latency_ms(&self) -> u64 {
        self.ms
    }

    fn clone_model(&self) -> Box<dyn LatencyModel> {
        Box::new(*self)
    }
}
