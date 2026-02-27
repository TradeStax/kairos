//! Level detection engine.
//!
//! Detects price levels from multiple sources: volume profile
//! (HVN/LVN/POC/VA), session boundaries, prior-day levels, delta
//! zones, and opening range. Supports both per-session and
//! aggregate detection modes.

use std::collections::HashMap;

use data::{Candle, Price, Side, Trade};

use super::session::{self, SessionInfo, SessionKey, SessionType};
use super::types::{LevelSource, MonitoredLevel};
use crate::config::StudyConfig;
use crate::output::NodeDetectionMethod;
use crate::studies::orderflow::vbp::profile_core;

/// Raw detected level before deduplication.
struct RawLevel {
    price_units: i64,
    source: LevelSource,
    session_key: SessionKey,
    /// Timestamp when this level was detected (session close_time
    /// for completed sessions, 0 for active/aggregate levels).
    detected_at: u64,
}

// ── Aggregate mode (backward-compatible) ────────────────────────

/// Run the full detection pipeline in aggregate mode and return
/// deduplicated levels. Builds a single profile from ALL data.
pub fn detect_levels(
    candles: &[Candle],
    trades: Option<&[Trade]>,
    tick_size: Price,
    config: &StudyConfig,
    existing_levels: &[MonitoredLevel],
    next_id: &mut u64,
) -> Vec<MonitoredLevel> {
    if candles.is_empty() {
        return Vec::new();
    }

    let last_candle_time =
        candles.last().map_or(0, |c| c.time.0);
    let session_key = SessionKey {
        trade_date: format!(
            "{}",
            last_candle_time / 86_400_000
        ),
        session_type: SessionType::Rth,
    };

    let mut raw_levels: Vec<RawLevel> = Vec::new();

    // Volume profile levels
    detect_profile_levels(
        candles,
        trades,
        tick_size,
        config,
        &session_key,
        last_candle_time,
        &mut raw_levels,
    );

    // Session levels
    detect_session_and_prior_day(
        candles,
        config,
        &session_key,
        last_candle_time,
        &mut raw_levels,
    );

    // Delta zones
    if config.get_bool("enable_delta_zones", false) {
        if let Some(trades) = trades {
            let delta_threshold =
                config.get_float("delta_threshold", 2.0);
            detect_delta_zones(
                trades,
                tick_size,
                delta_threshold,
                &session_key,
                last_candle_time,
                &mut raw_levels,
            );
        }
    }

    // Deduplicate and build result
    let max_levels =
        config.get_int("max_levels", 30) as usize;
    dedup_and_build(
        raw_levels,
        existing_levels,
        tick_size,
        max_levels,
        next_id,
    )
}

// ── Per-session mode ────────────────────────────────────────────

