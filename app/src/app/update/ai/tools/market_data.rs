//! Market data tools: get_chart_info, get_candles, get_market_state

use data::domain::assistant::ChartSnapshot;
use serde_json::{Value, json};

use super::{ToolExecResult, parse_time_range};

pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "get_chart_info",
                "description": "Get metadata about the linked chart: \
                    ticker, timeframe, chart type, candle count, trade \
                    count, active studies, date range, and live status.",
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
                "name": "get_candles",
                "description": "Get OHLCV+delta candle data. Defaults \
                    to last 50 candles, max 200. Supports time filtering.",
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
                            "description": "Number of candles \
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
                "name": "get_market_state",
                "description": "Get current market state: last price, \
                    session high/low, cumulative volume/delta, candle \
                    count, and the most recent candle.",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }
            }
        }),
    ]
}

pub fn exec_get_chart_info(snap: &ChartSnapshot) -> ToolExecResult {
    let result = json!({
        "ticker": snap.ticker,
        "timeframe": snap.timeframe,
        "chart_type": snap.chart_type,
        "candle_count": snap.candles.len(),
        "trade_count": snap.trades.len(),
        "trades_truncated": snap.trades_truncated,
        "active_studies": snap.active_studies,
        "date_range": snap.date_range_display,
        "is_live": snap.is_live,
        "is_tick_basis": snap.is_tick_basis,
        "tick_size": snap.tick_size,
        "contract_size": snap.contract_size,
        "drawing_count": snap.drawing_snapshots.len(),
        "has_footprint": !snap.footprint_candles.is_empty(),
        "has_profile": !snap.profile_snapshots.is_empty(),
    });
    ToolExecResult {
        content_json: result.to_string(),
        display_summary: format!(
            "{} {} | {} candles",
            snap.ticker,
            snap.timeframe,
            snap.candles.len()
        ),
        is_error: false,
    }
}

pub fn exec_get_candles(
    snap: &ChartSnapshot,
    args: &Value,
    tz: crate::config::UserTimezone,
) -> ToolExecResult {
    let count = args["count"].as_u64().unwrap_or(50).min(200) as usize;
    let (start_ns, end_ns) = parse_time_range(args, tz);

    let filtered: Vec<&data::domain::entities::Candle> = snap
        .candles
        .iter()
        .filter(|c| {
            if let Some(s) = start_ns
                && c.time.0 < s
            {
                return false;
            }
            if let Some(e) = end_ns
                && c.time.0 > e
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
            let delta = c.buy_volume.0 - c.sell_volume.0;
            json!({
                "time": c.time.0 / 1_000,
                "open": c.open.to_f64(),
                "high": c.high.to_f64(),
                "low": c.low.to_f64(),
                "close": c.close.to_f64(),
                "volume": c.volume(),
                "delta": delta,
                "buy_vol": c.buy_volume.0,
                "sell_vol": c.sell_volume.0,
            })
        })
        .collect();

    let summary = format!("{} candles returned", rows.len());
    ToolExecResult {
        content_json: json!({ "candles": rows }).to_string(),
        display_summary: summary,
        is_error: false,
    }
}

pub fn exec_get_market_state(snap: &ChartSnapshot) -> ToolExecResult {
    let candles = &snap.candles;
    if candles.is_empty() {
        return ToolExecResult {
            content_json: json!({ "error": "No candle data" }).to_string(),
            display_summary: "No data".to_string(),
            is_error: true,
        };
    }

    let last = candles.last().unwrap();
    let session_high = candles
        .iter()
        .map(|c| c.high.to_f64())
        .fold(f64::NEG_INFINITY, f64::max);
    let session_low = candles
        .iter()
        .map(|c| c.low.to_f64())
        .fold(f64::INFINITY, f64::min);
    let cum_volume: f64 = candles.iter().map(|c| c.volume() as f64).sum();
    let cum_delta: f64 = candles
        .iter()
        .map(|c| c.buy_volume.0 - c.sell_volume.0)
        .sum();

    let result = json!({
        "last_price": last.close.to_f64(),
        "session_high": session_high,
        "session_low": session_low,
        "cum_volume": cum_volume,
        "cum_delta": cum_delta,
        "candle_count": candles.len(),
        "last_candle": {
            "time": last.time.0 / 1_000,
            "open": last.open.to_f64(),
            "high": last.high.to_f64(),
            "low": last.low.to_f64(),
            "close": last.close.to_f64(),
            "volume": last.volume(),
            "delta": last.buy_volume.0 - last.sell_volume.0,
        }
    });

    ToolExecResult {
        content_json: result.to_string(),
        display_summary: format!(
            "Last {:.2} | H {:.2} L {:.2}",
            last.close.to_f64(),
            session_high,
            session_low
        ),
        is_error: false,
    }
}
