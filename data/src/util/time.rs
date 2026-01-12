//! Time and date utilities

use chrono::{DateTime, Datelike, Timelike};
use serde::{Deserialize, Deserializer};

const DAY_MS: u64 = 86_400_000;
const HOUR_MS: u64 = 3_600_000;
const MINUTE_MS: u64 = 60_000;
const SECOND_MS: u64 = 1_000;

pub fn ok_or_default<'a, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Deserialize<'a> + Default,
    D: Deserializer<'a>,
{
    let v: serde_json::Value = Deserialize::deserialize(deserializer)?;
    Ok(T::deserialize(v).unwrap_or_default())
}

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

pub fn reset_to_start_of_month_utc(dt: DateTime<chrono::Utc>) -> DateTime<chrono::Utc> {
    reset_to_start_of_day_utc(dt.with_day(1).unwrap_or(dt))
}

pub fn reset_to_start_of_year_utc(dt: DateTime<chrono::Utc>) -> DateTime<chrono::Utc> {
    reset_to_start_of_month_utc(dt.with_month(1).unwrap_or(dt))
}