/// Detect levels from each completed session independently.
pub fn detect_levels_per_session(
    candles: &[Candle],
    trades: Option<&[Trade]>,
    tick_size: Price,
    config: &StudyConfig,
    existing_levels: &[MonitoredLevel],
    next_id: &mut u64,
    sessions: &[SessionInfo],
    trade_ranges: &[(usize, usize)],
    visible_session_count: usize,
) -> Vec<MonitoredLevel> {
    if candles.is_empty() || sessions.is_empty() {
        return Vec::new();
    }

    let mut raw_levels: Vec<RawLevel> = Vec::new();

    // Select last N completed sessions
    let completed: Vec<usize> = sessions
        .iter()
        .enumerate()
        .filter(|(_, s)| s.is_complete)
        .map(|(i, _)| i)
        .collect();

    let start_idx = completed
        .len()
        .saturating_sub(visible_session_count);
    let visible_completed = &completed[start_idx..];

    // Detect levels for each completed session
    for &si in visible_completed {
        let session = &sessions[si];
        let trade_range = trade_ranges
            .get(si)
            .copied()
            .unwrap_or((0, 0));

        detect_levels_for_session(
            session,
            candles,
            trades,
            trade_range,
            tick_size,
            config,
            next_id,
            &mut raw_levels,
        );
    }

    // Cross-session levels
    detect_cross_session_levels(
        sessions,
        config,
        &mut raw_levels,
    );

    // Active session: only Session H/L, Opening Range, delta
    if let Some(active) = sessions.last() {
        if !active.is_complete {
            let cross_key = SessionKey::cross_session();

            if config.get_bool("enable_session_hl", true) {
                raw_levels.push(RawLevel {
                    price_units: active.high_units,
                    source: LevelSource::SessionHigh,
                    session_key: cross_key.clone(),
                    detected_at: 0,
                });
                raw_levels.push(RawLevel {
                    price_units: active.low_units,
                    source: LevelSource::SessionLow,
                    session_key: cross_key.clone(),
                    detected_at: 0,
                });
            }

            if config.get_bool("enable_opening_range", true)
                && active.key.session_type == SessionType::Rth
            {
                if let Some(orh) = active.or_high_units {
                    raw_levels.push(RawLevel {
                        price_units: orh,
                        source: LevelSource::OpeningRangeHigh,
                        session_key: cross_key.clone(),
                        detected_at: 0,
                    });
                }
                if let Some(orl) = active.or_low_units {
                    raw_levels.push(RawLevel {
                        price_units: orl,
                        source: LevelSource::OpeningRangeLow,
                        session_key: cross_key.clone(),
                        detected_at: 0,
                    });
                }
            }

            // Delta zones from active session
            if config.get_bool("enable_delta_zones", false) {
                if let Some(trades) = trades {
                    let tr = trade_ranges
                        .last()
                        .copied()
                        .unwrap_or((0, 0));
                    if tr.1 > tr.0 {
                        let delta_threshold = config
                            .get_float("delta_threshold", 2.0);
                        detect_delta_zones(
                            &trades[tr.0..tr.1],
                            tick_size,
                            delta_threshold,
                            &cross_key,
                            0,
                            &mut raw_levels,
                        );
                    }
                }
            }
        }
    }

    // Deduplicate within each session (NOT cross-session)
    let max_levels =
        config.get_int("max_levels", 30) as usize;
    dedup_per_session_and_build(
        raw_levels,
        existing_levels,
        tick_size,
        max_levels,
        next_id,
    )
}

/// Detect levels from a single completed session's data.
fn detect_levels_for_session(
    session: &SessionInfo,
    candles: &[Candle],
    trades: Option<&[Trade]>,
    trade_range: (usize, usize),
    tick_size: Price,
    config: &StudyConfig,
    next_id: &mut u64,
    out: &mut Vec<RawLevel>,
) {
    let (cs, ce) = session.candle_range;
    if ce < cs || cs >= candles.len() {
        return;
    }
    let session_candles =
        &candles[cs..=(ce.min(candles.len() - 1))];
    if session_candles.is_empty() {
        return;
    }

    let session_trades = trades.and_then(|t| {
        let (ts, te) = trade_range;
        if te > ts && te <= t.len() {
            Some(&t[ts..te])
        } else {
            None
        }
    });

    let anchor = session.close_time;

    // Profile levels for this session
    detect_profile_levels(
        session_candles,
        session_trades,
        tick_size,
        config,
        &session.key,
        anchor,
        out,
    );

    // Session high/low for completed sessions
    if config.get_bool("enable_session_hl", true) {
        out.push(RawLevel {
            price_units: session.high_units,
            source: LevelSource::SessionHigh,
            session_key: session.key.clone(),
            detected_at: anchor,
        });
        out.push(RawLevel {
            price_units: session.low_units,
            source: LevelSource::SessionLow,
            session_key: session.key.clone(),
            detected_at: anchor,
        });
    }

    // Delta zones for this session
    if config.get_bool("enable_delta_zones", false) {
        if let Some(t) = session_trades {
            let delta_threshold =
                config.get_float("delta_threshold", 2.0);
            detect_delta_zones(
                t,
                tick_size,
                delta_threshold,
                &session.key,
                anchor,
                out,
            );
        }
    }

    let _ = next_id; // IDs assigned during build phase
}

