//! Session model for CME futures — RTH and ETH boundaries.
//!
//! Groups candles into RTH (Regular Trading Hours) and ETH (Extended
//! Trading Hours) sessions. Each session tracks OHLC, opening range,
//! candle index range, and completion status.

use std::collections::BTreeMap;

use data::{Candle, Trade};
use serde::{Deserialize, Serialize};

/// Seconds in a day.
const SECS_PER_DAY: i64 = 86_400;

/// Buffer added after session close_time when mapping trades to
/// sessions, ensuring trades from the final candle are captured.
const TRADE_RANGE_BUFFER_MS: u64 = 60_000;

/// Session type: Regular or Extended trading hours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionType {
    Rth,
    Eth,
}

impl std::fmt::Display for SessionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rth => write!(f, "RTH"),
            Self::Eth => write!(f, "ETH"),
        }
    }
}

/// Uniquely identifies a trading session.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionKey {
    /// Trade date in "YYYY-MM-DD" format.
    pub trade_date: String,
    /// RTH or ETH.
    pub session_type: SessionType,
}

impl SessionKey {
    /// Sentinel key for manual levels (not tied to a session).
    pub fn manual() -> Self {
        Self {
            trade_date: "manual".into(),
            session_type: SessionType::Rth,
        }
    }

    /// Short tag for chart labels: "R02-25" or "E02-25".
    pub fn short_tag(&self) -> String {
        if self.trade_date == "manual" {
            return String::new();
        }
        let prefix = match self.session_type {
            SessionType::Rth => "R",
            SessionType::Eth => "E",
        };
        // Extract MM-DD from "YYYY-MM-DD"
        if self.trade_date.len() >= 10 {
            let mm_dd = &self.trade_date[5..10];
            format!("{prefix}{mm_dd}")
        } else {
            format!("{prefix}{}", self.trade_date)
        }
    }

    /// Whether this is a cross-session level (not tied to a
    /// specific session's profile).
    pub fn is_cross_session(&self) -> bool {
        self.trade_date == "cross"
    }

    /// Sentinel key for cross-session levels (prior day, opening
    /// range).
    pub fn cross_session() -> Self {
        Self {
            trade_date: "cross".into(),
            session_type: SessionType::Rth,
        }
    }
}

impl Ord for SessionKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.trade_date.cmp(&other.trade_date).then_with(|| {
            // ETH comes before RTH within the same date
            let a = match self.session_type {
                SessionType::Eth => 0,
                SessionType::Rth => 1,
            };
            let b = match other.session_type {
                SessionType::Eth => 0,
                SessionType::Rth => 1,
            };
            a.cmp(&b)
        })
    }
}

impl PartialOrd for SessionKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// A single trading session with aggregated data.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub key: SessionKey,
    pub high_units: i64,
    pub low_units: i64,
    pub close_units: i64,
    pub open_units: i64,
    pub open_time: u64,
    /// Timestamp of the last candle in this session.
    pub close_time: u64,
    /// Opening range high (only meaningful for RTH).
    pub or_high_units: Option<i64>,
    /// Opening range low (only meaningful for RTH).
    pub or_low_units: Option<i64>,
    /// Inclusive range of candle indices into the candle slice.
    pub candle_range: (usize, usize),
    /// Whether this session has completed (a later session exists).
    pub is_complete: bool,
}

// ── DST-aware Session Boundaries (UTC) ─────────────────────────

/// Returns (rth_open, rth_close, maintenance_end) in
/// minutes-since-midnight UTC, adjusted for US Central Time DST.
fn rth_boundaries(ts_ms: u64) -> (u32, u32, u32) {
    if is_cdt(ts_ms) {
        // CDT (UTC-5): 8:30–15:00 CT → 14:30–21:00 UTC, maint ends 22:00
        (14 * 60 + 30, 21 * 60, 22 * 60)
    } else {
        // CST (UTC-6): 8:30–15:00 CT → 15:30–22:00 UTC, maint ends 23:00
        (15 * 60 + 30, 22 * 60, 23 * 60)
    }
}

