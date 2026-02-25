//! Trade analysis tools: get_trades, get_volume_profile,
//! get_delta_profile, get_aggregated_trades

use data::domain::assistant::ChartSnapshot;
use serde_json::{Value, json};

use super::{ToolExecResult, parse_time_range, tick_multiplier};

pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "get_trades",
                "description": "Get trade data aggregated by price \
                    level. Returns buy/sell volume per price. Supports \
                    time and price filters. Max 100 levels.",
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
                        "price_min": {
                            "type": "number",
                            "description": "Minimum price filter"
                        },
                        "price_max": {
                            "type": "number",
                            "description": "Maximum price filter"
                        },
                        "count": {
                            "type": "integer",
                            "description": "Max price levels \
                                (default 100)"
                        }
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "get_volume_profile",
                "description": "Get volume-at-price profile with POC, \
                    value area high/low. Supports time filtering.",
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
                        }
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "get_delta_profile",
                "description": "Get delta (buy minus sell) volume by \
                    price level. Shows buying/selling pressure at each \
                    price. Supports time and price filters.",
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
                        "price_min": {
                            "type": "number",
                            "description": "Minimum price filter"
                        },
                        "price_max": {
                            "type": "number",
                            "description": "Maximum price filter"
                        }
                    },
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "get_aggregated_trades",
                "description": "Get time-bucketed volume/delta \
                    aggregation from trade data. Groups trades into \
                    time buckets with buy/sell/delta/vwap per bucket.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "bucket_seconds": {
                            "type": "integer",
                            "description": "Bucket size in seconds \
                                (min 10, max 3600, default 60)"
                        },
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
                            "description": "Max buckets to return \
                                (max 200, default 100)"
                        }
                    },
                    "additionalProperties": false
                }
            }
        }),
    ]
}

