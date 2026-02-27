//! Shared utility functions used across the data crate.
//!
//! - [`formatting`] — number abbreviation, comma grouping, percentage change
//! - [`math`] — tick rounding, panel split distribution
//! - [`time`] — duration formatting, start-of-day/month/year resets
//! - [`serde`] — `ok_or_default` deserializer, default value functions for serde
//! - [`logging`] — log file path construction

pub mod formatting;
pub mod logging;
pub mod math;
pub mod serde;

pub mod time;

pub use formatting::{
    abbr_large_numbers, count_decimals, currency_abbr, format_with_commas, pct_change,
};
pub use math::{calc_panel_splits, guesstimate_ticks, round_to_next_tick, round_to_tick};
pub use serde::ok_or_default;
pub use serde::{
    default_hvn_threshold, default_line_width, default_lvn_threshold, default_max_profiles,
    default_one, default_opacity, default_poc_width, default_split_value, default_true,
    default_va_fill_opacity, default_va_pct, default_zone_opacity,
};
pub use time::{
    format_duration_ms, reset_to_start_of_day_utc, reset_to_start_of_month_utc,
    reset_to_start_of_year_utc,
};