/// Determine if a UTC timestamp falls in CDT (true) or CST (false).
///
/// US Central Time DST rules (since 2007):
/// - Spring forward (CST→CDT): Second Sunday of March at 2:00 AM
///   local = 08:00 UTC
/// - Fall back (CDT→CST): First Sunday of November at 2:00 AM
///   local = 07:00 UTC (during CDT)
fn is_cdt(ts_ms: u64) -> bool {
    let secs = (ts_ms / 1000) as i64;
    let days = secs.div_euclid(SECS_PER_DAY);
    let (year, month, day) = days_to_ymd(days);
    let day_secs = secs.rem_euclid(SECS_PER_DAY) as u32;
    let hour = day_secs / 3600;

    match month {
        1 | 2 | 12 => false,
        4..=10 => true,
        3 => {
            let spring = second_sunday_of_march(year);
            if day > spring {
                true
            } else if day < spring {
                false
            } else {
                hour >= 8
            }
        }
        11 => {
            let fall = first_sunday_of_november(year);
            if day > fall {
                false
            } else if day < fall {
                true
            } else {
                hour < 7
            }
        }
        _ => true,
    }
}

/// Day-of-month for the second Sunday of March.
fn second_sunday_of_march(year: i32) -> u32 {
    let dow = day_of_week(year, 3, 1); // 0=Mon..6=Sun
    let first_sunday = 1 + (6 - dow) % 7;
    first_sunday + 7
}

/// Day-of-month for the first Sunday of November.
fn first_sunday_of_november(year: i32) -> u32 {
    let dow = day_of_week(year, 11, 1);
    1 + (6 - dow) % 7
}

/// Day of week (0=Mon, 1=Tue, ..., 6=Sun).
/// Tomohiko Sakamoto's algorithm (adjusted for 0=Mon).
fn day_of_week(year: i32, month: u32, day: u32) -> u32 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    let dow = (y + y / 4 - y / 100 + y / 400 + t[(month - 1) as usize] + day as i32) % 7;
    // Sakamoto returns 0=Sun; convert to 0=Mon
    ((dow + 6) % 7) as u32
}

/// Convert a unix timestamp (ms) to (hour, minute, YYYY-MM-DD
/// date, next-day date).
fn decompose_ts(ts_ms: u64) -> (u32, u32, String, String) {
    let secs = (ts_ms / 1000) as i64;
    let day_secs = secs.rem_euclid(SECS_PER_DAY) as u32;
    let hour = day_secs / 3600;
    let minute = (day_secs % 3600) / 60;

    let days = secs.div_euclid(SECS_PER_DAY);
    let (y, m, d) = days_to_ymd(days);
    let date = format!("{y:04}-{m:02}-{d:02}");

    let (ny, nm, nd) = days_to_ymd(days + 1);
    let next_date = format!("{ny:04}-{nm:02}-{nd:02}");

    (hour, minute, date, next_date)
}

