//! Monotonic simulation clock for the backtest engine.
//!
//! [`EngineClock`] tracks the current, start, and end timestamps
//! of the simulation. It enforces forward-only time progression to
//! guarantee deterministic replay.

use kairos_data::Timestamp;

/// Monotonic simulation clock that tracks the current time within
/// a backtest run.
///
/// The clock advances strictly forward — any attempt to move time
/// backward is logged as a warning and ignored, preserving the
/// determinism invariant.
pub struct EngineClock {
    /// Current simulation timestamp.
    current: Timestamp,
    /// Timestamp of the first event processed.
    start: Option<Timestamp>,
    /// Timestamp of the most recent event processed.
    end: Option<Timestamp>,
}

impl EngineClock {
    /// Creates a new clock at time zero with no start/end recorded.
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: Timestamp(0),
            start: None,
            end: None,
        }
    }

    /// Advances the clock to the given timestamp.
    ///
    /// If `ts` is earlier than the current time, a warning is logged
    /// and the clock remains unchanged (determinism invariant).
    pub fn advance(&mut self, ts: Timestamp) {
        if ts.0 < self.current.0 {
            log::warn!(
                "EngineClock: time went backward: {} -> {}",
                self.current.0,
                ts.0
            );
            return;
        }
        if self.start.is_none() {
            self.start = Some(ts);
        }
        self.current = ts;
        self.end = Some(ts);
    }

    /// Returns the current simulation timestamp.
    #[must_use]
    pub fn now(&self) -> Timestamp {
        self.current
    }

    /// Returns the timestamp of the first event, if any have been
    /// processed.
    #[must_use]
    pub fn start_time(&self) -> Option<Timestamp> {
        self.start
    }

    /// Returns the timestamp of the most recent event, if any have
    /// been processed.
    #[must_use]
    pub fn end_time(&self) -> Option<Timestamp> {
        self.end
    }

    /// Returns the elapsed simulation time in milliseconds from the
    /// first event to the most recent event.
    #[must_use]
    pub fn elapsed_ms(&self) -> u64 {
        match (self.start, self.end) {
            (Some(s), Some(e)) => e.0.saturating_sub(s.0),
            _ => 0,
        }
    }

    /// Resets the clock to its initial state (time zero, no
    /// start/end).
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for EngineClock {
    fn default() -> Self {
        Self::new()
    }
}
