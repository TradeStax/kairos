use crate::strategy::context::SessionState;
use kairos_data::Timestamp;

/// Tracks RTH session boundaries using a fixed UTC offset.
pub struct SessionClock {
    pub timezone_offset_hours: i32,
    pub rth_open_hhmm: u32,
    pub rth_close_hhmm: u32,
    /// The UTC day floor (ms) of the last trade processed.
    current_utc_day_ms: Option<u64>,
    pub session_state: SessionState,
    pub session_trade_count: u32,
}

/// Event emitted when a session boundary is crossed.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    Open {
        timestamp: Timestamp,
    },
    Close {
        timestamp: Timestamp,
        reason: SessionCloseReason,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum SessionCloseReason {
    EndOfDay,
    NewDayDetected,
}

impl SessionClock {
    pub fn new(timezone_offset_hours: i32, rth_open_hhmm: u32, rth_close_hhmm: u32) -> Self {
        Self {
            timezone_offset_hours,
            rth_open_hhmm,
            rth_close_hhmm,
            current_utc_day_ms: None,
            session_state: SessionState::PreMarket,
            session_trade_count: 0,
        }
    }

    /// Advance the clock by one trade.
    ///
    /// Returns `Some(SessionEvent)` if a boundary was crossed.
    /// Only the FIRST boundary crossing in a single call is returned;
    /// call again on the same trade if you need to detect both close + open.
    pub fn advance(&mut self, ts: Timestamp) -> Option<SessionEvent> {
        let ms = ts.0;
        let offset_ms = self.timezone_offset_hours as i64 * 3_600_000_i64;
        let local_ms = ms as i64 + offset_ms;

        // Local time-of-day components
        let ms_per_day = 86_400_000_i64;
        let time_of_day_ms = local_ms.rem_euclid(ms_per_day);
        let local_hour = (time_of_day_ms / 3_600_000) as u32;
        let local_minute = ((time_of_day_ms % 3_600_000) / 60_000) as u32;
        let hhmm = local_hour * 100 + local_minute;

        // UTC day boundary detection
        let utc_day_ms = (ms / 86_400_000) * 86_400_000;
        let new_day = self.current_utc_day_ms != Some(utc_day_ms);

        if new_day {
            self.current_utc_day_ms = Some(utc_day_ms);
            if self.session_state == SessionState::Open {
                // Session was open when the day rolled over — close it.
                self.session_state = SessionState::Closed;
                self.session_trade_count = 0;
                return Some(SessionEvent::Close {
                    timestamp: ts,
                    reason: SessionCloseReason::NewDayDetected,
                });
            }
        }

        // RTH window check
        let in_rth = hhmm >= self.rth_open_hhmm && hhmm < self.rth_close_hhmm;

        if in_rth && self.session_state != SessionState::Open {
            self.session_state = SessionState::Open;
            self.session_trade_count = 0;
            return Some(SessionEvent::Open { timestamp: ts });
        }

        if !in_rth && hhmm >= self.rth_close_hhmm && self.session_state == SessionState::Open {
            self.session_state = SessionState::Closed;
            return Some(SessionEvent::Close {
                timestamp: ts,
                reason: SessionCloseReason::EndOfDay,
            });
        }

        if self.session_state == SessionState::Open {
            self.session_trade_count += 1;
        }

        None
    }

    /// Compute local HHMM for a given timestamp (without advancing state).
    pub fn local_hhmm(&self, ts: Timestamp) -> u32 {
        let offset_ms = self.timezone_offset_hours as i64 * 3_600_000_i64;
        let local_ms = ts.0 as i64 + offset_ms;
        let time_of_day_ms = local_ms.rem_euclid(86_400_000_i64);
        let local_hour = (time_of_day_ms / 3_600_000) as u32;
        let local_minute = ((time_of_day_ms % 3_600_000) / 60_000) as u32;
        local_hour * 100 + local_minute
    }
}