/// Convert days since epoch to (year, month, day).
fn days_to_ymd(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

/// Assign a timestamp to its session (RTH or ETH).
///
/// Returns `None` during the CME maintenance window. Boundaries
/// shift with US Central Time DST:
///   CDT: RTH 14:30–21:00, maint 21:00–22:00, late ETH 22:00+
///   CST: RTH 15:30–22:00, maint 22:00–23:00, late ETH 23:00+
pub fn assign_session(ts_ms: u64) -> Option<SessionKey> {
    let (hour, minute, date, next_date) = decompose_ts(ts_ms);
    let hm = hour * 60 + minute;
    let (rth_open, rth_close, maint_end) = rth_boundaries(ts_ms);

    if (rth_open..rth_close).contains(&hm) {
        Some(SessionKey {
            trade_date: date,
            session_type: SessionType::Rth,
        })
    } else if hm < rth_open {
        Some(SessionKey {
            trade_date: date,
            session_type: SessionType::Eth,
        })
    } else if hm >= maint_end {
        Some(SessionKey {
            trade_date: next_date,
            session_type: SessionType::Eth,
        })
    } else {
        None
    }
}

/// Check if a timestamp falls within RTH hours.
pub fn is_rth(ts_ms: u64) -> bool {
    let (hour, minute, _, _) = decompose_ts(ts_ms);
    let hm = hour * 60 + minute;
    let (rth_open, rth_close, _) = rth_boundaries(ts_ms);
    (rth_open..rth_close).contains(&hm)
}

/// Extract sessions from candles, grouping into RTH and ETH.
///
/// Returns sessions sorted chronologically (ETH before RTH within
/// the same date). The last session is the current (potentially
/// incomplete) one. `is_complete` is set based on whether a later
/// session exists in the data.
pub fn extract_sessions(candles: &[Candle], opening_range_minutes: u32) -> Vec<SessionInfo> {
    if candles.is_empty() {
        return Vec::new();
    }

    let or_duration_ms = u64::from(opening_range_minutes) * 60 * 1000;
    let mut session_map: BTreeMap<SessionKey, SessionInfo> = BTreeMap::new();

    for (idx, c) in candles.iter().enumerate() {
        let ts = c.time.0;
        let Some(key) = assign_session(ts) else {
            continue; // maintenance window
        };

        let h = c.high.units();
        let l = c.low.units();
        let cl = c.close.units();
        let op = c.open.units();

        match session_map.get_mut(&key) {
            Some(session) => {
                if h > session.high_units {
                    session.high_units = h;
                }
                if l < session.low_units {
                    session.low_units = l;
                }
                session.close_units = cl;
                session.close_time = ts;
                session.candle_range.1 = idx;

                // Opening range (RTH only)
                if key.session_type == SessionType::Rth && ts < session.open_time + or_duration_ms {
                    match session.or_high_units {
                        Some(orh) if h > orh => {
                            session.or_high_units = Some(h);
                        }
                        None => session.or_high_units = Some(h),
                        _ => {}
                    }
                    match session.or_low_units {
                        Some(orl) if l < orl => {
                            session.or_low_units = Some(l);
                        }
                        None => session.or_low_units = Some(l),
                        _ => {}
                    }
                }
            }
            None => {
                let or_high = if key.session_type == SessionType::Rth {
                    Some(h)
                } else {
                    None
                };
                let or_low = if key.session_type == SessionType::Rth {
                    Some(l)
                } else {
                    None
                };

                session_map.insert(
                    key.clone(),
                    SessionInfo {
                        key,
                        high_units: h,
                        low_units: l,
                        close_units: cl,
                        open_units: op,
                        open_time: ts,
                        close_time: ts,
                        or_high_units: or_high,
                        or_low_units: or_low,
                        candle_range: (idx, idx),
                        is_complete: false,
                    },
                );
            }
        }
    }

    // Convert to sorted Vec and mark completion
    let mut sessions: Vec<SessionInfo> = session_map.into_values().collect();

    // Mark all sessions except the last as complete
    let len = sessions.len();
    for (i, session) in sessions.iter_mut().enumerate() {
        session.is_complete = i + 1 < len;
    }

    sessions
}

/// Map each session's time range to trade slice indices via binary
/// search.
///
/// Returns a vec parallel to `sessions` where each entry is
/// `(start_idx, end_idx)` (exclusive end) into the `trades` slice.
pub fn trade_ranges_for_sessions(
    trades: &[Trade],
    sessions: &[SessionInfo],
) -> Vec<(usize, usize)> {
    if trades.is_empty() || sessions.is_empty() {
        return vec![(0, 0); sessions.len()];
    }

    sessions
        .iter()
        .map(|session| {
            let start = trades.partition_point(|t| t.time.0 < session.open_time);
            // Add 1ms buffer after close_time to include the
            // closing candle's trades
            let end =
                trades.partition_point(|t| t.time.0 <= session.close_time + TRADE_RANGE_BUFFER_MS);
            (start, end)
        })
        .collect()
}

/// Filter sessions by type preference.
pub fn filter_sessions_by_type(sessions: &[SessionInfo], session_types: &str) -> Vec<usize> {
    sessions
        .iter()
        .enumerate()
        .filter(|(_, s)| match session_types {
            "RTH Only" => s.key.session_type == SessionType::Rth,
            "ETH Only" => s.key.session_type == SessionType::Eth,
            _ => true, // "RTH + ETH"
        })
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::{Price, Timestamp, Volume};

    #[test]
    fn test_days_to_ymd_epoch() {
        assert_eq!(days_to_ymd(0), (1970, 1, 1));
    }

    #[test]
    fn test_days_to_ymd_known_date() {
        assert_eq!(days_to_ymd(19737), (2024, 1, 15));
    }

    #[test]
    fn test_assign_session_rth_cdt() {
        // July 15, 2024 15:00 UTC → RTH during CDT (14:30–21:00)
        let ts = 1721055600000u64;
        let key = assign_session(ts).unwrap();
        assert_eq!(key.session_type, SessionType::Rth);
    }

    #[test]
    fn test_assign_session_rth_cst() {
        // 2024-02-26 16:00 UTC → RTH during CST (15:30–22:00)
        let ts = 1708964400000u64;
        let key = assign_session(ts).unwrap();
        assert_eq!(key.session_type, SessionType::Rth);
    }

    #[test]
    fn test_assign_session_eth_morning() {
        // 10:00 UTC → ETH (both CST and CDT)
        let ts = 1708942800000; // 2024-02-26 10:00 UTC
        let key = assign_session(ts).unwrap();
        assert_eq!(key.session_type, SessionType::Eth);
    }

    #[test]
    fn test_assign_session_eth_before_rth_cst() {
        // 2024-02-26 15:00 UTC → ETH during CST (RTH starts 15:30)
        let ts = 1708960800000u64;
        let key = assign_session(ts).unwrap();
        assert_eq!(key.session_type, SessionType::Eth);
    }

    #[test]
    fn test_assign_session_eth_evening() {
        // 23:00 UTC → ETH for next day (both CST and CDT)
        let ts = 1708989600000; // 2024-02-26 23:00 UTC
        let key = assign_session(ts).unwrap();
        assert_eq!(key.session_type, SessionType::Eth);
        assert_eq!(key.trade_date, "2024-02-27");
    }

    #[test]
    fn test_assign_session_maintenance_cdt() {
        // July 15, 2024 21:30 UTC → maintenance during CDT
        let ts = 1721079000000u64;
        assert!(assign_session(ts).is_none());
    }

    #[test]
    fn test_assign_session_maintenance_cst() {
        // 2024-02-26 22:30 UTC → maintenance during CST
        let ts = 1708987800000u64;
        assert!(assign_session(ts).is_none());
    }

    #[test]
    fn test_session_key_ordering() {
        let eth = SessionKey {
            trade_date: "2024-02-26".into(),
            session_type: SessionType::Eth,
        };
        let rth = SessionKey {
            trade_date: "2024-02-26".into(),
            session_type: SessionType::Rth,
        };
        assert!(eth < rth);
    }

    #[test]
    fn test_session_key_short_tag() {
        let key = SessionKey {
            trade_date: "2024-02-25".into(),
            session_type: SessionType::Rth,
        };
        assert_eq!(key.short_tag(), "R02-25");

        let eth = SessionKey {
            trade_date: "2024-02-25".into(),
            session_type: SessionType::Eth,
        };
        assert_eq!(eth.short_tag(), "E02-25");
    }

    #[test]
    fn test_extract_sessions_empty() {
        assert!(extract_sessions(&[], 30).is_empty());
    }

    fn make_candle(time: u64, o: f64, h: f64, l: f64, c: f64) -> Candle {
        Candle {
            time: Timestamp(time),
            open: Price::from_f64(o),
            high: Price::from_f64(h),
            low: Price::from_f64(l),
            close: Price::from_f64(c),
            buy_volume: Volume(50.0),
            sell_volume: Volume(50.0),
        }
    }

    #[test]
    fn test_extract_sessions_rth_and_eth() {
        // ETH candle at 10:00 UTC
        let eth_time = 1708942800000u64;
        // RTH candle at 16:00 UTC (CST: RTH is 15:30–22:00)
        let rth_time = 1708964400000u64;

        let candles = vec![
            make_candle(eth_time, 100.0, 102.0, 99.0, 101.0),
            make_candle(rth_time, 101.0, 103.0, 100.0, 102.0),
        ];

        let sessions = extract_sessions(&candles, 30);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].key.session_type, SessionType::Eth,);
        assert_eq!(sessions[1].key.session_type, SessionType::Rth,);
        // First session is complete (second exists)
        assert!(sessions[0].is_complete);
        // Last session is not complete
        assert!(!sessions[1].is_complete);
    }

    #[test]
    fn test_extract_sessions_completion() {
        // Two RTH sessions on different days (CST: 16:00 UTC = RTH)
        let day1_rth = 1708964400000u64; // Feb 26 16:00
        let day2_rth = day1_rth + 86_400_000; // Feb 27 16:00

        let candles = vec![
            make_candle(day1_rth, 100.0, 102.0, 99.0, 101.0),
            make_candle(day2_rth, 101.0, 103.0, 100.0, 102.0),
        ];

        let sessions = extract_sessions(&candles, 30);
        // Should have sessions (some may be ETH depending on exact
        // time mapping, but at minimum RTH sessions)
        assert!(sessions.len() >= 2);
        // All but last should be complete
        for s in &sessions[..sessions.len() - 1] {
            assert!(s.is_complete);
        }
        assert!(!sessions.last().unwrap().is_complete);
    }

    // ── DST tests ──────────────────────────────────────────────

    #[test]
    fn test_is_cdt_summer() {
        // July 15, 2024 15:00 UTC — definitely CDT
        let ts = 1721055600000u64;
        assert!(is_cdt(ts));
    }

    #[test]
    fn test_is_cst_winter() {
        // January 15, 2025 15:00 UTC — definitely CST
        let ts = 1736953200000u64;
        assert!(!is_cdt(ts));
    }

    #[test]
    fn test_dst_spring_forward_2024() {
        // March 10, 2024 is the second Sunday of March
        // At 07:59 UTC: still CST
        let before = 1710057540000u64; // 2024-03-10 07:59 UTC
        assert!(!is_cdt(before));
        // At 08:00 UTC: CDT
        let after = 1710057600000u64; // 2024-03-10 08:00 UTC
        assert!(is_cdt(after));
    }

    #[test]
    fn test_dst_fall_back_2024() {
        // November 3, 2024 is the first Sunday of November
        // At 06:59 UTC: still CDT
        let before = 1730617140000u64; // 2024-11-03 06:59 UTC
        assert!(is_cdt(before));
        // At 07:00 UTC: CST
        let after = 1730617200000u64; // 2024-11-03 07:00 UTC
        assert!(!is_cdt(after));
    }

    #[test]
    fn test_is_rth_cst_vs_cdt() {
        // 15:00 UTC: RTH during CDT, ETH during CST
        // CDT day: July 15, 2024
        let cdt_ts = 1721055600000u64; // 2024-07-15 15:00 UTC
        assert!(is_rth(cdt_ts));

        // CST day: Jan 15, 2025
        let cst_ts = 1736953200000u64; // 2025-01-15 15:00 UTC
        assert!(!is_rth(cst_ts));
    }

    #[test]
    fn test_day_of_week_known_dates() {
        // 2024-03-10 is a Sunday
        assert_eq!(day_of_week(2024, 3, 10), 6);
        // 2024-11-03 is a Sunday
        assert_eq!(day_of_week(2024, 11, 3), 6);
        // 2024-01-01 is a Monday
        assert_eq!(day_of_week(2024, 1, 1), 0);
    }

    #[test]
    fn test_second_sunday_of_march() {
        assert_eq!(second_sunday_of_march(2024), 10);
        assert_eq!(second_sunday_of_march(2025), 9);
        assert_eq!(second_sunday_of_march(2026), 8);
    }

    #[test]
    fn test_first_sunday_of_november() {
        assert_eq!(first_sunday_of_november(2024), 3);
        assert_eq!(first_sunday_of_november(2025), 2);
        assert_eq!(first_sunday_of_november(2026), 1);
    }
}
