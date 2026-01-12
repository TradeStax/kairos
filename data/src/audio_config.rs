//! Audio Configuration Types

use crate::domain::FuturesTicker;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

/// Audio stream configuration (persisted to state)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AudioStream {
    pub volume: Option<f32>,
    pub streams: FxHashMap<FuturesTicker, StreamCfg>,
}

/// Per-ticker stream configuration
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct StreamCfg {
    pub enabled: bool,
    pub threshold: Threshold,
}

impl Default for StreamCfg {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: Threshold::default(),
        }
    }
}

/// Audio threshold for triggering sounds
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Threshold {
    /// Trigger when buy/sell count in buffer >= N
    Count(usize),
    /// Trigger when any trade's size >= N
    Qty(usize),
}

impl Default for Threshold {
    fn default() -> Self {
        Threshold::Count(10)
    }
}

