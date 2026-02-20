//! Utility functions for formatting, math, and time operations

pub mod formatting;
pub mod logging;
pub mod math;
pub mod time;

// Re-export commonly used functions
pub use formatting::{
    abbr_large_numbers, count_decimals, currency_abbr, format_with_commas, pct_change,
};
pub use math::{calc_panel_splits, guesstimate_ticks, round_to_next_tick, round_to_tick};
pub use time::{
    format_duration_ms, ok_or_default, reset_to_start_of_day_utc, reset_to_start_of_month_utc,
    reset_to_start_of_year_utc,
};
