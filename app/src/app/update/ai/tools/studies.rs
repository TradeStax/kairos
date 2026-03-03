//! Study tools: get_study_values, get_big_trades, get_footprint,
//! get_profile_data

use data::domain::assistant::ChartSnapshot;
use serde_json::{Value, json};

use super::{ToolExecResult, parse_time_range};

pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "get_study_values",
                "description": "Get latest values from all active \
                    studies (RSI, SMA, EMA, VWAP, MACD, Bollinger, \
                    ATR, etc). Returns the most recent data points.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "count": {
                            "type": "integer",
                            "description": "Number of recent data \
                                points per study (max 50, default 10)"
                        }
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "get_big_trades",
                "description": "Get significant (institutional-size) \
                    trade markers from the Big Trades study. Returns \
                    time, price, quantity, and side. Requires Big \
                    Trades study to be active.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "count": {
                            "type": "integer",
                            "description": "Max trades to return \
                                (max 200, default 50)"
                        }
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "get_footprint",
                "description": "Get per-candle footprint trade \
                    distribution data. Shows buy/sell volume at each \
                    price level within each candle. Requires the \
                    Footprint study to be active.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "start_time": {
                            "type": "string",
                            "description": "ISO 8601 start time filter"
                        },
                        "end_time": {
                            "type": "string",
                            "description": "ISO 8601 end time filter"
                        },
                        "count": {
                            "type": "integer",
                            "description": "Max candles (max 50, \
                                default 20)"
                        }
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "get_profile_data",
                "description": "Get full VBP (Volume by Price) \
                    profile data including POC, value area, HVN/LVN \
                    zones. Requires VBP study to be active.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "max_levels": {
                            "type": "integer",
                            "description": "Max price levels to \
                                return (max 200, default 50)"
                        }
                    },
                    "additionalProperties": false
                }
            }
        }),
    ]
}

pub fn exec_get_study_values(snap: &ChartSnapshot, args: &Value) -> ToolExecResult {
    let count = args["count"].as_u64().unwrap_or(10).min(50) as usize;

    if snap.study_snapshots.is_empty() {
        return ToolExecResult {
            content_json: json!({
                "error": "No active studies. Ask the user to add \
                    indicators like RSI, SMA, VWAP, etc."
            })
            .to_string(),
            display_summary: "No active studies".to_string(),
            is_error: true,
        };
    }

    let studies: Vec<Value> = snap
        .study_snapshots
        .iter()
        .map(|s| {
            let mut obj = json!({
                "id": s.study_id,
                "name": s.study_name,
            });

            if !s.line_values.is_empty() {
                let lines: Vec<Value> = s
                    .line_values
                    .iter()
                    .map(|(label, points)| {
                        let n = points.len();
                        let start = n.saturating_sub(count);
                        let pts: Vec<Value> = points[start..]
                            .iter()
                            .map(|(t, v)| json!({"time": t, "value": v}))
                            .collect();
                        json!({ "label": label, "values": pts })
                    })
                    .collect();
                obj["lines"] = json!(lines);
            }

            if !s.bar_values.is_empty() {
                let bars: Vec<Value> = s
                    .bar_values
                    .iter()
                    .map(|(label, points)| {
                        let n = points.len();
                        let start = n.saturating_sub(count);
                        let pts: Vec<Value> = points[start..]
                            .iter()
                            .map(|(t, v)| json!({"time": t, "value": v}))
                            .collect();
                        json!({ "label": label, "values": pts })
                    })
                    .collect();
                obj["bars"] = json!(bars);
            }

            if !s.levels.is_empty() {
                let lvls: Vec<Value> = s
                    .levels
                    .iter()
                    .map(|(label, price)| json!({"label": label, "price": price}))
                    .collect();
                obj["levels"] = json!(lvls);
            }

            obj
        })
        .collect();

    let names: Vec<&str> = snap
        .study_snapshots
        .iter()
        .map(|s| s.study_name.as_str())
        .collect();
    let summary = format!("{} studies: {}", studies.len(), names.join(", "));

    ToolExecResult {
        content_json: json!({ "studies": studies }).to_string(),
        display_summary: summary,
        is_error: false,
    }
}

