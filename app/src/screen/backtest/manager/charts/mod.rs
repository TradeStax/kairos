//! Canvas `Program` implementations for the backtest management modal.
//!
//! Seven chart types used in the analytics tabs: equity curve,
//! drawdown, Monte Carlo simulation, histogram, scatter, bar chart,
//! and monthly returns heatmap grid.
//!
//! All charts use `ChartHoverState` as their `Program::State` and
//! render in two layers: a cached base layer and a fresh overlay
//! (crosshair, tooltip, hover highlight) redrawn each frame.

pub mod bar_chart;
pub mod drawdown;
pub mod equity;
pub mod histogram;
pub mod monte_carlo;
pub mod returns_grid;
pub mod scatter;

pub use bar_chart::BarChart;
pub use drawdown::DrawdownChart;
pub use equity::{EquityChart, PropFirmEquityChart};
pub use histogram::HistogramChart;
pub use monte_carlo::MonteCarloChart;
pub use returns_grid::ReturnsGrid;
pub use scatter::ScatterChart;

use super::ManagerMessage;
use crate::config::UserTimezone;
use crate::style::tokens;
use iced::widget::canvas::{Frame, Path, Stroke, Text};
use iced::{Color, Point, Rectangle, Size};

// ── Shared Hover State ─────────────────────────────────────────────

#[derive(Default, Debug, Clone)]
pub struct ChartHoverState {
    pub cursor: Option<Point>,
}

// ── Tooltip Constants ──────────────────────────────────────────────

pub(super) const TOOLTIP_PADDING: f32 = 6.0;
pub(super) const TOOLTIP_OFFSET: f32 = 12.0;
pub(super) const TOOLTIP_FONT_SIZE: f32 = 10.0;
pub(super) const TOOLTIP_LINE_HEIGHT: f32 = 14.0;
pub(super) const TOOLTIP_BG: Color = Color::from_rgba(0.1, 0.1, 0.12, 0.92);
pub(super) const TOOLTIP_BORDER: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.15);

// ── Shared Helpers ─────────────────────────────────────────────────

pub(super) fn grid_lines(frame: &mut Frame, bounds: Rectangle, padding: f32, n_lines: usize) {
    let color = tokens::backtest::GRID_LINE;
    let usable_h = bounds.height - padding * 2.0;
    for i in 1..=n_lines {
        let y = padding + usable_h * (i as f32 / (n_lines + 1) as f32);
        let line = Path::line(
            Point::new(padding, y),
            Point::new(bounds.width - padding, y),
        );
        frame.stroke(
            &line,
            Stroke {
                style: color.into(),
                width: 1.0,
                ..Default::default()
            },
        );
    }
}

pub(super) fn format_currency(value: f64) -> String {
    let abs = value.abs();
    let formatted = if abs >= 1_000_000.0 {
        format!("{:.1}M", abs / 1_000_000.0)
    } else if abs >= 1_000.0 {
        let whole = abs as i64;
        let thousands = whole / 1000;
        let remainder = whole % 1000;
        format!("{},{:03}", thousands, remainder)
    } else {
        format!("{:.0}", abs)
    };
    if value < 0.0 {
        format!("-${}", formatted)
    } else {
        format!("${}", formatted)
    }
}

/// Position a tooltip box so it stays within canvas bounds.
pub(super) fn position_tooltip(
    cursor: Point,
    tooltip_w: f32,
    tooltip_h: f32,
    bounds: Size,
) -> Point {
    let mut x = cursor.x + TOOLTIP_OFFSET;
    let mut y = cursor.y - tooltip_h - TOOLTIP_OFFSET;

    // Flip right → left if overflows right edge
    if x + tooltip_w > bounds.width {
        x = cursor.x - tooltip_w - TOOLTIP_OFFSET;
    }
    // Flip above → below if overflows top edge
    if y < 0.0 {
        y = cursor.y + TOOLTIP_OFFSET;
    }
    // Clamp to bounds
    x = x.clamp(0.0, (bounds.width - tooltip_w).max(0.0));
    y = y.clamp(0.0, (bounds.height - tooltip_h).max(0.0));

    Point::new(x, y)
}

