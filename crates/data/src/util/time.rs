//! Time and date utilities for duration formatting and date truncation.
//!
//! Provides human-readable duration formatting and functions to reset
//! a `DateTime<Utc>` to the start of its day, month, or year.

use chrono::{DateTime, Datelike, Timelike};

/// Milliseconds in one day.
const DAY_MS: u64 = 86_400_000;
/// Milliseconds in one hour.
const HOUR_MS: u64 = 3_600_000;
/// Milliseconds in one minute.
const MINUTE_MS: u64 = 60_000;
/// Milliseconds in one second.
const SECOND_MS: u64 = 1_000;

/// Formats a duration in milliseconds as a human-readable string.
///
/// Adapts precision to magnitude: "2d 3h", "1h 30m", "45s", "123ms".
#[must_use]
pub fn format_duration_ms(diff_ms: u64) -> String {
    if diff_ms >= DAY_MS {
        let days = diff_ms / DAY_MS;
        let hours = (diff_ms % DAY_MS) / HOUR_MS;
        if hours > 0 {
            format!("{}d {}h", days, hours)
        } else {
            format!("{}d", days)
        }
    } else if diff_ms >= HOUR_MS {
        let hours = diff_ms / HOUR_MS;
        let mins = (diff_ms % HOUR_MS) / MINUTE_MS;
        if mins > 0 {
            format!("{}h {}m", hours, mins)
        } else {
            format!("{}h", hours)
        }
    } else if diff_ms >= MINUTE_MS {
        let mins = diff_ms / MINUTE_MS;
        let secs = (diff_ms % MINUTE_MS) / SECOND_MS;
        if secs > 0 {
            format!("{}m {}s", mins, secs)
        } else {
            format!("{}m", mins)
        }
    } else if diff_ms >= 5_000 {
        format!("{}s", diff_ms / SECOND_MS)
    } else {
        format!("{}ms", diff_ms)
    }
}

/// Resets a datetime to the start of its UTC day (00:00:00.000)
#[must_use]
pub fn reset_to_start_of_day_utc(dt: DateTime<chrono::Utc>) -> DateTime<chrono::Utc> {
    dt.with_hour(0)
        .unwrap_or(dt)
        .with_minute(0)
        .unwrap_or(dt)
        .with_second(0)
        .unwrap_or(dt)
        .with_nanosecond(0)
        .unwrap_or(dt)
}

/// Resets a datetime to the start of its UTC month (1st day, 00:00:00.000)
#[must_use]
pub fn reset_to_start_of_month_utc(dt: DateTime<chrono::Utc>) -> DateTime<chrono::Utc> {
    reset_to_start_of_day_utc(dt.with_day(1).unwrap_or(dt))
}

/// Resets a datetime to the start of its UTC year (Jan 1st, 00:00:00.000)
#[must_use]
pub fn reset_to_start_of_year_utc(dt: DateTime<chrono::Utc>) -> DateTime<chrono::Utc> {
    reset_to_start_of_month_utc(dt.with_month(1).unwrap_or(dt))
}