pub fn exec_get_big_trades(snap: &ChartSnapshot, args: &Value) -> ToolExecResult {
    let count = args["count"].as_u64().unwrap_or(50).min(200) as usize;

    if snap.big_trade_markers.is_empty() {
        return ToolExecResult {
            content_json: json!({
                "error": "No big trade data. The Big Trades study \
                    may not be active. Ask the user to enable it."
            })
            .to_string(),
            display_summary: "No big trades".to_string(),
            is_error: true,
        };
    }

    let n = snap.big_trade_markers.len();
    let start_idx = n.saturating_sub(count);
    let rows: Vec<Value> = snap.big_trade_markers[start_idx..]
        .iter()
        .map(|m| {
            json!({
                "time": m.time,
                "price": m.price,
                "quantity": m.quantity,
                "side": if m.is_buy { "buy" } else { "sell" },
            })
        })
        .collect();

    ToolExecResult {
        content_json: json!({ "big_trades": rows }).to_string(),
        display_summary: format!("{} big trades returned", rows.len()),
        is_error: false,
    }
}

pub fn exec_get_footprint(
    snap: &ChartSnapshot,
    args: &Value,
    tz: crate::config::UserTimezone,
) -> ToolExecResult {
    if snap.footprint_candles.is_empty() {
        return ToolExecResult {
            content_json: json!({
                "error": "No footprint data. The Footprint study \
                    may not be active. Ask the user to enable it."
            })
            .to_string(),
            display_summary: "No footprint data".to_string(),
            is_error: true,
        };
    }

    let count = args["count"].as_u64().unwrap_or(20).min(50) as usize;
    let (start_ms, end_ms) = parse_time_range(args, tz);

    let filtered: Vec<_> = snap
        .footprint_candles
        .iter()
        .filter(|c| {
            if let Some(s) = start_ms
                && c.time_secs < s / 1_000
            {
                return false;
            }
            if let Some(e) = end_ms
                && c.time_secs > e / 1_000
            {
                return false;
            }
            true
        })
        .collect();

    let start_idx = filtered.len().saturating_sub(count);
    let slice = &filtered[start_idx..];

    let rows: Vec<Value> = slice
        .iter()
        .map(|c| {
            let levels: Vec<Value> = c
                .levels
                .iter()
                .map(|l| {
                    json!({
                        "price": l.price,
                        "buy_vol": l.buy_volume,
                        "sell_vol": l.sell_volume,
                        "delta": l.buy_volume - l.sell_volume,
                    })
                })
                .collect();
            json!({
                "time": c.time_secs,
                "open": c.open,
                "high": c.high,
                "low": c.low,
                "close": c.close,
                "poc_price": c.poc_price,
                "levels": levels,
            })
        })
        .collect();

    ToolExecResult {
        content_json: json!({ "footprint_candles": rows }).to_string(),
        display_summary: format!("{} footprint candles", rows.len()),
        is_error: false,
    }
}

pub fn exec_get_profile_data(snap: &ChartSnapshot, args: &Value) -> ToolExecResult {
    if snap.profile_snapshots.is_empty() {
        return ToolExecResult {
            content_json: json!({
                "error": "No VBP profile data. The VBP study may \
                    not be active. Ask the user to enable it."
            })
            .to_string(),
            display_summary: "No profile data".to_string(),
            is_error: true,
        };
    }

    let max_levels = args["max_levels"].as_u64().unwrap_or(50).min(200) as usize;

    let profiles: Vec<Value> = snap
        .profile_snapshots
        .iter()
        .map(|p| {
            // Take top levels by total volume
            let mut sorted: Vec<_> = p
                .levels
                .iter()
                .map(|l| (l.price, l.buy_volume, l.sell_volume))
                .collect();
            sorted.sort_by(|a, b| {
                let va = a.1 + a.2;
                let vb = b.1 + b.2;
                vb.partial_cmp(&va).unwrap_or(std::cmp::Ordering::Equal)
            });
            sorted.truncate(max_levels);
            sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

            let levels: Vec<Value> = sorted
                .iter()
                .map(|(price, buy, sell)| {
                    json!({
                        "price": price,
                        "buy_vol": buy,
                        "sell_vol": sell,
                        "total": *buy + *sell,
                    })
                })
                .collect();

            json!({
                "levels": levels,
                "poc_price": p.poc_price,
                "value_area_high": p.value_area_high,
                "value_area_low": p.value_area_low,
                "total_volume": p.total_volume,
                "hvn_zones": p.hvn_prices,
                "lvn_zones": p.lvn_prices,
                "time_range": p.time_range,
            })
        })
        .collect();

    let total_profiles = profiles.len();
    ToolExecResult {
        content_json: json!({ "profiles": profiles }).to_string(),
        display_summary: format!("{} profile(s), {} levels", total_profiles, max_levels),
        is_error: false,
    }
}
