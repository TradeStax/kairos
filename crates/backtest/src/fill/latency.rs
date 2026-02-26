//! Latency simulation models for order-to-fill delay.
//!
//! Not yet integrated into the Engine — fills are currently instant.
//! When implemented, the Engine will use LatencyModel to delay order
//! activation by the configured latency.

use serde::{Deserialize, Serialize};

/// Trait for simulating order-to-fill latency.
pub trait LatencyModel: Send + Sync {
    /// Latency in milliseconds for this order event.
    fn latency_ms(&self) -> u64;
    fn clone_model(&self) -> Box<dyn LatencyModel>;
}

/// No latency -- instant fills.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroLatency;

impl LatencyModel for ZeroLatency {
    fn latency_ms(&self) -> u64 {
        0
    }
    fn clone_model(&self) -> Box<dyn LatencyModel> {
        Box::new(ZeroLatency)
    }
}

/// Fixed latency in milliseconds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixedLatency {
    pub ms: u64,
}

impl FixedLatency {
    pub fn new(ms: u64) -> Self {
        Self { ms }
    }
}

impl LatencyModel for FixedLatency {
    fn latency_ms(&self) -> u64 {
        self.ms
    }
    fn clone_model(&self) -> Box<dyn LatencyModel> {
        Box::new(self.clone())
    }
}