/// Detect cross-session levels: prior day H/L/C, opening range.
fn detect_cross_session_levels(
    sessions: &[SessionInfo],
    config: &StudyConfig,
    out: &mut Vec<RawLevel>,
) {
    let cross_key = SessionKey::cross_session();

    // Find last completed RTH session for prior day levels
    if config.get_bool("enable_prior_day", true) {
        let completed_rth: Vec<&SessionInfo> = sessions
            .iter()
            .filter(|s| {
                s.is_complete
                    && s.key.session_type == SessionType::Rth
            })
            .collect();

        if let Some(prior) = completed_rth.last() {
            let anchor = prior.close_time;
            out.push(RawLevel {
                price_units: prior.high_units,
                source: LevelSource::PriorDayHigh,
                session_key: cross_key.clone(),
                detected_at: anchor,
            });
            out.push(RawLevel {
                price_units: prior.low_units,
                source: LevelSource::PriorDayLow,
                session_key: cross_key.clone(),
                detected_at: anchor,
            });
            out.push(RawLevel {
                price_units: prior.close_units,
                source: LevelSource::PriorDayClose,
                session_key: cross_key.clone(),
                detected_at: anchor,
            });
        }
    }
}

// ── Shared detection helpers ────────────────────────────────────

/// Detect volume profile levels (HVN, LVN, POC, VAH/VAL) from a
/// candle/trade slice.
fn detect_profile_levels(
    candles: &[Candle],
    trades: Option<&[Trade]>,
    tick_size: Price,
    config: &StudyConfig,
    session_key: &SessionKey,
    detected_at: u64,
    out: &mut Vec<RawLevel>,
) {
    let enable_hvn = config.get_bool("enable_hvn", true);
    let enable_lvn = config.get_bool("enable_lvn", true);
    let enable_poc = config.get_bool("enable_poc", true);
    let enable_vah_val =
        config.get_bool("enable_vah_val", true);

    if !(enable_hvn || enable_lvn || enable_poc || enable_vah_val)
    {
        return;
    }

    let profile = match trades {
        Some(t) if !t.is_empty() => {
            profile_core::build_profile_from_trades(
                t,
                tick_size,
                tick_size.units(),
            )
        }
        _ => profile_core::build_profile_from_candles(
            candles,
            tick_size,
            tick_size.units(),
        ),
    };

    if profile.is_empty() {
        return;
    }

    let poc_idx = profile_core::find_poc_index(&profile);

    // POC
    if let Some(poc_idx) = poc_idx {
        if enable_poc {
            out.push(RawLevel {
                price_units: profile[poc_idx].price_units,
                source: LevelSource::Poc,
                session_key: session_key.clone(),
                detected_at,
            });
        }

        // Value area
        if enable_vah_val {
            let va_pct =
                config.get_float("va_percentage", 0.7);
            if let Some((vah_idx, val_idx)) =
                profile_core::calculate_value_area(
                    &profile, poc_idx, va_pct,
                )
            {
                out.push(RawLevel {
                    price_units: profile[vah_idx].price_units,
                    source: LevelSource::Vah,
                    session_key: session_key.clone(),
                    detected_at,
                });
                out.push(RawLevel {
                    price_units: profile[val_idx].price_units,
                    source: LevelSource::Val,
                    session_key: session_key.clone(),
                    detected_at,
                });
            }
        }
    }

    // HVN / LVN with configurable thresholds + POC exclusion
    if enable_hvn || enable_lvn {
        let hvn_threshold =
            config.get_float("hvn_threshold", 1.5) as f32;
        let hvn_prominence =
            config.get_float("hvn_min_prominence", 0.25) as f32;
        let poc_exclusion_ticks =
            config.get_int("hvn_poc_exclusion", 10);
        let poc_exclusion_units =
            poc_exclusion_ticks * tick_size.units();

        let poc_price_units = poc_idx
            .map(|i| profile[i].price_units);

        let (hvn_nodes, lvn_nodes) =
            profile_core::detect_volume_nodes(
                &profile,
                NodeDetectionMethod::StdDev,
                hvn_threshold,
                NodeDetectionMethod::StdDev,
                -0.5,
                hvn_prominence,
            );

        if enable_hvn {
            for node in &hvn_nodes {
                // Skip HVNs within exclusion zone of POC
                if let Some(poc_pu) = poc_price_units {
                    let dist = (node.price_units - poc_pu)
                        .abs();
                    if dist <= poc_exclusion_units {
                        continue;
                    }
                }
                out.push(RawLevel {
                    price_units: node.price_units,
                    source: LevelSource::Hvn,
                    session_key: session_key.clone(),
                    detected_at,
                });
            }
        }
        if enable_lvn {
            for node in &lvn_nodes {
                out.push(RawLevel {
                    price_units: node.price_units,
                    source: LevelSource::Lvn,
                    session_key: session_key.clone(),
                    detected_at,
                });
            }
        }
    }
}

