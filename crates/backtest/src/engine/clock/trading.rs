use kairos_data::Timestamp;

/// Logical simulation clock — tracks the current simulation time.
pub struct EngineClock {
    current: Timestamp,
    start: Option<Timestamp>,
    end: Option<Timestamp>,
}

impl EngineClock {
    pub fn new() -> Self {
        Self {
            current: Timestamp(0),
            start: None,
            end: None,
        }
    }

    /// Advance the clock to the given timestamp.
    /// Warns and skips if time goes backward (determinism invariant).
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

    pub fn now(&self) -> Timestamp {
        self.current
    }

    pub fn start_time(&self) -> Option<Timestamp> {
        self.start
    }

    pub fn end_time(&self) -> Option<Timestamp> {
        self.end
    }

    pub fn elapsed_ms(&self) -> u64 {
        match (self.start, self.end) {
            (Some(s), Some(e)) => e.0.saturating_sub(s.0),
            _ => 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for EngineClock {
    fn default() -> Self {
        Self::new()
    }
}
