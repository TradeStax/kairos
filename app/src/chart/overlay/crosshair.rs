//! Crosshair Overlay
//!
//! Draws horizontal and vertical crosshair lines that snap to price/time grid,
//! with label boxes at the Y-axis (price) and X-axis (time) edges.

use crate::chart::core::{Interaction, ViewState};
use crate::components::primitives::AZERET_MONO;
use crate::style;
use crate::style::tokens;
use data::ChartBasis;
use iced::theme::palette::Extended;
use iced::widget::canvas::{Frame, Path, Text};
use iced::{Point, Size, Theme};

/// Result of drawing crosshair - the snapped price and interval values
#[allow(dead_code)]
pub struct CrosshairResult {
    /// Rounded price at crosshair position
    pub price: f32,
    /// Rounded timestamp or tick index at crosshair position
    pub interval: u64,
}

/// Draw crosshair lines on the chart
///
/// Returns the snapped price and interval values for use in axis labels.
pub fn draw_crosshair(
    state: &ViewState,
    frame: &mut Frame,
    theme: &Theme,
    bounds: Size,
    cursor_position: Point,
    _interaction: &Interaction,
) -> CrosshairResult {
    let region = state.visible_region(bounds);
    let dashed_line = style::dashed_line(theme);
    let palette = theme.extended_palette();

    let highest_p = state.y_to_price(region.y);
    let lowest_p = state.y_to_price(region.y + region.height);
    let highest = highest_p.to_f32();
    let lowest = lowest_p.to_f32();

    // Horizontal price line
    if bounds.height < f32::EPSILON {
        return CrosshairResult {
            price: highest,
            interval: 0,
        };
    }

    let crosshair_ratio = cursor_position.y / bounds.height;
    let crosshair_price = highest + crosshair_ratio * (lowest - highest);

    let rounded_price = data::Price::from_f32(crosshair_price)
        .round_to_tick(state.tick_size.to_price())
        .to_f32();
    let price_range = lowest - highest;
    let snap_ratio = if price_range.abs() < f32::EPSILON {
        0.5 // Center when no price range
    } else {
        (rounded_price - highest) / price_range
    };

    let snapped_y = snap_ratio * bounds.height;

    frame.stroke(
        &Path::line(
            Point::new(0.0, snapped_y),
            Point::new(bounds.width, snapped_y),
        ),
        dashed_line,
    );

    // Vertical time/tick line
    let rounded_interval = match state.basis {
        ChartBasis::Time(_) => {
            let (rounded_timestamp, snap_ratio) =
                state.snap_x_to_index(cursor_position.x, bounds, region);

            let snapped_x = snap_ratio * bounds.width;

            frame.stroke(
                &Path::line(
                    Point::new(snapped_x, 0.0),
                    Point::new(snapped_x, bounds.height),
                ),
                dashed_line,
            );

            // Time label box at bottom edge
            let time_text = format_timestamp(rounded_timestamp);
            draw_time_label(frame, palette, &time_text, snapped_x, bounds);

            rounded_timestamp
        }
        ChartBasis::Tick(aggregation) => {
            let (chart_x_min, chart_x_max) = (region.x, region.x + region.width);
            let crosshair_pos = chart_x_min + (cursor_position.x / bounds.width) * region.width;

            let cell_index = (crosshair_pos / state.cell_width).round();

            let snapped_crosshair = cell_index * state.cell_width;
            let snap_ratio = (snapped_crosshair - chart_x_min) / (chart_x_max - chart_x_min);

            let rounded_tick = (-cell_index as u64) * (u64::from(aggregation));

            let snapped_x = snap_ratio * bounds.width;

            frame.stroke(
                &Path::line(
                    Point::new(snapped_x, 0.0),
                    Point::new(snapped_x, bounds.height),
                ),
                dashed_line,
            );

            // Tick label box at bottom edge
            let tick_text = format!("#{}", rounded_tick);
            draw_time_label(frame, palette, &tick_text, snapped_x, bounds);

            rounded_tick
        }
    };

    CrosshairResult {
        price: rounded_price,
        interval: rounded_interval,
    }
}

/// Draw a remote crosshair vertical line from a linked pane.
///
/// Only draws the vertical line and time label (no horizontal price line)
/// since the remote pane may have a different price scale.
pub fn draw_remote_crosshair(
    state: &ViewState,
    frame: &mut Frame,
    theme: &Theme,
    bounds: Size,
    interval: u64,
) {
    let region = state.visible_region(bounds);
    let dashed_line = style::dashed_line(theme);
    let palette = theme.extended_palette();

    match state.basis {
        ChartBasis::Time(_) => {
            // Convert interval to chart X coordinate, then to screen X
            let chart_x = state.interval_to_x(interval);
            let x_min = region.x;
            let x_max = region.x + region.width;
            let range = x_max - x_min;

            if range.abs() < f32::EPSILON {
                return;
            }

            let screen_x = ((chart_x - x_min) / range) * bounds.width;

            if screen_x < 0.0 || screen_x > bounds.width {
                return;
            }

            frame.stroke(
                &Path::line(
                    Point::new(screen_x, 0.0),
                    Point::new(screen_x, bounds.height),
                ),
                dashed_line,
            );

            let time_text = format_timestamp(interval);
            draw_time_label(frame, palette, &time_text, screen_x, bounds);
        }
        ChartBasis::Tick(aggregation) => {
            // Convert tick index back to cell position
            let agg = u64::from(aggregation);
            if agg == 0 {
                return;
            }
            let cell_index = -(interval as f32 / agg as f32);
            let chart_x = cell_index * state.cell_width;

            let x_min = region.x;
            let x_max = region.x + region.width;
            let range = x_max - x_min;

            if range.abs() < f32::EPSILON {
                return;
            }

            let screen_x = ((chart_x - x_min) / range) * bounds.width;

            if screen_x < 0.0 || screen_x > bounds.width {
                return;
            }

            frame.stroke(
                &Path::line(
                    Point::new(screen_x, 0.0),
                    Point::new(screen_x, bounds.height),
                ),
                dashed_line,
            );

            let tick_text = format!("#{}", interval);
            draw_time_label(frame, palette, &tick_text, screen_x, bounds);
        }
    }
}

/// Draw a time/tick label box at the bottom edge of the chart.
fn draw_time_label(
    frame: &mut Frame,
    palette: &Extended,
    label: &str,
    x: f32,
    bounds: Size,
) {
    let text_size = tokens::text::SMALL;
    let char_width = text_size * 0.7;
    let pad_x = 4.0;
    let pad_y = 2.0;
    let label_w = label.len() as f32 * char_width + pad_x * 2.0;
    let label_h = text_size + pad_y * 2.0;

    let label_x = (x - label_w / 2.0).clamp(0.0, bounds.width - label_w);
    let label_y = bounds.height - label_h;

    frame.fill_rectangle(
        Point::new(label_x, label_y),
        Size::new(label_w, label_h),
        palette.secondary.base.color,
    );

    frame.fill_text(Text {
        content: label.to_string(),
        position: Point::new(label_x + pad_x, label_y + pad_y),
        size: iced::Pixels(text_size),
        color: palette.secondary.base.text,
        font: AZERET_MONO,
        ..Text::default()
    });
}

/// Format a millisecond timestamp to a short time string.
pub(crate) fn format_timestamp(ms: u64) -> String {
    if ms == 0 {
        return String::new();
    }
    let secs = (ms / 1000) as i64;
    let hours = (secs / 3600) % 24;
    let minutes = (secs / 60) % 60;
    format!("{:02}:{:02}", hours, minutes)
}