/// Detect session H/L and prior-day levels from extracted sessions
/// (aggregate mode).
fn detect_session_and_prior_day(
    candles: &[Candle],
    config: &StudyConfig,
    session_key: &SessionKey,
    detected_at: u64,
    out: &mut Vec<RawLevel>,
) {
    let enable_session_hl =
        config.get_bool("enable_session_hl", true);
    let enable_prior_day =
        config.get_bool("enable_prior_day", true);
    let enable_opening_range =
        config.get_bool("enable_opening_range", true);
    let or_minutes =
        config.get_int("opening_range_minutes", 30) as u32;

    if !(enable_session_hl
        || enable_prior_day
        || enable_opening_range)
    {
        return;
    }

    let sessions =
        session::extract_sessions(candles, or_minutes);

    // Current session
    if let Some(current) = sessions.last() {
        if enable_session_hl {
            out.push(RawLevel {
                price_units: current.high_units,
                source: LevelSource::SessionHigh,
                session_key: session_key.clone(),
                detected_at,
            });
            out.push(RawLevel {
                price_units: current.low_units,
                source: LevelSource::SessionLow,
                session_key: session_key.clone(),
                detected_at,
            });
        }

        if enable_opening_range
            && current.key.session_type == SessionType::Rth
        {
            if let Some(orh) = current.or_high_units {
                out.push(RawLevel {
                    price_units: orh,
                    source: LevelSource::OpeningRangeHigh,
                    session_key: session_key.clone(),
                    detected_at,
                });
            }
            if let Some(orl) = current.or_low_units {
                out.push(RawLevel {
                    price_units: orl,
                    source: LevelSource::OpeningRangeLow,
                    session_key: session_key.clone(),
                    detected_at,
                });
            }
        }
    }

    // Prior day (from last completed RTH session)
    if enable_prior_day {
        let completed_rth: Vec<&SessionInfo> = sessions
            .iter()
            .filter(|s| {
                s.is_complete
                    && s.key.session_type == SessionType::Rth
            })
            .collect();

        if let Some(prior) = completed_rth.last() {
            let anchor = prior.close_time;
            out.push(RawLevel {
                price_units: prior.high_units,
                source: LevelSource::PriorDayHigh,
                session_key: session_key.clone(),
                detected_at: anchor,
            });
            out.push(RawLevel {
                price_units: prior.low_units,
                source: LevelSource::PriorDayLow,
                session_key: session_key.clone(),
                detected_at: anchor,
            });
            out.push(RawLevel {
                price_units: prior.close_units,
                source: LevelSource::PriorDayClose,
                session_key: session_key.clone(),
                detected_at: anchor,
            });
        }
    }
}

