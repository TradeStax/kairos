//! IVB level computation and StudyOutput conversion.

use crate::config::StudyConfig;
use crate::output::{PriceLevel, StudyOutput};
use data::SerializableColor;

/// Bias direction based on breakout rate comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bias {
    Bullish,
    Bearish,
    Neutral,
}

/// Current session's breakout state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BreakoutState {
    Forming,
    BrokeHigh,
    BrokeLow,
    BrokeBoth,
}

/// Entry intelligence derived from historical session analysis.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct EntryIntel {
    pub up_retest_rate: f64,
    pub down_retest_rate: f64,
    pub up_close_confirm_rate: f64,
    pub down_close_confirm_rate: f64,
    pub avg_time_to_max_above_hrs: f64,
    pub avg_time_to_max_below_hrs: f64,
}

/// Computed IVB levels for the current session.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct IvbLevelSet {
    pub or_high: f64,
    pub or_low: f64,
    pub or_mid: f64,
    pub bias: Bias,
    pub breakout_state: BreakoutState,

    pub up_protection: Option<f64>,
    pub up_average: Option<f64>,
    pub up_projection: Option<f64>,

    pub down_protection: Option<f64>,
    pub down_average: Option<f64>,
    pub down_projection: Option<f64>,

    pub up_sample_count: usize,
    pub down_sample_count: usize,
    pub up_breakout_rate: f64,
    pub down_breakout_rate: f64,
    pub no_breakout_rate: f64,
    pub filters_applied: Vec<String>,

    pub entry_intel: Option<EntryIntel>,
    pub bias_label: String,
    pub down_partial_target: Option<f64>,
}

/// Compute bias from multiple signals.
pub fn compute_bias(
    or_close: f64,
    or_mid: f64,
    or_range: f64,
    high_formed_first: bool,
    up_breakout_rate: f64,
    down_breakout_rate: f64,
) -> Bias {
    if or_range <= 0.0 {
        return Bias::Neutral;
    }

    let mut score: f64 = 0.0;

    // Primary: OR close position relative to mid (weight 3)
    let close_offset = (or_close - or_mid) / or_range;
    if close_offset > 0.1 {
        score += 3.0;
    } else if close_offset < -0.1 {
        score -= 3.0;
    }

    // Secondary: high formed first = slightly bearish (weight 1)
    if high_formed_first {
        score -= 1.0;
    } else {
        score += 1.0;
    }

    // Tertiary: breakout rate differential (weight 2)
    let rate_diff = up_breakout_rate - down_breakout_rate;
    if rate_diff > 0.05 {
        score += 2.0;
    } else if rate_diff < -0.05 {
        score -= 2.0;
    }

    if score > 1.0 {
        Bias::Bullish
    } else if score < -1.0 {
        Bias::Bearish
    } else {
        Bias::Neutral
    }
}

/// RTH duration: 6.5 hours in milliseconds.
const RTH_DURATION_MS: u64 = 23_400_000;

/// Snap a price to the nearest tick boundary.
fn snap_to_tick(price: f64, tick_size_units: i64) -> f64 {
    if tick_size_units <= 0 {
        return price;
    }
    let tick_f64 = data::Price::from_units(tick_size_units).to_f64();
    if tick_f64 <= 0.0 {
        return price;
    }
    (price / tick_f64).round() * tick_f64
}

/// Determine decimal places for price formatting based on tick size.
fn price_decimals(tick_size_units: i64) -> usize {
    let tick_f64 = data::Price::from_units(tick_size_units).to_f64();
    if tick_f64 >= 1.0 {
        0
    } else if tick_f64 >= 0.1 {
        1
    } else if tick_f64 >= 0.01 {
        2
    } else if tick_f64 >= 0.001 {
        3
    } else {
        4
    }
}

