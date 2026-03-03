//! Analysis tools: get_drawings, get_session_stats, identify_levels

use data::domain::assistant::ChartSnapshot;
use serde_json::{Value, json};

use super::{ToolExecResult, tick_multiplier};

pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "get_drawings",
                "description": "List all user drawings on the chart: \
                    lines, rectangles, arrows, text labels, etc. \
                    Returns ID, type, points, and properties.",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "get_session_stats",
                "description": "Get RTH/ETH session statistics: \
                    session high/low/open/close, total volume/delta, \
                    opening range (first 30min), initial balance \
                    (first 60min). Times are US Eastern.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "session": {
                            "type": "string",
                            "enum": ["rth", "eth", "both"],
                            "description": "Which session to analyze \
                                (default: rth)"
                        }
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "identify_levels",
                "description": "Detect support/resistance levels \
                    using swing highs/lows, volume profile nodes, \
                    and round numbers.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "method": {
                            "type": "string",
                            "enum": [
                                "all", "swing", "volume", "round"
                            ],
                            "description": "Detection method \
                                (default: all)"
                        },
                        "lookback": {
                            "type": "integer",
                            "description": "Candle lookback count \
                                (default: all candles)"
                        }
                    },
                    "additionalProperties": false
                }
            }
        }),
    ]
}

pub fn exec_get_drawings(snap: &ChartSnapshot) -> ToolExecResult {
    if snap.drawing_snapshots.is_empty() {
        return ToolExecResult {
            content_json: json!({
                "drawings": [],
                "count": 0
            })
            .to_string(),
            display_summary: "No drawings".to_string(),
            is_error: false,
        };
    }

    let rows: Vec<Value> = snap
        .drawing_snapshots
        .iter()
        .map(|d| {
            let points: Vec<Value> = d
                .points
                .iter()
                .map(|p| {
                    json!({
                        "price": p.price,
                        "time": p.time_secs,
                    })
                })
                .collect();
            json!({
                "id": d.id,
                "type": d.tool_type,
                "points": points,
                "label": d.label,
                "visible": d.visible,
                "locked": d.locked,
            })
        })
        .collect();

    ToolExecResult {
        content_json: json!({
            "drawings": rows,
            "count": rows.len(),
        })
        .to_string(),
        display_summary: format!("{} drawings", rows.len()),
        is_error: false,
    }
}

