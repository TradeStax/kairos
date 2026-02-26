//! Crosshair Overlay
//!
//! Draws horizontal and vertical crosshair lines that snap to price/time grid.
//! Time/date labels are rendered by AxisLabelsX on the X-axis widget.

use crate::chart::core::{Interaction, ViewState};
use crate::style;
use data::ChartBasis;
use iced::widget::canvas::{Frame, Path};
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
/// Only draws the vertical line (no horizontal price line)
/// since the remote pane may have a different price scale.
/// Time labels are rendered by AxisLabelsX.
pub fn draw_remote_crosshair(
    state: &ViewState,
    frame: &mut Frame,
    theme: &Theme,
    bounds: Size,
    interval: u64,
) {
    let region = state.visible_region(bounds);
    let dashed_line = style::dashed_line(theme);

    match state.basis {
        ChartBasis::Time(_) => {
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
        }
        ChartBasis::Tick(aggregation) => {
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
        }
    }
}