/// Produce output for a completed historical session.
///
/// Shows OR High/Low levels and projected breakout levels
/// (protection, average, projection) at reduced opacity.
pub fn to_historical_output(
    session: &crate::util::session::SessionInfo,
    up_dist: Option<&dyn super::distributions::IvbDistribution>,
    down_dist: Option<&dyn super::distributions::IvbDistribution>,
    tick_size_units: i64,
    config: &StudyConfig,
) -> StudyOutput {
    let (Some(or_high), Some(or_low)) = (session.or_high_units, session.or_low_units) else {
        return StudyOutput::Empty;
    };
    let or_range = or_high - or_low;
    if or_range <= 0 {
        return StudyOutput::Empty;
    }

    let or_high_f64 = data::Price::from_units(or_high).to_f64();
    let or_low_f64 = data::Price::from_units(or_low).to_f64();
    let or_range_f64 = data::Price::from_units(or_range).to_f64();

    let rth_end_time = session.open_time + RTH_DURATION_MS;
    let decimals = price_decimals(tick_size_units);
    let level_width = config.get_float("level_width", 1.0) as f32;
    let level_style = crate::config::LineStyleValue::Dashed;

    let show_or = config.get_bool("show_or_range", true);
    let show_protection = config.get_bool("show_protection", true);
    let show_average = config.get_bool("show_average", true);
    let show_projection = config.get_bool("show_projection", true);
    let show_downside = config.get_bool("show_downside", true);
    let show_labels = config.get_bool("show_labels", true);

    let or_high_color =
        config.get_color("or_high_color", SerializableColor::new(0.2, 0.8, 0.2, 1.0));
    let or_low_color = config.get_color("or_low_color", SerializableColor::new(0.8, 0.2, 0.2, 1.0));
    let protection_color = config.get_color(
        "protection_color",
        SerializableColor::new(0.29, 0.56, 0.85, 1.0),
    );
    let average_color = config.get_color(
        "average_color",
        SerializableColor::new(0.83, 0.66, 0.26, 1.0),
    );
    let projection_color = config.get_color(
        "projection_color",
        SerializableColor::new(0.91, 0.49, 0.24, 1.0),
    );

    // Historical opacity multiplier — fainter than current
    let hist_op = 0.55_f32;

    let mut price_levels = Vec::new();

    // OR High / Low level lines
    if show_or {
        let mut pl = PriceLevel::horizontal(
            or_high_f64,
            format!("OR High {:.*}", decimals, or_high_f64),
            or_high_color,
        )
        .with_opacity(0.5 * hist_op)
        .with_width(level_width)
        .with_start_x(session.open_time)
        .with_end_x(rth_end_time);
        pl.show_label = show_labels;
        price_levels.push(pl);

        let mut pl = PriceLevel::horizontal(
            or_low_f64,
            format!("OR Low {:.*}", decimals, or_low_f64),
            or_low_color,
        )
        .with_opacity(0.5 * hist_op)
        .with_width(level_width)
        .with_start_x(session.open_time)
        .with_end_x(rth_end_time);
        pl.show_label = show_labels;
        price_levels.push(pl);
    }

    // Upside projected levels
    if let Some(d) = up_dist {
        if show_protection {
            let prot = snap_to_tick(or_high_f64 + d.protection() * or_range_f64, tick_size_units);
            let mut pl = PriceLevel::horizontal(
                prot,
                format!("Prot \u{2191} {:.*}", decimals, prot),
                protection_color,
            )
            .with_style(level_style)
            .with_opacity(0.5 * hist_op)
            .with_width(level_width)
            .with_start_x(session.open_time)
            .with_end_x(rth_end_time);
            pl.show_label = show_labels;
            price_levels.push(pl);
        }
        if show_average {
            let avg = snap_to_tick(or_high_f64 + d.average() * or_range_f64, tick_size_units);
            let mut pl = PriceLevel::horizontal(
                avg,
                format!("Avg \u{2191} {:.*}", decimals, avg),
                average_color,
            )
            .with_style(level_style)
            .with_opacity(0.5 * hist_op)
            .with_width(level_width)
            .with_start_x(session.open_time)
            .with_end_x(rth_end_time);
            pl.show_label = show_labels;
            price_levels.push(pl);
        }
        if show_projection {
            let proj = snap_to_tick(or_high_f64 + d.projection() * or_range_f64, tick_size_units);
            let mut pl = PriceLevel::horizontal(
                proj,
                format!("Proj \u{2191} {:.*}", decimals, proj,),
                projection_color,
            )
            .with_style(level_style)
            .with_opacity(0.4 * hist_op)
            .with_width(level_width)
            .with_start_x(session.open_time)
            .with_end_x(rth_end_time);
            pl.show_label = show_labels;
            price_levels.push(pl);
        }
    }

    // Downside projected levels
    if show_downside && let Some(d) = down_dist {
        if show_protection {
            let prot = snap_to_tick(or_low_f64 - d.protection() * or_range_f64, tick_size_units);
            let mut pl = PriceLevel::horizontal(
                prot,
                format!("Prot \u{2193} {:.*}", decimals, prot,),
                protection_color,
            )
            .with_style(level_style)
            .with_opacity(0.5 * hist_op)
            .with_width(level_width)
            .with_start_x(session.open_time)
            .with_end_x(rth_end_time);
            pl.show_label = show_labels;
            price_levels.push(pl);
        }
        if show_average {
            let avg = snap_to_tick(or_low_f64 - d.average() * or_range_f64, tick_size_units);
            let mut pl = PriceLevel::horizontal(
                avg,
                format!("Avg \u{2193} {:.*}", decimals, avg,),
                average_color,
            )
            .with_style(level_style)
            .with_opacity(0.5 * hist_op)
            .with_width(level_width)
            .with_start_x(session.open_time)
            .with_end_x(rth_end_time);
            pl.show_label = show_labels;
            price_levels.push(pl);
        }
        if show_projection {
            let proj = snap_to_tick(or_low_f64 - d.projection() * or_range_f64, tick_size_units);
            let mut pl = PriceLevel::horizontal(
                proj,
                format!("Proj \u{2193} {:.*}", decimals, proj,),
                projection_color,
            )
            .with_style(level_style)
            .with_opacity(0.4 * hist_op)
            .with_width(level_width)
            .with_start_x(session.open_time)
            .with_end_x(rth_end_time);
            pl.show_label = show_labels;
            price_levels.push(pl);
        }
    }

    if price_levels.is_empty() {
        StudyOutput::Empty
    } else {
        StudyOutput::Levels(price_levels)
    }
}