pub fn exec_get_trades(
    snap: &ChartSnapshot,
    args: &Value,
) -> ToolExecResult {
    let max_levels =
        args["count"].as_u64().unwrap_or(100).min(100) as usize;
    let price_min = args["price_min"].as_f64();
    let price_max = args["price_max"].as_f64();
    let (start_ms, end_ms) = parse_time_range(args);

    let mut levels: std::collections::BTreeMap<i64, (f64, f64, u32)> =
        std::collections::BTreeMap::new();
    let tick_mult = tick_multiplier(snap.tick_size);

    for trade in &snap.trades {
        if let Some(s) = start_ms {
            if trade.time.0 < s {
                continue;
            }
        }
        if let Some(e) = end_ms {
            if trade.time.0 > e {
                continue;
            }
        }
        let price_f64 = trade.price.to_f64();
        if price_min.is_some_and(|min| price_f64 < min) {
            continue;
        }
        if price_max.is_some_and(|max| price_f64 > max) {
            continue;
        }
        let key = (price_f64 * tick_mult as f64).round() as i64;
        let entry = levels.entry(key).or_insert((0.0, 0.0, 0));
        let qty = trade.quantity.0;
        if trade.is_buy() {
            entry.0 += qty;
        } else {
            entry.1 += qty;
        }
        entry.2 += 1;
    }

    let mut level_vec: Vec<(i64, f64, f64, u32)> = levels
        .into_iter()
        .map(|(k, (buy, sell, count))| (k, buy, sell, count))
        .collect();
    level_vec.sort_by(|a, b| {
        (b.1 + b.2)
            .partial_cmp(&(a.1 + a.2))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    level_vec.truncate(max_levels);
    level_vec.sort_by_key(|l| l.0);

    let rows: Vec<Value> = level_vec
        .iter()
        .map(|(key, buy, sell, count)| {
            let price = *key as f64 / tick_mult as f64;
            json!({
                "price": price,
                "buy_qty": buy,
                "sell_qty": sell,
                "count": count,
            })
        })
        .collect();

    ToolExecResult {
        content_json: json!({ "levels": rows }).to_string(),
        display_summary: format!("{} price levels", rows.len()),
        is_error: false,
    }
}

pub fn exec_get_volume_profile(
    snap: &ChartSnapshot,
    args: &Value,
) -> ToolExecResult {
    let candles = &snap.candles;
    if candles.is_empty() {
        return ToolExecResult {
            content_json: json!({ "error": "No candle data" })
                .to_string(),
            display_summary: "No data".to_string(),
            is_error: true,
        };
    }

    let (start_ms, end_ms) = parse_time_range(args);
    let tick_mult = tick_multiplier(snap.tick_size);

    let mut levels: std::collections::BTreeMap<i64, (f64, f64)> =
        std::collections::BTreeMap::new();

    let filtered_trades: Vec<&data::Trade> = snap
        .trades
        .iter()
        .filter(|t| {
            if let Some(s) = start_ms {
                if t.time.0 < s {
                    return false;
                }
            }
            if let Some(e) = end_ms {
                if t.time.0 > e {
                    return false;
                }
            }
            true
        })
        .collect();

    if !filtered_trades.is_empty() {
        for trade in &filtered_trades {
            let key =
                (trade.price.to_f64() * tick_mult as f64).round()
                    as i64;
            let entry = levels.entry(key).or_insert((0.0, 0.0));
            let qty = trade.quantity.0;
            if trade.is_buy() {
                entry.0 += qty;
            } else {
                entry.1 += qty;
            }
        }
    } else {
        let filtered_candles: Vec<_> = candles
            .iter()
            .filter(|c| {
                if let Some(s) = start_ms {
                    if c.time.0 < s {
                        return false;
                    }
                }
                if let Some(e) = end_ms {
                    if c.time.0 > e {
                        return false;
                    }
                }
                true
            })
            .collect();

        for c in &filtered_candles {
            let low_key =
                (c.low.to_f64() * tick_mult as f64).round() as i64;
            let high_key =
                (c.high.to_f64() * tick_mult as f64).round() as i64;
            let num_levels = (high_key - low_key).max(1) as f64;
            let buy_per = c.buy_volume.0 / num_levels;
            let sell_per = c.sell_volume.0 / num_levels;
            for key in low_key..=high_key {
                let entry =
                    levels.entry(key).or_insert((0.0, 0.0));
                entry.0 += buy_per;
                entry.1 += sell_per;
            }
        }
    }

    let total_volume: f64 =
        levels.values().map(|(b, s)| b + s).sum();

    let poc_key = levels
        .iter()
        .max_by(|a, b| {
            let va = a.1 .0 + a.1 .1;
            let vb = b.1 .0 + b.1 .1;
            va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(k, _)| *k)
        .unwrap_or(0);

    // Value area: 68.2% of volume around POC (1-sigma)
    let va_target = total_volume * 0.682;
    let mut sorted_by_vol: Vec<(i64, f64)> = levels
        .iter()
        .map(|(k, (b, s))| (*k, b + s))
        .collect();
    sorted_by_vol.sort_by(|a, b| {
        b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut va_volume = 0.0;
    let mut va_prices: Vec<i64> = Vec::new();
    for (key, vol) in &sorted_by_vol {
        if va_volume >= va_target {
            break;
        }
        va_prices.push(*key);
        va_volume += vol;
    }
    let va_high = va_prices.iter().max().copied().unwrap_or(poc_key);
    let va_low = va_prices.iter().min().copied().unwrap_or(poc_key);

    sorted_by_vol.truncate(50);
    let mut output_keys: Vec<i64> =
        sorted_by_vol.iter().map(|(k, _)| *k).collect();
    output_keys.sort();

    let rows: Vec<Value> = output_keys
        .iter()
        .filter_map(|key| {
            let (buy, sell) = levels.get(key)?;
            let total = buy + sell;
            let pct = if total_volume > 0.0 {
                total / total_volume * 100.0
            } else {
                0.0
            };
            let price = *key as f64 / tick_mult as f64;
            Some(json!({
                "price": price,
                "buy_vol": buy,
                "sell_vol": sell,
                "total": total,
                "pct": (pct * 100.0).round() / 100.0,
            }))
        })
        .collect();

    let poc_price = poc_key as f64 / tick_mult as f64;
    let vah_price = va_high as f64 / tick_mult as f64;
    let val_price = va_low as f64 / tick_mult as f64;

    let result = json!({
        "levels": rows,
        "poc_price": poc_price,
        "value_area_high": vah_price,
        "value_area_low": val_price,
        "total_volume": total_volume,
    });

    ToolExecResult {
        content_json: result.to_string(),
        display_summary: format!(
            "POC {:.2} | VAH {:.2} VAL {:.2}",
            poc_price, vah_price, val_price
        ),
        is_error: false,
    }
}

pub fn exec_get_delta_profile(
    snap: &ChartSnapshot,
    args: &Value,
) -> ToolExecResult {
    let price_min = args["price_min"].as_f64();
    let price_max = args["price_max"].as_f64();
    let (start_ms, end_ms) = parse_time_range(args);

    if snap.trades.is_empty() {
        return ToolExecResult {
            content_json: json!({ "error": "No trade data" })
                .to_string(),
            display_summary: "No data".to_string(),
            is_error: true,
        };
    }

    let tick_mult = tick_multiplier(snap.tick_size);
    let mut levels: std::collections::BTreeMap<i64, (f64, f64)> =
        std::collections::BTreeMap::new();

    for trade in &snap.trades {
        if let Some(s) = start_ms {
            if trade.time.0 < s {
                continue;
            }
        }
        if let Some(e) = end_ms {
            if trade.time.0 > e {
                continue;
            }
        }
        let price_f64 = trade.price.to_f64();
        if price_min.is_some_and(|min| price_f64 < min) {
            continue;
        }
        if price_max.is_some_and(|max| price_f64 > max) {
            continue;
        }
        let key = (price_f64 * tick_mult as f64).round() as i64;
        let entry = levels.entry(key).or_insert((0.0, 0.0));
        let qty = trade.quantity.0;
        if trade.is_buy() {
            entry.0 += qty;
        } else {
            entry.1 += qty;
        }
    }

    let total_buy: f64 = levels.values().map(|(b, _)| b).sum();
    let total_sell: f64 = levels.values().map(|(_, s)| s).sum();
    let total_delta = total_buy - total_sell;

    let mut level_vec: Vec<(i64, f64, f64)> = levels
        .into_iter()
        .map(|(k, (buy, sell))| (k, buy, sell))
        .collect();
    level_vec.sort_by(|a, b| {
        let da = (a.1 - a.2).abs();
        let db = (b.1 - b.2).abs();
        db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
    });
    level_vec.truncate(100);
    level_vec.sort_by_key(|l| l.0);

    let rows: Vec<Value> = level_vec
        .iter()
        .map(|(key, buy, sell)| {
            let price = *key as f64 / tick_mult as f64;
            let delta = buy - sell;
            let total = buy + sell;
            let delta_pct = if total > 0.0 {
                (delta / total * 100.0 * 10.0).round() / 10.0
            } else {
                0.0
            };
            json!({
                "price": price,
                "buy_vol": buy,
                "sell_vol": sell,
                "delta": delta,
                "delta_pct": delta_pct,
            })
        })
        .collect();

    let result = json!({
        "levels": rows,
        "total_buy": total_buy,
        "total_sell": total_sell,
        "total_delta": total_delta,
    });

    ToolExecResult {
        content_json: result.to_string(),
        display_summary: format!(
            "Delta profile: {} levels, net delta {:.0}",
            rows.len(),
            total_delta
        ),
        is_error: false,
    }
}

pub fn exec_get_aggregated_trades(
    snap: &ChartSnapshot,
    args: &Value,
) -> ToolExecResult {
    let bucket_secs = args["bucket_seconds"]
        .as_u64()
        .unwrap_or(60)
        .clamp(10, 3600);
    let max_buckets =
        args["count"].as_u64().unwrap_or(100).min(200) as usize;
    let (start_ms, end_ms) = parse_time_range(args);

    if snap.trades.is_empty() {
        return ToolExecResult {
            content_json: json!({ "error": "No trade data" })
                .to_string(),
            display_summary: "No data".to_string(),
            is_error: true,
        };
    }

    let bucket_ms = bucket_secs * 1_000;

    // Accumulate into buckets: (buy_vol, sell_vol, count,
    //   price*qty sum, qty sum for vwap)
    let mut buckets: std::collections::BTreeMap<
        u64,
        (f64, f64, u32, f64, f64),
    > = std::collections::BTreeMap::new();

    for trade in &snap.trades {
        if let Some(s) = start_ms {
            if trade.time.0 < s {
                continue;
            }
        }
        if let Some(e) = end_ms {
            if trade.time.0 > e {
                continue;
            }
        }
        let bucket_key = (trade.time.0 / bucket_ms) * bucket_ms;
        let entry =
            buckets.entry(bucket_key).or_insert((0.0, 0.0, 0, 0.0, 0.0));
        let qty = trade.quantity.0;
        let price = trade.price.to_f64();
        if trade.is_buy() {
            entry.0 += qty;
        } else {
            entry.1 += qty;
        }
        entry.2 += 1;
        entry.3 += price * qty;
        entry.4 += qty;
    }

    // Take last N buckets
    let total = buckets.len();
    let skip = total.saturating_sub(max_buckets);

    let rows: Vec<Value> = buckets
        .iter()
        .skip(skip)
        .map(|(key, (buy, sell, count, pq_sum, q_sum))| {
            let time_secs = key / 1_000;
            let vwap = if *q_sum > 0.0 {
                pq_sum / q_sum
            } else {
                0.0
            };
            json!({
                "time": time_secs,
                "buy_vol": buy,
                "sell_vol": sell,
                "delta": buy - sell,
                "count": count,
                "vwap": (vwap * 100.0).round() / 100.0,
            })
        })
        .collect();

    ToolExecResult {
        content_json: json!({ "buckets": rows }).to_string(),
        display_summary: format!(
            "{} buckets ({}s each)",
            rows.len(),
            bucket_secs
        ),
        is_error: false,
    }
}