pub fn exec_get_session_stats(snap: &ChartSnapshot, args: &Value) -> ToolExecResult {
    if snap.candles.is_empty() {
        return ToolExecResult {
            content_json: json!({ "error": "No candle data" }).to_string(),
            display_summary: "No data".to_string(),
            is_error: true,
        };
    }

    let session = args["session"].as_str().unwrap_or("rth").to_lowercase();

    // RTH: 09:30-16:00 ET, ETH: 18:00-09:30 ET
    // We approximate ET as UTC-5 (EST). DST handling would require
    // the chrono-tz crate which may not be available.
    // SAFETY: 5*3600 = 18000 seconds is a valid west offset (EST = UTC-5)
    let et_offset = chrono::FixedOffset::west_opt(5 * 3600).unwrap();

    let filtered: Vec<_> = snap
        .candles
        .iter()
        .filter(|c| {
            let secs = (c.time.0 / 1_000) as i64;
            let Some(dt) = chrono::DateTime::from_timestamp(secs, 0) else {
                return false;
            };
            let et = dt.with_timezone(&et_offset);
            let time = et.time();
            // SAFETY: 09:30 and 16:00 are valid times
            let rth_start = chrono::NaiveTime::from_hms_opt(9, 30, 0).unwrap();
            let rth_end = chrono::NaiveTime::from_hms_opt(16, 0, 0).unwrap();

            match session.as_str() {
                "rth" => time >= rth_start && time < rth_end,
                "eth" => time < rth_start || time >= rth_end,
                _ => true, // "both"
            }
        })
        .collect();

    if filtered.is_empty() {
        return ToolExecResult {
            content_json: json!({
                "error": format!(
                    "No candles found for {} session",
                    session
                )
            })
            .to_string(),
            display_summary: "No session data".to_string(),
            is_error: true,
        };
    }

    let session_open = filtered.first().unwrap().open.to_f64();
    let session_close = filtered.last().unwrap().close.to_f64();
    let session_high = filtered
        .iter()
        .map(|c| c.high.to_f64())
        .fold(f64::NEG_INFINITY, f64::max);
    let session_low = filtered
        .iter()
        .map(|c| c.low.to_f64())
        .fold(f64::INFINITY, f64::min);
    let total_volume: f64 = filtered.iter().map(|c| c.volume() as f64).sum();
    let total_delta: f64 = filtered
        .iter()
        .map(|c| c.buy_volume.0 - c.sell_volume.0)
        .sum();

    // Opening range (first 30 min) and Initial Balance (first 60
    // min) — only for RTH
    let mut opening_range = json!(null);
    let mut initial_balance = json!(null);

    if session == "rth" || session == "both" {
        let first_time_secs = filtered.first().unwrap().time.0 / 1_000;
        let or_end = first_time_secs + 30 * 60;
        let ib_end = first_time_secs + 60 * 60;

        let or_candles: Vec<_> = filtered
            .iter()
            .filter(|c| c.time.0 / 1_000 < or_end)
            .collect();
        if !or_candles.is_empty() {
            let or_high = or_candles
                .iter()
                .map(|c| c.high.to_f64())
                .fold(f64::NEG_INFINITY, f64::max);
            let or_low = or_candles
                .iter()
                .map(|c| c.low.to_f64())
                .fold(f64::INFINITY, f64::min);
            opening_range = json!({ "high": or_high, "low": or_low });
        }

        let ib_candles: Vec<_> = filtered
            .iter()
            .filter(|c| c.time.0 / 1_000 < ib_end)
            .collect();
        if !ib_candles.is_empty() {
            let ib_high = ib_candles
                .iter()
                .map(|c| c.high.to_f64())
                .fold(f64::NEG_INFINITY, f64::max);
            let ib_low = ib_candles
                .iter()
                .map(|c| c.low.to_f64())
                .fold(f64::INFINITY, f64::min);
            initial_balance = json!({ "high": ib_high, "low": ib_low });
        }
    }

    let result = json!({
        "session": session,
        "session_open": session_open,
        "session_close": session_close,
        "session_high": session_high,
        "session_low": session_low,
        "total_volume": total_volume,
        "total_delta": total_delta,
        "candle_count": filtered.len(),
        "opening_range": opening_range,
        "initial_balance": initial_balance,
    });

    ToolExecResult {
        content_json: result.to_string(),
        display_summary: format!(
            "{} session: H {:.2} L {:.2}",
            session.to_uppercase(),
            session_high,
            session_low
        ),
        is_error: false,
    }
}