/// Convert IVB levels to StudyOutput.
pub fn to_study_output(
    levels: &IvbLevelSet,
    or_start_x: u64,
    tick_size_units: i64,
    config: &StudyConfig,
) -> StudyOutput {
    let show_or = config.get_bool("show_or_range", true);
    let show_protection = config.get_bool("show_protection", true);
    let show_average = config.get_bool("show_average", true);
    let show_projection = config.get_bool("show_projection", true);
    let show_labels = config.get_bool("show_labels", true);
    let show_stats = config.get_bool("show_stats_in_labels", true);
    let show_downside = config.get_bool("show_downside", true);
    let level_width = config.get_float("level_width", 1.0) as f32;

    let or_high_color =
        config.get_color("or_high_color", SerializableColor::new(0.2, 0.8, 0.2, 1.0));
    let or_low_color = config.get_color("or_low_color", SerializableColor::new(0.8, 0.2, 0.2, 1.0));
    let or_mid_color = config.get_color("or_mid_color", SerializableColor::new(0.5, 0.5, 0.5, 1.0));
    let protection_color = config.get_color(
        "protection_color",
        SerializableColor::new(0.29, 0.56, 0.85, 1.0),
    );
    let average_color = config.get_color(
        "average_color",
        SerializableColor::new(0.83, 0.66, 0.26, 1.0),
    );
    let projection_color = config.get_color(
        "projection_color",
        SerializableColor::new(0.91, 0.49, 0.24, 1.0),
    );

    let decimals = price_decimals(tick_size_units);
    let level_style = crate::config::LineStyleValue::Dashed;
    let rth_end_time = or_start_x + RTH_DURATION_MS;

    let (up_op, dn_op) = match levels.bias {
        Bias::Bullish => (1.0_f32, 0.6),
        Bias::Bearish => (0.6, 1.0),
        Bias::Neutral => (0.8, 0.8),
    };

    let mut price_levels = Vec::new();

    // OR High/Low
    if show_or {
        let mut pl = PriceLevel::horizontal(levels.or_high, "OR High", or_high_color)
            .with_opacity(0.8)
            .with_width(level_width)
            .with_start_x(or_start_x)
            .with_end_x(rth_end_time);
        pl.show_label = show_labels;
        price_levels.push(pl);

        let mut pl = PriceLevel::horizontal(levels.or_low, "OR Low", or_low_color)
            .with_opacity(0.8)
            .with_width(level_width)
            .with_start_x(or_start_x)
            .with_end_x(rth_end_time);
        pl.show_label = show_labels;
        price_levels.push(pl);

        // OR Mid as PriceLevel with Dotted style
        let mid_bias = match levels.bias {
            Bias::Bullish => "Bullish",
            Bias::Bearish => "Bearish",
            Bias::Neutral => "Neutral",
        };
        let mut pl =
            PriceLevel::horizontal(levels.or_mid, format!("OR Mid · {mid_bias}"), or_mid_color)
                .with_style(crate::config::LineStyleValue::Dotted)
                .with_opacity(0.6)
                .with_width(level_width * 0.75)
                .with_start_x(or_start_x)
                .with_end_x(rth_end_time);
        pl.show_label = show_labels;
        price_levels.push(pl);
    }

    // Upside levels
    if let Some(prot) = levels.up_protection
        && show_protection
    {
        let prot = snap_to_tick(prot, tick_size_units);
        let label = if show_stats {
            format!(
                "Prot \u{2191} {:.*} (n={}, {:.0}%)",
                decimals,
                prot,
                levels.up_sample_count,
                levels.up_breakout_rate * 100.0,
            )
        } else {
            format!("Prot \u{2191} {:.*}", decimals, prot)
        };
        let filters_str = if levels.filters_applied.is_empty() {
            "none".to_string()
        } else {
            levels.filters_applied.join(", ")
        };
        let tooltip = levels.entry_intel.as_ref().map(|ei| {
            format!(
                "Upside Protection (Median)\n\
                 Entry: on break (no retest wait)\n\
                 Close confirm: {:.0}% | \
                 Time to max: {:.1}h\n\
                 Samples: {} | \
                 Breakout: {:.0}%\n\
                 Filters: {}",
                ei.up_close_confirm_rate * 100.0,
                ei.avg_time_to_max_above_hrs,
                levels.up_sample_count,
                levels.up_breakout_rate * 100.0,
                filters_str,
            )
        });
        let mut pl = PriceLevel::horizontal(prot, label, protection_color)
            .with_style(level_style)
            .with_opacity(0.7 * up_op)
            .with_width(level_width)
            .with_start_x(or_start_x)
            .with_end_x(rth_end_time);
        pl.show_label = show_labels;
        pl.tooltip_data = tooltip;
        price_levels.push(pl);
    }
    if let Some(avg) = levels.up_average
        && show_average
    {
        let avg = snap_to_tick(avg, tick_size_units);
        let label = if show_stats {
            format!(
                "Avg \u{2191} {:.*} (n={})",
                decimals, avg, levels.up_sample_count,
            )
        } else {
            format!("Avg \u{2191} {:.*}", decimals, avg)
        };
        let mut pl = PriceLevel::horizontal(avg, label, average_color)
            .with_style(level_style)
            .with_opacity(0.7 * up_op)
            .with_width(level_width)
            .with_start_x(or_start_x)
            .with_end_x(rth_end_time);
        pl.show_label = show_labels;
        price_levels.push(pl);
    }
    if let Some(proj) = levels.up_projection
        && show_projection
    {
        let proj = snap_to_tick(proj, tick_size_units);
        let label = if show_stats {
            format!(
                "Proj \u{2191} {:.*} +1\u{03c3} (n={})",
                decimals, proj, levels.up_sample_count,
            )
        } else {
            format!("Proj \u{2191} {:.*}", decimals, proj)
        };
        let mut pl = PriceLevel::horizontal(proj, label, projection_color)
            .with_style(level_style)
            .with_opacity(0.6 * up_op)
            .with_width(level_width)
            .with_start_x(or_start_x)
            .with_end_x(rth_end_time);
        pl.show_label = show_labels;
        price_levels.push(pl);
    }
    // Downside levels
    if show_downside {
        if let Some(prot) = levels.down_protection
            && show_protection
        {
            let prot = snap_to_tick(prot, tick_size_units);
            let label = if show_stats {
                format!(
                    "Prot \u{2193} {:.*} \
                     (n={}, {:.0}%)",
                    decimals,
                    prot,
                    levels.down_sample_count,
                    levels.down_breakout_rate * 100.0,
                )
            } else {
                format!("Prot \u{2193} {:.*}", decimals, prot)
            };
            let filters_str = if levels.filters_applied.is_empty() {
                "none".to_string()
            } else {
                levels.filters_applied.join(", ")
            };
            let tooltip = levels.entry_intel.as_ref().map(|ei| {
                format!(
                    "Downside Protection (Median)\n\
                         Entry: {}\n\
                         Close confirm: {:.0}% | \
                         Time to max: {:.1}h\n\
                         Samples: {} | \
                         Breakout: {:.0}%\n\
                         Filters: {}",
                    if ei.down_retest_rate > 0.6 {
                        "wait retest"
                    } else {
                        "on break"
                    },
                    ei.down_close_confirm_rate * 100.0,
                    ei.avg_time_to_max_below_hrs,
                    levels.down_sample_count,
                    levels.down_breakout_rate * 100.0,
                    filters_str,
                )
            });
            let mut pl = PriceLevel::horizontal(prot, label, protection_color)
                .with_style(level_style)
                .with_opacity(0.7 * dn_op)
                .with_width(level_width)
                .with_start_x(or_start_x)
                .with_end_x(rth_end_time);
            pl.show_label = show_labels;
            pl.tooltip_data = tooltip;
            price_levels.push(pl);
        }

        // Downside partial target
        if let Some(partial) = levels.down_partial_target {
            let partial = snap_to_tick(partial, tick_size_units);
            let mut pl = PriceLevel::horizontal(
                partial,
                format!("Partial \u{2193} {:.*} (62%)", decimals, partial,),
                protection_color,
            )
            .with_style(crate::config::LineStyleValue::Dotted)
            .with_opacity(0.5 * dn_op)
            .with_width(level_width * 0.75)
            .with_start_x(or_start_x)
            .with_end_x(rth_end_time)
            .with_tooltip(format!(
                "Downside Partial Target (62.5%)\n\
                 Take partial — downside peaks fast \
                 but reverses\n\
                 {}",
                levels
                    .entry_intel
                    .as_ref()
                    .map(|ei| format!(
                        "Retest rate: {:.0}% | \
                         Time to max: {:.1}h",
                        ei.down_retest_rate * 100.0,
                        ei.avg_time_to_max_below_hrs,
                    ))
                    .unwrap_or_default(),
            ));
            pl.show_label = show_labels;
            price_levels.push(pl);
        }

        if let Some(avg) = levels.down_average
            && show_average
        {
            let avg = snap_to_tick(avg, tick_size_units);
            let label = if show_stats {
                format!(
                    "Avg \u{2193} {:.*} (n={})",
                    decimals, avg, levels.down_sample_count,
                )
            } else {
                format!("Avg \u{2193} {:.*}", decimals, avg)
            };
            let mut pl = PriceLevel::horizontal(avg, label, average_color)
                .with_style(level_style)
                .with_opacity(0.7 * dn_op)
                .with_width(level_width)
                .with_start_x(or_start_x)
                .with_end_x(rth_end_time);
            pl.show_label = show_labels;
            price_levels.push(pl);
        }
        if let Some(proj) = levels.down_projection
            && show_projection
        {
            let proj = snap_to_tick(proj, tick_size_units);
            let label = if show_stats {
                format!(
                    "Proj \u{2193} {:.*} +1\u{03c3} (n={})",
                    decimals, proj, levels.down_sample_count,
                )
            } else {
                format!("Proj \u{2193} {:.*}", decimals, proj)
            };
            let mut pl = PriceLevel::horizontal(proj, label, projection_color)
                .with_style(level_style)
                .with_opacity(0.6 * dn_op)
                .with_width(level_width)
                .with_start_x(or_start_x)
                .with_end_x(rth_end_time);
            pl.show_label = show_labels;
            price_levels.push(pl);
        }
    }

    if price_levels.is_empty() {
        StudyOutput::Empty
    } else {
        StudyOutput::Levels(price_levels)
    }
}
