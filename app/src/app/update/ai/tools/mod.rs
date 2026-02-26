//! AI Tool Definitions & Executor
//!
//! Tools are organized into focused submodules:
//! - `market_data` -- chart info, candles, market state
//! - `trades` -- trade analysis, volume profile, delta, aggregation
//! - `studies` -- study values, big trades, footprint, profile data
//! - `analysis` -- drawings query, session stats, level detection
//! - `drawings` -- chart interaction (add/remove drawings)

mod analysis;
pub(crate) mod drawings;
mod market_data;
mod studies;
mod trades;

use data::domain::assistant::ChartSnapshot;
use serde_json::{Value, json};

use crate::app::core::globals::AiStreamEventClone;

/// Context passed to every tool execution — gives access to the
/// snapshot, the event sender (for drawing actions), and the
/// conversation ID.
pub struct ToolContext<'a> {
    pub snapshot: &'a Option<ChartSnapshot>,
    pub sender: &'static tokio::sync::mpsc::UnboundedSender<AiStreamEventClone>,
    pub conversation_id: uuid::Uuid,
}

/// Result of executing a single tool call.
pub struct ToolExecResult {
    pub content_json: String,
    pub display_summary: String,
    pub is_error: bool,
}

// ── Shared helpers ───────────────────────────────────────────────

/// Parse an ISO 8601 time string to milliseconds since epoch.
/// Timestamp.0 in the data crate stores milliseconds.
pub(super) fn parse_iso_to_millis(s: &str) -> Option<u64> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp_millis() as u64);
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(dt.and_utc().timestamp_millis() as u64);
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(dt.and_utc().timestamp_millis() as u64);
    }
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let dt = d.and_hms_opt(0, 0, 0)?;
        return Some(dt.and_utc().timestamp_millis() as u64);
    }
    None
}

/// Parse optional start/end time filters from tool arguments.
/// Returns milliseconds to compare directly with Timestamp.0.
pub(super) fn parse_time_range(args: &Value) -> (Option<u64>, Option<u64>) {
    let start = args["start_time"].as_str().and_then(parse_iso_to_millis);
    let end = args["end_time"].as_str().and_then(parse_iso_to_millis);
    (start, end)
}

/// Compute tick multiplier from tick size.
pub(super) fn tick_multiplier(tick_size: f32) -> i64 {
    if tick_size > 0.0 {
        (1.0 / tick_size as f64) as i64
    } else {
        100
    }
}

/// Parse a color name to a SerializableColor.
pub(super) fn parse_color_name(name: &str) -> data::SerializableColor {
    match name.to_lowercase().as_str() {
        "red" => data::SerializableColor::from_rgb8(220, 50, 47),
        "green" => data::SerializableColor::from_rgb8(81, 205, 160),
        "blue" => data::SerializableColor::from_rgb8(77, 148, 255),
        "yellow" => data::SerializableColor::from_rgb8(255, 204, 0),
        "orange" => data::SerializableColor::from_rgb8(255, 153, 51),
        "purple" => data::SerializableColor::from_rgb8(180, 120, 255),
        "white" => data::SerializableColor::new(1.0, 1.0, 1.0, 1.0),
        "gray" | "grey" => data::SerializableColor::new(0.6, 0.6, 0.6, 1.0),
        "cyan" => data::SerializableColor::from_rgb8(0, 200, 220),
        "magenta" | "pink" => data::SerializableColor::from_rgb8(255, 100, 180),
        _ => data::SerializableColor::from_rgb8(77, 148, 255),
    }
}

/// Parse a LineStyle from a string name.
pub(super) fn parse_line_style(name: &str) -> crate::drawing::LineStyle {
    match name.to_lowercase().as_str() {
        "solid" => crate::drawing::LineStyle::Solid,
        "dashed" => crate::drawing::LineStyle::Dashed,
        "dotted" => crate::drawing::LineStyle::Dotted,
        _ => crate::drawing::LineStyle::Dashed,
    }
}

// ── Public API ───────────────────────────────────────────────────

/// Build the OpenAI-format `tools` array for the API request.
pub fn build_tools_json() -> Value {
    let mut tools = Vec::new();
    tools.extend(market_data::tool_definitions());
    tools.extend(trades::tool_definitions());
    tools.extend(studies::tool_definitions());
    tools.extend(analysis::tool_definitions());
    tools.extend(drawings::tool_definitions());
    json!(tools)
}

/// Execute a tool call against the chart snapshot (or via events buf
/// for drawing actions).
pub fn execute_tool(name: &str, arguments_json: &str, ctx: &ToolContext<'_>) -> ToolExecResult {
    let Some(snap) = ctx.snapshot else {
        return ToolExecResult {
            content_json: json!({
                "error": "No chart linked. Ask the user to set a \
                    link group connecting this AI pane to a chart."
            })
            .to_string(),
            display_summary: "No chart linked".to_string(),
            is_error: true,
        };
    };

    let args: Value = match serde_json::from_str(arguments_json) {
        Ok(v) => v,
        Err(e) => {
            return ToolExecResult {
                content_json: json!({
                    "error": format!("Invalid JSON arguments: {}", e)
                })
                .to_string(),
                display_summary: "Bad arguments".to_string(),
                is_error: true,
            };
        }
    };

    match name {
        // Market data
        "get_chart_info" => market_data::exec_get_chart_info(snap),
        "get_candles" => market_data::exec_get_candles(snap, &args),
        "get_market_state" => market_data::exec_get_market_state(snap),
        // Trades
        "get_trades" => trades::exec_get_trades(snap, &args),
        "get_volume_profile" => trades::exec_get_volume_profile(snap, &args),
        "get_delta_profile" => trades::exec_get_delta_profile(snap, &args),
        "get_aggregated_trades" => trades::exec_get_aggregated_trades(snap, &args),
        // Studies
        "get_study_values" => studies::exec_get_study_values(snap, &args),
        "get_big_trades" => studies::exec_get_big_trades(snap, &args),
        "get_footprint" => studies::exec_get_footprint(snap, &args),
        "get_profile_data" => studies::exec_get_profile_data(snap, &args),
        // Analysis
        "get_drawings" => analysis::exec_get_drawings(snap),
        "get_session_stats" => analysis::exec_get_session_stats(snap, &args),
        "identify_levels" => analysis::exec_identify_levels(snap, &args),
        // Drawing actions
        "add_horizontal_line"
        | "add_vertical_line"
        | "add_text_annotation"
        | "add_price_level"
        | "add_price_label"
        | "add_line"
        | "add_extended_line"
        | "add_rectangle"
        | "add_ellipse"
        | "add_arrow"
        | "add_fib_retracement"
        | "remove_drawing"
        | "remove_all_drawings" => drawings::execute_drawing_tool(name, &args, ctx),
        _ => ToolExecResult {
            content_json: json!({
                "error": format!("Unknown tool: {}", name)
            })
            .to_string(),
            display_summary: format!("Unknown tool: {}", name),
            is_error: true,
        },
    }
}
