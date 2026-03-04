//! Strategy-specific chart overlay helpers and context formatting.

use crate::style::tokens;
use iced::widget::canvas::{Frame, Path, Stroke};
use iced::{Color, Point};

/// Draw strategy-specific overlays on the trade chart.
pub fn draw_strategy_overlays(
    frame: &mut Frame,
    strategy_id: &str,
    context: &std::collections::BTreeMap<String, backtest::ContextValue>,
    price_to_y: &dyn Fn(f64) -> f32,
    x_start: f32,
    x_end: f32,
) {
    let overlay_color = tokens::backtest::STRATEGY_OVERLAY;
    let line_color = tokens::backtest::STRATEGY_OVERLAY_LINE;

    match strategy_id {
        "orb" => {
            if let (Some(high), Some(low)) = (
                context.get("or_high").and_then(|v| v.as_f64()),
                context.get("or_low").and_then(|v| v.as_f64()),
            ) {
                let y_high = price_to_y(high);
                let y_low = price_to_y(low);
                // Fill band
                let band = Path::rectangle(
                    Point::new(x_start, y_high),
                    iced::Size::new(x_end - x_start, y_low - y_high),
                );
                frame.fill(
                    &band,
                    iced::widget::canvas::Fill {
                        style: overlay_color.into(),
                        ..Default::default()
                    },
                );
                // Border lines
                for y in [y_high, y_low] {
                    let line = Path::line(Point::new(x_start, y), Point::new(x_end, y));
                    frame.stroke(
                        &line,
                        Stroke {
                            style: line_color.into(),
                            width: 1.0,
                            ..Default::default()
                        },
                    );
                }
            }
        }
        "vwap_reversion" => {
            if let Some(vwap) = context.get("vwap").and_then(|v| v.as_f64()) {
                let y = price_to_y(vwap);
                let line = Path::line(Point::new(x_start, y), Point::new(x_end, y));
                frame.stroke(
                    &line,
                    Stroke {
                        style: line_color.into(),
                        width: 1.5,
                        ..Default::default()
                    },
                );
            }
            for key in ["upper_band", "lower_band"] {
                if let Some(val) = context.get(key).and_then(|v| v.as_f64()) {
                    let y = price_to_y(val);
                    draw_dashed_h_line(frame, x_start, x_end, y, line_color);
                }
            }
        }
        "momentum_breakout" => {
            for key in ["channel_high", "channel_low"] {
                if let Some(val) = context.get(key).and_then(|v| v.as_f64()) {
                    let y = price_to_y(val);
                    let line = Path::line(Point::new(x_start, y), Point::new(x_end, y));
                    frame.stroke(
                        &line,
                        Stroke {
                            style: line_color.into(),
                            width: 1.0,
                            ..Default::default()
                        },
                    );
                }
            }
        }
        _ => {} // Unknown strategy — skip chart overlays
    }
}

fn draw_dashed_h_line(frame: &mut Frame, x_start: f32, x_end: f32, y: f32, color: Color) {
    let dash = 4.0_f32;
    let gap = 3.0_f32;
    let mut x = x_start;
    while x < x_end {
        let end = (x + dash).min(x_end);
        let seg = Path::line(Point::new(x, y), Point::new(end, y));
        frame.stroke(
            &seg,
            Stroke {
                style: color.into(),
                width: 1.0,
                ..Default::default()
            },
        );
        x += dash + gap;
    }
}

/// Build a compact strategy context summary for the info bar.
///
/// Returns `None` if no context data is available.
pub fn strategy_context_summary(
    context: &std::collections::BTreeMap<String, backtest::ContextValue>,
) -> Option<String> {
    if context.is_empty() {
        return None;
    }
    let parts: Vec<String> = context
        .iter()
        .map(|(key, value)| {
            let val_str = format_context_value(key, value);
            let key_str = format_context_key(key);
            format!("{}: {}", key_str, val_str)
        })
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" \u{00B7} "))
    }
}

fn format_context_key(key: &str) -> String {
    key.replace('_', " ")
        .split(' ')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(c) => {
                    let mut s = c.to_uppercase().to_string();
                    s.extend(chars);
                    s
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_context_value(key: &str, value: &backtest::ContextValue) -> String {
    match value {
        backtest::ContextValue::Price(p) => format!("{:.2}", p.to_f64()),
        backtest::ContextValue::Float(f) => {
            if key.contains("pct") || key.contains("percent") {
                format!("{:.2}%", f)
            } else {
                format!("{:.4}", f)
            }
        }
        backtest::ContextValue::Integer(i) => format!("{}", i),
        backtest::ContextValue::Bool(b) => format!("{}", b),
        backtest::ContextValue::Text(s) => s.clone(),
        backtest::ContextValue::Timestamp(ts) => format!("{}", ts.0),
    }
}
