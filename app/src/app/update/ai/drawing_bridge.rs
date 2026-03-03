//! Drawing bridge: converts `ai::DrawingSpec` → `SerializableDrawing`.
//!
//! The AI crate produces flat `DrawingSpec` values that are decoupled from
//! the app's drawing types.  This module translates them into the native
//! `SerializableDrawing` used by the chart drawing system.

use crate::drawing::{
    DrawingId, DrawingStyle, DrawingTool, FibonacciConfig, LineStyle, SerializableDrawing,
    SerializablePoint,
};

/// Convert an f64 price to fixed-point i64 units (10^-8 precision).
fn price_to_units(price: f64) -> i64 {
    (price * 1e8).round() as i64
}

/// Convert an `ai::DrawingSpec` into a native `SerializableDrawing`.
pub(crate) fn spec_to_drawing(spec: &ai::DrawingSpec) -> SerializableDrawing {
    let tool = match spec.tool_name.as_str() {
        "horizontal_line" => DrawingTool::HorizontalLine,
        "vertical_line" => DrawingTool::VerticalLine,
        "line" => DrawingTool::Line,
        "ray" => DrawingTool::Ray,
        "extended_line" => DrawingTool::ExtendedLine,
        "rectangle" => DrawingTool::Rectangle,
        "text_label" => DrawingTool::TextLabel,
        "price_label" => DrawingTool::PriceLabel,
        "arrow" => DrawingTool::Arrow,
        "fib_retracement" => DrawingTool::FibRetracement,
        "parallel_channel" => DrawingTool::ParallelChannel,
        _ => DrawingTool::Line,
    };

    let points = build_points(tool, spec);

    let mut style = DrawingStyle::default();

    if let Some(ref color) = spec.color {
        style.stroke_color = *color;
        if matches!(tool, DrawingTool::Rectangle | DrawingTool::Ellipse) {
            style.fill_color = Some(*color);
        }
    }

    if let Some(ref line_style) = spec.line_style {
        style.line_style = match line_style.as_str() {
            "dashed" => LineStyle::Dashed,
            "dotted" => LineStyle::Dotted,
            "dash_dot" => LineStyle::DashDot,
            _ => LineStyle::Solid,
        };
    }

    if let Some(opacity) = spec.opacity {
        style.fill_opacity = opacity;
    }

    if let Some(ref text) = spec.text {
        style.text = Some(text.clone());
    }

    if spec.fibonacci {
        style.fibonacci = Some(FibonacciConfig::default());
    }

    SerializableDrawing {
        id: DrawingId(uuid::Uuid::new_v4()),
        tool,
        points,
        style,
        visible: true,
        locked: false,
        label: spec.label.clone(),
    }
}

/// Build the `SerializablePoint` list for a given drawing tool from the spec.
fn build_points(tool: DrawingTool, spec: &ai::DrawingSpec) -> Vec<SerializablePoint> {
    match tool {
        // Single-price, no time
        DrawingTool::HorizontalLine | DrawingTool::PriceLabel => {
            let price = spec.price.unwrap_or(0.0);
            vec![SerializablePoint::new(price_to_units(price), 0)]
        }
        // Single-time, no price
        DrawingTool::VerticalLine => {
            let time = spec.time_millis.unwrap_or(0);
            vec![SerializablePoint::new(0, time)]
        }
        // Two-corner range drawings
        DrawingTool::Rectangle | DrawingTool::FibRetracement => {
            let p1 = spec.price_high.or(spec.from_price).unwrap_or(0.0);
            let p2 = spec.price_low.or(spec.to_price).unwrap_or(0.0);
            let t1 = spec.from_time_millis.unwrap_or(0);
            let t2 = spec.to_time_millis.unwrap_or(0);
            vec![
                SerializablePoint::new(price_to_units(p1), t1),
                SerializablePoint::new(price_to_units(p2), t2),
            ]
        }
        // Single-point with time (text label)
        DrawingTool::TextLabel => {
            let price = spec.price.unwrap_or(0.0);
            let time = spec.time_millis.unwrap_or(0);
            vec![SerializablePoint::new(price_to_units(price), time)]
        }
        // Two-point tools: Line, Ray, ExtendedLine, Arrow
        DrawingTool::Line | DrawingTool::Ray | DrawingTool::ExtendedLine | DrawingTool::Arrow => {
            let fp = spec.from_price.unwrap_or(0.0);
            let ft = spec.from_time_millis.unwrap_or(0);
            let tp = spec.to_price.unwrap_or(0.0);
            let tt = spec.to_time_millis.unwrap_or(0);
            vec![
                SerializablePoint::new(price_to_units(fp), ft),
                SerializablePoint::new(price_to_units(tp), tt),
            ]
        }
        // Three-point: ParallelChannel (from, to, anchor)
        DrawingTool::ParallelChannel => {
            let fp = spec.from_price.unwrap_or(0.0);
            let ft = spec.from_time_millis.unwrap_or(0);
            let tp = spec.to_price.unwrap_or(0.0);
            let tt = spec.to_time_millis.unwrap_or(0);
            let mid_time = (ft + tt) / 2;
            vec![
                SerializablePoint::new(price_to_units(fp), ft),
                SerializablePoint::new(price_to_units(tp), tt),
                SerializablePoint::new(price_to_units(fp), mid_time),
            ]
        }
        // Fallback: two-point
        _ => {
            let fp = spec.from_price.or(spec.price).unwrap_or(0.0);
            let ft = spec.from_time_millis.or(spec.time_millis).unwrap_or(0);
            let tp = spec.to_price.unwrap_or(fp);
            let tt = spec.to_time_millis.unwrap_or(ft);
            vec![
                SerializablePoint::new(price_to_units(fp), ft),
                SerializablePoint::new(price_to_units(tp), tt),
            ]
        }
    }
}
