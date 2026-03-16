//! Session record types and database cache for IVB historical analysis.

use crate::util::session::{SessionInfo, SessionType};
use data::Candle;
use serde::{Deserialize, Serialize};

/// Serializable session database for export/import.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDatabase {
    pub version: u16,
    pub instrument: String,
    pub or_window_minutes: u32,
    pub records: Vec<IvbSessionRecord>,
    pub last_date: String,
}

/// Historical data for one completed RTH session's opening range
/// behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct IvbSessionRecord {
    pub date: String,
    pub or_high_units: i64,
    pub or_low_units: i64,
    pub or_mid_units: i64,
    pub or_range_units: i64,
    pub or_high_formed_first: bool,

    pub broke_high: bool,
    pub max_extension_above_units: i64,
    pub extension_above_ratio: f64,

    pub broke_low: bool,
    pub max_extension_below_units: i64,
    pub extension_below_ratio: f64,

    // 0=Mon, 1=Tue, 2=Wed, 3=Thu, 4=Fri, 5=Sat, 6=Sun
    // (Unix epoch 1970-01-01 was Thursday)
    pub day_of_week: u8,
    pub or_range_percentile: f64,
    pub overnight_gap_units: i64,
    pub session_high_units: i64,
    pub session_low_units: i64,
    pub session_close_units: i64,

    // Phase 2 additions
    pub or_close_units: i64,
    pub session_close_above_or_high: bool,
    pub session_close_below_or_low: bool,
    pub retraced_to_mid_after_high_break: bool,
    pub retraced_to_mid_after_low_break: bool,
    pub break_high_time: Option<u64>,
    pub break_low_time: Option<u64>,
    pub time_to_max_above: Option<u64>,
    pub time_to_max_below: Option<u64>,
    pub session_range_units: i64,
}

/// Build session records from completed RTH sessions.
pub fn build_session_records(
    sessions: &[SessionInfo],
    candles: &[Candle],
    or_window_minutes: u32,
) -> Vec<IvbSessionRecord> {
    let or_duration_ms = u64::from(or_window_minutes) * 60 * 1000;
    let mut records = Vec::new();
    let mut prior_rth_close: Option<i64> = None;

    // Collect all OR ranges for percentile computation
    let mut all_or_ranges: Vec<f64> = Vec::new();

    for session in sessions {
        if session.key.session_type != SessionType::Rth || !session.is_complete {
            if session.key.session_type == SessionType::Rth {
                prior_rth_close = Some(session.close_units);
            }
            continue;
        }

        // Holiday detection: skip sessions < 4 hours
        let duration = session.close_time.saturating_sub(session.open_time);
        if duration < 14_400_000 {
            prior_rth_close = Some(session.close_units);
            continue;
        }

        let (Some(or_high), Some(or_low)) = (session.or_high_units, session.or_low_units) else {
            prior_rth_close = Some(session.close_units);
            continue;
        };

        let or_range = or_high - or_low;
        if or_range <= 0 {
            prior_rth_close = Some(session.close_units);
            continue;
        }

        let or_mid = (or_high + or_low) / 2;
        let or_end_time = session.open_time + or_duration_ms;
        let mut first_high_time = u64::MAX;
        let mut first_low_time = u64::MAX;
        let mut or_close_units: i64 = session.open_units;

        let (start_ci, end_ci) = session.candle_range;
        let range_end = (end_ci + 1).min(candles.len());

        // Scan OR candles
        for c in &candles[start_ci..range_end] {
            if c.time.0 >= or_end_time {
                break;
            }
            if c.high.units() == or_high && first_high_time == u64::MAX {
                first_high_time = c.time.0;
            }
            if c.low.units() == or_low && first_low_time == u64::MAX {
                first_low_time = c.time.0;
            }
            or_close_units = c.close.units();
        }

        // Scan post-OR candles for extensions and breakout times
        let mut max_above: i64 = 0;
        let mut max_below: i64 = 0;
        let mut break_high_time: Option<u64> = None;
        let mut break_low_time: Option<u64> = None;
        let mut time_of_max_above: Option<u64> = None;
        let mut time_of_max_below: Option<u64> = None;
        let mut retraced_mid_after_high = false;
        let mut retraced_mid_after_low = false;

        for c in &candles[start_ci..range_end] {
            if c.time.0 < or_end_time {
                continue;
            }
            let ext_above = c.high.units() - or_high;
            if ext_above > 0 && break_high_time.is_none() {
                break_high_time = Some(c.time.0);
            }
            if ext_above > max_above {
                max_above = ext_above;
                time_of_max_above = Some(c.time.0);
            }

            let ext_below = or_low - c.low.units();
            if ext_below > 0 && break_low_time.is_none() {
                break_low_time = Some(c.time.0);
            }
            if ext_below > max_below {
                max_below = ext_below;
                time_of_max_below = Some(c.time.0);
            }

            // Check retracement to mid after breakout
            if break_high_time.is_some() && c.low.units() <= or_mid {
                retraced_mid_after_high = true;
            }
            if break_low_time.is_some() && c.high.units() >= or_mid {
                retraced_mid_after_low = true;
            }
        }

        let broke_high = session.high_units > or_high;
        let broke_low = session.low_units < or_low;
        let or_range_f64 = or_range as f64;

        let overnight_gap = prior_rth_close
            .map(|pc| session.open_units - pc)
            .unwrap_or(0);

        // Day of week from timestamp
        let secs = (session.open_time / 1000) as i64;
        let days = secs.div_euclid(86400);
        let dow = ((days + 3) % 7) as u8; // epoch was Thursday=3

        all_or_ranges.push(or_range_f64);

        let time_to_max_above = time_of_max_above.map(|t| t.saturating_sub(or_end_time));
        let time_to_max_below = time_of_max_below.map(|t| t.saturating_sub(or_end_time));

        records.push(IvbSessionRecord {
            date: session.key.trade_date.clone(),
            or_high_units: or_high,
            or_low_units: or_low,
            or_mid_units: or_mid,
            or_range_units: or_range,
            or_high_formed_first: first_high_time <= first_low_time,
            broke_high,
            max_extension_above_units: max_above,
            extension_above_ratio: if or_range > 0 {
                max_above as f64 / or_range_f64
            } else {
                0.0
            },
            broke_low,
            max_extension_below_units: max_below,
            extension_below_ratio: if or_range > 0 {
                max_below as f64 / or_range_f64
            } else {
                0.0
            },
            day_of_week: dow,
            or_range_percentile: 0.0, // computed below
            overnight_gap_units: overnight_gap,
            session_high_units: session.high_units,
            session_low_units: session.low_units,
            session_close_units: session.close_units,
            or_close_units,
            session_close_above_or_high: session.close_units > or_high,
            session_close_below_or_low: session.close_units < or_low,
            retraced_to_mid_after_high_break: retraced_mid_after_high,
            retraced_to_mid_after_low_break: retraced_mid_after_low,
            break_high_time,
            break_low_time,
            time_to_max_above,
            time_to_max_below,
            session_range_units: session.high_units - session.low_units,
        });

        prior_rth_close = Some(session.close_units);
    }

    // Compute OR range percentiles (rolling — no lookahead)
    for i in 0..records.len() {
        let mut prior_ranges: Vec<f64> = all_or_ranges[..=i].to_vec();
        prior_ranges.sort_by(|a, b| a.partial_cmp(b).unwrap());
        records[i].or_range_percentile =
            crate::util::math::percentile_rank(&prior_ranges, all_or_ranges[i]);
    }

    records
}