pub fn exec_identify_levels(snap: &ChartSnapshot, args: &Value) -> ToolExecResult {
    let method = args["method"].as_str().unwrap_or("all").to_lowercase();
    let lookback = args["lookback"]
        .as_u64()
        .map(|v| v as usize)
        .unwrap_or(snap.candles.len());

    if snap.candles.is_empty() {
        return ToolExecResult {
            content_json: json!({ "error": "No candle data" }).to_string(),
            display_summary: "No data".to_string(),
            is_error: true,
        };
    }

    let candle_count = snap.candles.len();
    let start = candle_count.saturating_sub(lookback);
    let candles = &snap.candles[start..];

    let mut levels: Vec<Value> = Vec::new();

    // Swing highs/lows (5-candle window)
    if method == "all" || method == "swing" {
        let window = 5usize;
        if candles.len() >= window {
            for i in (window / 2)..(candles.len() - window / 2) {
                let lo = i.saturating_sub(window / 2);
                let hi = (i + window / 2 + 1).min(candles.len());
                let slice = &candles[lo..hi];

                // Swing high
                let is_high = slice
                    .iter()
                    .all(|c| c.high.to_f64() <= candles[i].high.to_f64());
                if is_high {
                    let price = candles[i].high.to_f64();
                    let time = candles[i].time.0 / 1_000;
                    levels.push(json!({
                        "price": price,
                        "type": "swing_high",
                        "strength": 1.0,
                        "last_tested_time": time,
                    }));
                }

                // Swing low
                let is_low = slice
                    .iter()
                    .all(|c| c.low.to_f64() >= candles[i].low.to_f64());
                if is_low {
                    let price = candles[i].low.to_f64();
                    let time = candles[i].time.0 / 1_000;
                    levels.push(json!({
                        "price": price,
                        "type": "swing_low",
                        "strength": 1.0,
                        "last_tested_time": time,
                    }));
                }
            }
        }
    }

    // Volume-based levels from profile data
    if method == "all" || method == "volume" {
        for profile in &snap.profile_snapshots {
            if let Some(poc) = profile.poc_price {
                levels.push(json!({
                    "price": poc,
                    "type": "poc",
                    "strength": 1.0,
                    "last_tested_time": null,
                }));
            }
            if let Some(vah) = profile.value_area_high {
                levels.push(json!({
                    "price": vah,
                    "type": "value_area_high",
                    "strength": 0.8,
                    "last_tested_time": null,
                }));
            }
            if let Some(val) = profile.value_area_low {
                levels.push(json!({
                    "price": val,
                    "type": "value_area_low",
                    "strength": 0.8,
                    "last_tested_time": null,
                }));
            }
            for hvn in &profile.hvn_prices {
                levels.push(json!({
                    "price": hvn,
                    "type": "hvn",
                    "strength": 0.7,
                    "last_tested_time": null,
                }));
            }
            for lvn in &profile.lvn_prices {
                levels.push(json!({
                    "price": lvn,
                    "type": "lvn",
                    "strength": 0.6,
                    "last_tested_time": null,
                }));
            }
        }
    }

    // Round number levels
    if method == "all" || method == "round" {
        let data_high = candles
            .iter()
            .map(|c| c.high.to_f64())
            .fold(f64::NEG_INFINITY, f64::max);
        let data_low = candles
            .iter()
            .map(|c| c.low.to_f64())
            .fold(f64::INFINITY, f64::min);
        let _tick_mult = tick_multiplier(snap.tick_size);

        // Find appropriate round number step
        let range = data_high - data_low;
        let step = if range > 1000.0 {
            100.0
        } else if range > 100.0 {
            10.0
        } else if range > 10.0 {
            1.0
        } else {
            // For instruments like ES: use 25 or 50 point levels
            let tick = if snap.tick_size > 0.0 {
                snap.tick_size as f64
            } else {
                0.01
            };
            (tick * 100.0).max(0.5)
        };

        let start_level = (data_low / step).floor() as i64 * step as i64;
        let end_level = (data_high / step).ceil() as i64 * step as i64;
        let mut price = start_level as f64;
        while price <= end_level as f64 {
            if price >= data_low && price <= data_high {
                levels.push(json!({
                    "price": price,
                    "type": "round_number",
                    "strength": 0.5,
                    "last_tested_time": null,
                }));
            }
            price += step;
        }
    }

    // Deduplicate nearby levels (within 1 tick)
    let tick = snap.tick_size as f64;
    if tick > 0.0 {
        levels.sort_by(|a, b| {
            let pa = a["price"].as_f64().unwrap_or(0.0);
            let pb = b["price"].as_f64().unwrap_or(0.0);
            pa.partial_cmp(&pb).unwrap_or(std::cmp::Ordering::Equal)
        });
        levels.dedup_by(|a, b| {
            let pa = a["price"].as_f64().unwrap_or(0.0);
            let pb = b["price"].as_f64().unwrap_or(0.0);
            (pa - pb).abs() < tick * 2.0
        });
    }

    ToolExecResult {
        content_json: json!({
            "levels": levels,
            "count": levels.len(),
            "method": method,
        })
        .to_string(),
        display_summary: format!("{} levels identified ({})", levels.len(), method),
        is_error: false,
    }
}