/// Detect price levels with extreme net delta.
fn detect_delta_zones(
    trades: &[Trade],
    tick_size: Price,
    threshold_std_devs: f64,
    session_key: &SessionKey,
    detected_at: u64,
    out: &mut Vec<RawLevel>,
) {
    if trades.is_empty() {
        return;
    }

    let step = tick_size.units();
    let mut delta_map: HashMap<i64, f64> = HashMap::new();

    for t in trades {
        let pu = (t.price.units() / step) * step;
        let signed_qty = match t.side {
            Side::Buy | Side::Ask => t.quantity.0 as f64,
            Side::Sell | Side::Bid => -(t.quantity.0 as f64),
        };
        *delta_map.entry(pu).or_default() += signed_qty;
    }

    if delta_map.is_empty() {
        return;
    }

    let deltas: Vec<f64> = delta_map.values().copied().collect();
    let n = deltas.len() as f64;
    let mean = deltas.iter().sum::<f64>() / n;
    let variance =
        deltas.iter().map(|d| (d - mean).powi(2)).sum::<f64>()
            / n;
    let std_dev = variance.sqrt();

    if std_dev < f64::EPSILON {
        return;
    }

    let high_cutoff = mean + threshold_std_devs * std_dev;
    let low_cutoff = mean - threshold_std_devs * std_dev;

    for (&pu, &delta) in &delta_map {
        if delta >= high_cutoff {
            out.push(RawLevel {
                price_units: pu,
                source: LevelSource::HighDeltaZone,
                session_key: session_key.clone(),
                detected_at,
            });
        } else if delta <= low_cutoff {
            out.push(RawLevel {
                price_units: pu,
                source: LevelSource::LowDeltaZone,
                session_key: session_key.clone(),
                detected_at,
            });
        }
    }
}

// ── Deduplication ───────────────────────────────────────────────

/// Deduplicate levels globally (aggregate mode) and build
/// `MonitoredLevel` instances.
fn dedup_and_build(
    mut raw_levels: Vec<RawLevel>,
    existing_levels: &[MonitoredLevel],
    tick_size: Price,
    max_levels: usize,
    next_id: &mut u64,
) -> Vec<MonitoredLevel> {
    let dedup_ticks = 2i64;
    let tick_units = tick_size.units();
    let dedup_range = dedup_ticks * tick_units;

    // Sort by priority descending
    raw_levels.sort_by(|a, b| {
        b.source.priority().cmp(&a.source.priority())
    });

    let mut used: Vec<i64> = Vec::new();
    let mut deduped: Vec<RawLevel> = Vec::new();

    for raw in raw_levels {
        let dominated = used.iter().any(|&existing| {
            (raw.price_units - existing).abs() <= dedup_range
        });
        if !dominated {
            used.push(raw.price_units);
            deduped.push(raw);
        }
    }

    build_monitored_levels(
        deduped,
        existing_levels,
        max_levels,
        next_id,
    )
}

/// Deduplicate levels within each session (per-session mode) and
/// build `MonitoredLevel` instances.
///
/// Cross-session dedup is NOT performed — a POC at the same price
/// from two different sessions are BOTH kept.
fn dedup_per_session_and_build(
    mut raw_levels: Vec<RawLevel>,
    existing_levels: &[MonitoredLevel],
    tick_size: Price,
    max_levels: usize,
    next_id: &mut u64,
) -> Vec<MonitoredLevel> {
    let dedup_ticks = 2i64;
    let tick_units = tick_size.units();
    let dedup_range = dedup_ticks * tick_units;

    // Sort by priority descending
    raw_levels.sort_by(|a, b| {
        b.source.priority().cmp(&a.source.priority())
    });

    // Group by session key and dedup within each group
    let mut deduped: Vec<RawLevel> = Vec::new();
    let mut session_used: HashMap<SessionKey, Vec<i64>> =
        HashMap::new();

    for raw in raw_levels {
        let used = session_used
            .entry(raw.session_key.clone())
            .or_default();

        let dominated = used.iter().any(|&existing| {
            (raw.price_units - existing).abs() <= dedup_range
        });
        if !dominated {
            used.push(raw.price_units);
            deduped.push(raw);
        }
    }

    build_monitored_levels(
        deduped,
        existing_levels,
        max_levels,
        next_id,
    )
}

