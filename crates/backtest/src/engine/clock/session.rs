//! RTH (Regular Trading Hours) session boundary detection.
//!
//! [`SessionClock`] converts UTC timestamps to local time using a
//! fixed offset and detects session open/close transitions based on
//! configurable HHMM boundaries. It emits [`SessionEvent`]s when
//! transitions occur.

use crate::strategy::context::SessionState;
use kairos_data::Timestamp;

/// Milliseconds per hour.
const MS_PER_HOUR: i64 = 3_600_000;
/// Milliseconds per day.
const MS_PER_DAY: i64 = 86_400_000;

/// Tracks RTH session boundaries using a fixed UTC offset.
///
/// Converts each incoming trade timestamp to local time and
/// determines whether the trade falls within the configured RTH
/// window (`rth_open_hhmm..rth_close_hhmm`). Emits session
/// open/close events on state transitions.
pub struct SessionClock {
    /// Hours offset from UTC (e.g., -5 for US Eastern).
    pub timezone_offset_hours: i32,
    /// RTH open time in HHMM format (e.g., 930 for 09:30).
    pub rth_open_hhmm: u32,
    /// RTH close time in HHMM format (e.g., 1600 for 16:00).
    pub rth_close_hhmm: u32,
    /// The UTC day floor (ms) of the last trade processed.
    current_utc_day_ms: Option<u64>,
    /// Current session state (PreMarket, Open, or Closed).
    pub session_state: SessionState,
    /// Number of trades seen in the current session.
    pub session_trade_count: u32,
}

/// Event emitted when a session boundary is crossed.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// RTH session opened at this timestamp.
    Open {
        /// Timestamp of the first trade in the new session.
        timestamp: Timestamp,
    },
    /// RTH session closed at this timestamp.
    Close {
        /// Timestamp when the close was detected.
        timestamp: Timestamp,
        /// Why the session was closed.
        reason: SessionCloseReason,
    },
}

/// Reason a trading session was closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionCloseReason {
    /// The close HHMM boundary was reached.
    EndOfDay,
    /// A new UTC day was detected while the session was still open.
    NewDayDetected,
}

impl SessionClock {
    /// Creates a new session clock with the given timezone offset
    /// and RTH boundaries.
    ///
    /// # Arguments
    /// - `timezone_offset_hours` — fixed UTC offset (e.g., -5 for
    ///   EST).
    /// - `rth_open_hhmm` — session open in HHMM (e.g., 930).
    /// - `rth_close_hhmm` — session close in HHMM (e.g., 1600).
    #[must_use]
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

    /// Advances the clock by one trade timestamp.
    ///
    /// Returns `Some(SessionEvent)` if a session boundary was
    /// crossed. Only the **first** boundary crossing per call is
    /// returned — if a day rollover triggers a close, the
    /// subsequent open will be emitted on the next call.
    pub fn advance(&mut self, ts: Timestamp) -> Option<SessionEvent> {
        let ms = ts.0;
        let offset_ms = self.timezone_offset_hours as i64 * MS_PER_HOUR;
        let local_ms = ms as i64 + offset_ms;

        // Local time-of-day in HHMM format
        let time_of_day_ms = local_ms.rem_euclid(MS_PER_DAY);
        let local_hour = (time_of_day_ms / MS_PER_HOUR) as u32;
        let local_minute = ((time_of_day_ms % MS_PER_HOUR) / 60_000) as u32;
        let hhmm = local_hour * 100 + local_minute;

        // UTC day boundary detection
        let utc_day_ms = (ms / MS_PER_DAY as u64) * MS_PER_DAY as u64;
        let new_day = self.current_utc_day_ms != Some(utc_day_ms);

        if new_day {
            self.current_utc_day_ms = Some(utc_day_ms);
            if self.session_state == SessionState::Open {
                // Session was open when the day rolled over
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

    /// Computes the local HHMM for a timestamp without advancing
    /// session state.
    ///
    /// Useful for logging or display purposes.
    #[must_use]
    pub fn local_hhmm(&self, ts: Timestamp) -> u32 {
        let offset_ms = self.timezone_offset_hours as i64 * MS_PER_HOUR;
        let local_ms = ts.0 as i64 + offset_ms;
        let time_of_day_ms = local_ms.rem_euclid(MS_PER_DAY);
        let local_hour = (time_of_day_ms / MS_PER_HOUR) as u32;
        let local_minute = ((time_of_day_ms % MS_PER_HOUR) / 60_000) as u32;
        local_hour * 100 + local_minute
    }
}
