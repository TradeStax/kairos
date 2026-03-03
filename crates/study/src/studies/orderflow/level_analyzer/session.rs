//! Session model for CME futures — RTH and ETH boundaries.
//!
//! Groups candles into RTH (Regular Trading Hours) and ETH (Extended
//! Trading Hours) sessions. Each session tracks OHLC, opening range,
//! candle index range, and completion status.

use std::collections::BTreeMap;

use data::{Candle, Trade};
use serde::{Deserialize, Serialize};

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

// ── Time Constants (UTC) ────────────────────────────────────────

/// CME RTH open: 14:30 UTC (8:30 CT during CDT)
const RTH_OPEN_MIN: u32 = 14 * 60 + 30; // 870
/// CME RTH close: 21:00 UTC (15:00 CT during CDT)
const RTH_CLOSE_MIN: u32 = 21 * 60; // 1260
/// CME maintenance end / ETH resume: 22:00 UTC
const MAINTENANCE_END_MIN: u32 = 22 * 60; // 1320

/// Convert a unix timestamp (ms) to (hour, minute, YYYY-MM-DD
/// date, next-day date).
fn decompose_ts(ts_ms: u64) -> (u32, u32, String, String) {
    let secs = (ts_ms / 1000) as i64;
    let day_secs = secs.rem_euclid(86400) as u32;
    let hour = day_secs / 3600;
    let minute = (day_secs % 3600) / 60;

    let days = secs.div_euclid(86400);
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
/// Returns `None` during the CME maintenance window (21:00-22:00
/// UTC).
///
/// Session boundaries (UTC):
///   00:00 – 14:30  → ETH for `date`
///   14:30 – 21:00  → RTH for `date`
///   21:00 – 22:00  → None (maintenance)
///   22:00 – 24:00  → ETH for `next_date`
pub fn assign_session(ts_ms: u64) -> Option<SessionKey> {
    let (hour, minute, date, next_date) = decompose_ts(ts_ms);
    let hm = hour * 60 + minute;

    if (RTH_OPEN_MIN..RTH_CLOSE_MIN).contains(&hm) {
        // RTH: 14:30 – 21:00 UTC
        Some(SessionKey {
            trade_date: date,
            session_type: SessionType::Rth,
        })
    } else if hm < RTH_OPEN_MIN {
        // Early ETH: 00:00 – 14:30 UTC
        Some(SessionKey {
            trade_date: date,
            session_type: SessionType::Eth,
        })
    } else if hm >= MAINTENANCE_END_MIN {
        // Late ETH: 22:00 – 24:00 UTC → next day's ETH
        Some(SessionKey {
            trade_date: next_date,
            session_type: SessionType::Eth,
        })
    } else {
        // Maintenance: 21:00 – 22:00 UTC
        None
    }
}

/// Check if a timestamp falls within RTH hours (for backward
/// compat).
pub fn is_rth(ts_ms: u64) -> bool {
    let (hour, minute, _, _) = decompose_ts(ts_ms);
    let hm = hour * 60 + minute;
    (RTH_OPEN_MIN..RTH_CLOSE_MIN).contains(&hm)
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
            let end = trades.partition_point(|t| t.time.0 <= session.close_time + 60_000);
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
    fn test_assign_session_rth() {
        // 15:00 UTC → RTH
        let ts = 1708960800000; // 2024-02-26 15:00 UTC
        let key = assign_session(ts).unwrap();
        assert_eq!(key.session_type, SessionType::Rth);
    }

    #[test]
    fn test_assign_session_eth_morning() {
        // 10:00 UTC → ETH
        let ts = 1708942800000; // 2024-02-26 10:00 UTC
        let key = assign_session(ts).unwrap();
        assert_eq!(key.session_type, SessionType::Eth);
    }

    #[test]
    fn test_assign_session_eth_evening() {
        // 23:00 UTC → ETH for next day
        let ts = 1708989600000; // 2024-02-26 23:00 UTC
        let key = assign_session(ts).unwrap();
        assert_eq!(key.session_type, SessionType::Eth);
        assert_eq!(key.trade_date, "2024-02-27");
    }

    #[test]
    fn test_assign_session_maintenance() {
        // 21:30 UTC → None (maintenance)
        let ts = 1708982200000; // 2024-02-26 21:30 UTC approx
        let secs = (ts / 1000) as i64;
        let day_secs = secs.rem_euclid(86400) as u32;
        let hour = day_secs / 3600;
        let minute = (day_secs % 3600) / 60;
        let hm = hour * 60 + minute;
        // Verify we're in maintenance window
        assert!(hm >= 1260 && hm < 1320);
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
        // RTH candle at 15:00 UTC
        let rth_time = 1708960800000u64;

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
        // Two RTH sessions on different days
        let day1_rth = 1708960800000u64; // Feb 26 15:00
        let day2_rth = day1_rth + 86_400_000; // Feb 27 15:00

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
}