/// Convert raw levels into `MonitoredLevel` instances, re-using
/// existing levels where possible to preserve monitoring state.
fn build_monitored_levels(
    deduped: Vec<RawLevel>,
    existing_levels: &[MonitoredLevel],
    max_levels: usize,
    next_id: &mut u64,
) -> Vec<MonitoredLevel> {
    // Build lookup: (price_units, source, session_key) -> existing
    let existing_by_key: HashMap<
        (i64, LevelSource, &SessionKey),
        &MonitoredLevel,
    > = existing_levels
        .iter()
        .map(|l| {
            ((l.price_units, l.source, &l.session_key), l)
        })
        .collect();

    // Preserve all manual levels
    let mut result: Vec<MonitoredLevel> = existing_levels
        .iter()
        .filter(|l| l.source == LevelSource::Manual)
        .cloned()
        .collect();

    for raw in deduped {
        if result.len() >= max_levels {
            break;
        }

        // Try to match existing level (price + source +
        // session_key)
        if let Some(existing) = existing_by_key.get(&(
            raw.price_units,
            raw.source,
            &raw.session_key,
        )) {
            result.push((*existing).clone());
        } else {
            let id = *next_id;
            *next_id += 1;
            let price =
                Price::from_units(raw.price_units).to_f64();
            result.push(MonitoredLevel::new(
                id,
                raw.price_units,
                price,
                raw.source,
                raw.detected_at,
                raw.session_key,
            ));
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::{Price, Quantity, Side, Timestamp, Volume};

    fn make_candle(
        time: u64,
        o: f64,
        h: f64,
        l: f64,
        c: f64,
    ) -> Candle {
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
    fn test_detect_levels_basic() {
        let tick = Price::from_f64(0.25);
        let candles = vec![
            make_candle(1_000_000, 100.0, 102.0, 99.0, 101.0),
            make_candle(2_000_000, 101.0, 103.0, 100.0, 102.0),
            make_candle(3_000_000, 102.0, 104.0, 101.0, 103.0),
        ];

        let config = StudyConfig::new("level_analyzer");
        let mut next_id = 1;

        let levels = detect_levels(
            &candles,
            None,
            tick,
            &config,
            &[],
            &mut next_id,
        );

        assert!(!levels.is_empty());
        let ids: Vec<u64> = levels.iter().map(|l| l.id).collect();
        let unique: std::collections::HashSet<u64> =
            ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn test_deduplication() {
        let tick = Price::from_f64(0.25);
        let candles = vec![make_candle(
            1_000_000, 100.0, 101.0, 99.0, 100.5,
        )];

        let mut config = StudyConfig::new("level_analyzer");
        config.set(
            "enable_lvn",
            crate::config::ParameterValue::Boolean(false),
        );
        config.set(
            "enable_vah_val",
            crate::config::ParameterValue::Boolean(false),
        );
        config.set(
            "enable_session_hl",
            crate::config::ParameterValue::Boolean(false),
        );
        config.set(
            "enable_prior_day",
            crate::config::ParameterValue::Boolean(false),
        );
        config.set(
            "enable_opening_range",
            crate::config::ParameterValue::Boolean(false),
        );

        let mut next_id = 1;
        let levels = detect_levels(
            &candles,
            None,
            tick,
            &config,
            &[],
            &mut next_id,
        );

        for (i, a) in levels.iter().enumerate() {
            for b in &levels[i + 1..] {
                let diff =
                    (a.price_units - b.price_units).abs();
                assert!(
                    diff > tick.units() * 2
                        || a.source != b.source,
                    "Duplicate levels within dedup range: \
                     {:?}@{} and {:?}@{}",
                    a.source,
                    a.price_units,
                    b.source,
                    b.price_units,
                );
            }
        }
    }

    #[test]
    fn test_delta_zones() {
        let tick = Price::from_f64(0.25);

        let mut trades = Vec::new();

        // Extreme positive delta at 100.0
        let p100 = Price::from_f64(100.0);
        for i in 0..100 {
            trades.push(Trade::new(
                Timestamp(i * 1000),
                p100,
                Quantity(10.0),
                Side::Buy,
            ));
        }
        for i in 0..10 {
            trades.push(Trade::new(
                Timestamp(100_000 + i * 1000),
                p100,
                Quantity(10.0),
                Side::Sell,
            ));
        }

        // Balanced at several other levels
        for level_idx in 1..6 {
            let p = Price::from_f64(
                100.0 + level_idx as f64 * 0.25,
            );
            for i in 0..30 {
                let base =
                    (level_idx * 100_000 + i * 1000) as u64;
                trades.push(Trade::new(
                    Timestamp(base),
                    p,
                    Quantity(10.0),
                    Side::Buy,
                ));
                trades.push(Trade::new(
                    Timestamp(base + 500),
                    p,
                    Quantity(10.0),
                    Side::Sell,
                ));
            }
        }

        let key = SessionKey {
            trade_date: "test".into(),
            session_type: SessionType::Rth,
        };
        let mut raw = Vec::new();
        detect_delta_zones(&trades, tick, 1.5, &key, 0, &mut raw);
        assert!(
            !raw.is_empty(),
            "Expected delta zones from skewed data"
        );
    }

    #[test]
    fn test_hvn_poc_exclusion() {
        // Create a profile where HVNs would cluster near POC
        let tick = Price::from_f64(0.25);
        let mut config = StudyConfig::new("level_analyzer");
        config.set(
            "enable_hvn",
            crate::config::ParameterValue::Boolean(true),
        );
        config.set(
            "enable_poc",
            crate::config::ParameterValue::Boolean(true),
        );
        config.set(
            "hvn_poc_exclusion",
            crate::config::ParameterValue::Integer(10),
        );
        config.set(
            "hvn_threshold",
            crate::config::ParameterValue::Float(1.0),
        );
        config.set(
            "hvn_min_prominence",
            crate::config::ParameterValue::Float(0.1),
        );

        // Build candles with high volume at center
        let mut candles = Vec::new();
        for i in 0..50 {
            candles.push(make_candle(
                i * 60_000,
                100.0,
                101.0,
                99.0,
                100.5,
            ));
        }

        let key = SessionKey {
            trade_date: "test".into(),
            session_type: SessionType::Rth,
        };
        let mut out = Vec::new();
        detect_profile_levels(
            &candles, None, tick, &config, &key, 0, &mut out,
        );

        // Check that no HVN is within 10 ticks of any POC
        let pocs: Vec<i64> = out
            .iter()
            .filter(|r| r.source == LevelSource::Poc)
            .map(|r| r.price_units)
            .collect();
        let hvns: Vec<i64> = out
            .iter()
            .filter(|r| r.source == LevelSource::Hvn)
            .map(|r| r.price_units)
            .collect();

        let exclusion = 10 * tick.units();
        for hvn in &hvns {
            for poc in &pocs {
                assert!(
                    (*hvn - *poc).abs() > exclusion,
                    "HVN at {} is within exclusion zone \
                     of POC at {}",
                    hvn,
                    poc,
                );
            }
        }
    }

    #[test]
    fn test_per_session_no_cross_dedup() {
        // Two sessions with POC at same price should both be kept
        let key1 = SessionKey {
            trade_date: "2024-02-25".into(),
            session_type: SessionType::Rth,
        };
        let key2 = SessionKey {
            trade_date: "2024-02-26".into(),
            session_type: SessionType::Rth,
        };

        let raw = vec![
            RawLevel {
                price_units: 10000,
                source: LevelSource::Poc,
                session_key: key1.clone(),
                detected_at: 0,
            },
            RawLevel {
                price_units: 10000,
                source: LevelSource::Poc,
                session_key: key2.clone(),
                detected_at: 0,
            },
        ];

        let tick = Price::from_f64(0.25);
        let mut next_id = 1;
        let result = dedup_per_session_and_build(
            raw,
            &[],
            tick,
            30,
            &mut next_id,
        );

        // Both should be kept (different sessions)
        let pocs: Vec<_> = result
            .iter()
            .filter(|l| l.source == LevelSource::Poc)
            .collect();
        assert_eq!(pocs.len(), 2);
    }
}
