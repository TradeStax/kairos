//! Crosshair Overlay
//!
//! Draws horizontal and vertical crosshair lines that snap to price/time grid.

use crate::chart::core::{Interaction, ViewState};
use crate::style;
use data::ChartBasis;
use iced::widget::canvas::{Frame, Path};
use iced::{Point, Size, Theme};

/// Result of drawing crosshair - the snapped price and interval values
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
    interaction: &Interaction,
) -> CrosshairResult {
    let region = state.visible_region(bounds);
    let dashed_line = style::dashed_line(theme);

    let highest_p = state.y_to_price(region.y);
    let lowest_p = state.y_to_price(region.y + region.height);
    let highest = highest_p.to_f32_lossy();
    let lowest = lowest_p.to_f32_lossy();

    let tick_size = state.tick_size.to_f32_lossy();

    // Check if ruler is active and draw it
    if let Interaction::Ruler { start: Some(_) } = interaction {
        // Ruler drawing is handled by ruler.rs
    }

    // Horizontal price line
    let crosshair_ratio = cursor_position.y / bounds.height;
    let crosshair_price = highest + crosshair_ratio * (lowest - highest);

    let rounded_price = (crosshair_price / tick_size).round() * tick_size;
    let snap_ratio = (rounded_price - highest) / (lowest - highest);

    frame.stroke(
        &Path::line(
            Point::new(0.0, snap_ratio * bounds.height),
            Point::new(bounds.width, snap_ratio * bounds.height),
        ),
        dashed_line,
    );

    // Vertical time/tick line
    let rounded_interval = match state.basis {
        ChartBasis::Time(_) => {
            let (rounded_timestamp, snap_ratio) =
                state.snap_x_to_index(cursor_position.x, bounds, region);

            frame.stroke(
                &Path::line(
                    Point::new(snap_ratio * bounds.width, 0.0),
                    Point::new(snap_ratio * bounds.width, bounds.height),
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

            frame.stroke(
                &Path::line(
                    Point::new(snap_ratio * bounds.width, 0.0),
                    Point::new(snap_ratio * bounds.width, bounds.height),
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