/// Draw a tooltip box with multiple text lines.
pub(super) fn draw_tooltip_box(frame: &mut Frame, position: Point, lines: &[String]) {
    use iced::widget::canvas::Fill;

    if lines.is_empty() {
        return;
    }

    let max_chars = lines.iter().map(|l| l.len()).max().unwrap_or(0);
    let w = max_chars as f32 * 6.0 + TOOLTIP_PADDING * 2.0;
    let h = lines.len() as f32 * TOOLTIP_LINE_HEIGHT + TOOLTIP_PADDING * 2.0;

    // Background
    let bg = Path::rectangle(position, Size::new(w, h));
    frame.fill(
        &bg,
        Fill {
            style: TOOLTIP_BG.into(),
            ..Default::default()
        },
    );
    // Border
    frame.stroke(
        &bg,
        Stroke {
            style: TOOLTIP_BORDER.into(),
            width: 1.0,
            ..Default::default()
        },
    );

    // Text lines
    for (i, line) in lines.iter().enumerate() {
        let text = Text {
            content: line.clone(),
            position: Point::new(
                position.x + TOOLTIP_PADDING,
                position.y + TOOLTIP_PADDING + i as f32 * TOOLTIP_LINE_HEIGHT,
            ),
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.85),
            size: iced::Pixels(TOOLTIP_FONT_SIZE),
            ..Default::default()
        };
        frame.fill_text(text);
    }
}

/// Draw dashed crosshair lines (horizontal + vertical).
pub(super) fn draw_crosshair_lines(frame: &mut Frame, cursor: Point, bounds: Size, pad: f32) {
    let color = tokens::backtest::CROSSHAIR_LINE;
    let dash_len = 4.0_f32;
    let gap_len = 3.0_f32;

    // Vertical line
    let mut y = pad;
    while y < bounds.height - pad {
        let end = (y + dash_len).min(bounds.height - pad);
        let seg = Path::line(Point::new(cursor.x, y), Point::new(cursor.x, end));
        frame.stroke(
            &seg,
            Stroke {
                style: color.into(),
                width: 1.0,
                ..Default::default()
            },
        );
        y += dash_len + gap_len;
    }

    // Horizontal line
    let mut x = pad;
    while x < bounds.width - pad {
        let end = (x + dash_len).min(bounds.width - pad);
        let seg = Path::line(Point::new(x, cursor.y), Point::new(end, cursor.y));
        frame.stroke(
            &seg,
            Stroke {
                style: color.into(),
                width: 1.0,
                ..Default::default()
            },
        );
        x += dash_len + gap_len;
    }
}

/// Draw a snap dot at a point on a curve.
pub(super) fn draw_snap_dot(frame: &mut Frame, center: Point, radius: f32) {
    use iced::widget::canvas::Fill;
    let circle = Path::circle(center, radius);
    frame.fill(
        &circle,
        Fill {
            style: tokens::backtest::SNAP_DOT.into(),
            ..Default::default()
        },
    );
}

/// Common update logic: track cursor position, return redraw action.
pub(super) fn handle_cursor_event(
    state: &mut ChartHoverState,
    event: &iced::widget::canvas::Event,
    bounds: Rectangle,
    cursor: iced::mouse::Cursor,
) -> Option<iced::widget::canvas::Action<ManagerMessage>> {
    use iced::mouse;
    match event {
        iced::widget::canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
            let new_pos = cursor.position_in(bounds);
            if new_pos != state.cursor {
                state.cursor = new_pos;
                Some(iced::widget::canvas::Action::request_redraw())
            } else {
                None
            }
        }
        iced::widget::canvas::Event::Mouse(mouse::Event::CursorLeft) => {
            if state.cursor.is_some() {
                state.cursor = None;
                Some(iced::widget::canvas::Action::request_redraw())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Tooltip width estimate from lines.
pub(super) fn tooltip_size(lines: &[String]) -> (f32, f32) {
    let max_chars = lines.iter().map(|l| l.len()).max().unwrap_or(0);
    let w = max_chars as f32 * 6.0 + TOOLTIP_PADDING * 2.0;
    let h = lines.len() as f32 * TOOLTIP_LINE_HEIGHT + TOOLTIP_PADDING * 2.0;
    (w, h)
}

/// Format a timestamp (ms) to a short date string in the
/// user's timezone.
pub(super) fn format_date(ts_ms: u64, tz: UserTimezone) -> String {
    let millis = ts_ms as i64;
    let Some(dt) = chrono::DateTime::from_timestamp_millis(millis) else {
        return format!("{}", ts_ms);
    };
    match tz {
        UserTimezone::Local => dt
            .with_timezone(&chrono::Local)
            .format("%m/%d %H:%M")
            .to_string(),
        UserTimezone::Utc => dt.format("%m/%d %H:%M").to_string(),
    }
}
