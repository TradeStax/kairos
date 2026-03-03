//! Chart Grid Lines
//!
//! Draws subtle horizontal price grid lines and vertical time grid lines
//! behind all chart content.

use crate::chart::core::ViewState;
use crate::chart::core::tokens;
use data::ChartBasis;
use iced::theme::palette::Extended;
use iced::widget::canvas::{Frame, Path, Stroke};
use iced::{Point, Rectangle};

/// Draw horizontal price grid lines at Y-axis tick intervals.
pub fn draw_price_grid(
    state: &ViewState,
    frame: &mut Frame,
    palette: &Extended,
    region: &Rectangle,
) {
    let highest = state.y_to_price(region.y).to_f32();
    let lowest = state.y_to_price(region.y + region.height).to_f32();

    if !highest.is_finite() || !lowest.is_finite() {
        return;
    }

    let range = (highest - lowest).abs();
    if range < f32::EPSILON {
        return;
    }

    // Calculate how many labels can fit vertically
    let labels_can_fit = (region.height / 40.0) as i32;
    if labels_can_fit < 1 {
        return;
    }

    let (step, rounded_highest) = calc_grid_ticks(highest, lowest, labels_can_fit);
    if step < f32::EPSILON || !step.is_finite() {
        return;
    }

    let grid_color = palette
        .background
        .weak
        .color
        .scale_alpha(tokens::grid::ALPHA);
    let grid_stroke = Stroke {
        width: tokens::grid::LINE_WIDTH,
        ..Stroke::default()
    };
    let grid_stroke = Stroke::with_color(grid_stroke, grid_color);

    let grid_path = Path::new(|builder| {
        let mut price = rounded_highest;
        let mut iterations = 0;

        while price >= lowest && iterations < 200 {
            let y = state.price_to_y(data::Price::from_f32(price));
            builder.move_to(Point::new(region.x, y));
            builder.line_to(Point::new(region.x + region.width, y));
            price -= step;
            iterations += 1;
        }
    });
    frame.stroke(&grid_path, grid_stroke);
}

/// Draw vertical time grid lines at X-axis intervals.
pub fn draw_time_grid(
    state: &ViewState,
    frame: &mut Frame,
    palette: &Extended,
    region: &Rectangle,
) {
    let grid_color = palette
        .background
        .weak
        .color
        .scale_alpha(tokens::grid::ALPHA);
    let grid_stroke = Stroke {
        width: tokens::grid::LINE_WIDTH,
        ..Stroke::default()
    };
    let grid_stroke = Stroke::with_color(grid_stroke, grid_color);

    match state.basis {
        ChartBasis::Time(timeframe) => {
            let interval_ms = timeframe.to_milliseconds();
            let earliest = state.x_to_interval(region.x);
            let latest = state.x_to_interval(region.x + region.width);

            if latest < earliest {
                return;
            }

            // Choose a grid step: enough so lines aren't too dense
            let visible_intervals = (latest - earliest) / interval_ms;
            let target_lines = (region.width / 80.0).max(3.0) as u64;
            let step_mult = (visible_intervals / target_lines).max(1);

            // Round step to a nice number
            let step_ms = interval_ms * round_up_nice(step_mult);

            if step_ms == 0 {
                return;
            }

            let start = (earliest / step_ms) * step_ms;

            let time_grid_path = Path::new(|builder| {
                let mut t = start;
                let mut iterations = 0;
                while t <= latest && iterations < 200 {
                    let x = state.interval_to_x(t);
                    builder.move_to(Point::new(x, region.y));
                    builder.line_to(Point::new(x, region.y + region.height));
                    t += step_ms;
                    iterations += 1;
                }
            });
            frame.stroke(&time_grid_path, grid_stroke);
        }
        ChartBasis::Tick(_) => {
            // For tick-based charts, draw vertical lines at regular cell intervals
            let earliest_idx = state.x_to_interval(region.x + region.width);
            let latest_idx = state.x_to_interval(region.x);

            let visible_cells = latest_idx.saturating_sub(earliest_idx);
            let target_lines = (region.width / 80.0).max(3.0) as u64;
            let step = (visible_cells / target_lines).max(1);
            let step = round_up_nice(step);

            let start = (earliest_idx / step) * step;

            let tick_grid_path = Path::new(|builder| {
                let mut idx = start;
                let mut iterations = 0;
                while idx <= latest_idx && iterations < 200 {
                    let x = state.interval_to_x(idx);
                    builder.move_to(Point::new(x, region.y));
                    builder.line_to(Point::new(x, region.y + region.height));
                    idx += step;
                    iterations += 1;
                }
            });
            frame.stroke(&tick_grid_path, grid_stroke);
        }
    }
}

/// Draw vertical separator lines at day boundaries for sub-daily charts.
///
/// Only draws for `ChartBasis::Time` with intervals less than one day.
/// Uses UTC midnight as the boundary — an acceptable approximation that
/// avoids threading timezone into the canvas renderer.
pub fn draw_date_separators(
    state: &ViewState,
    frame: &mut Frame,
    palette: &Extended,
    region: &Rectangle,
) {
    let interval_ms = match state.basis {
        ChartBasis::Time(timeframe) => timeframe.to_milliseconds(),
        ChartBasis::Tick(_) => return,
    };

    const ONE_DAY_MS: u64 = 24 * 60 * 60 * 1000;

    if interval_ms >= ONE_DAY_MS {
        return;
    }

    let earliest = state.x_to_interval(region.x);
    let latest = state.x_to_interval(region.x + region.width);

    if latest <= earliest {
        return;
    }

    let sep_color = palette
        .background
        .weak
        .color
        .scale_alpha(tokens::date_separator::ALPHA);
    let sep_stroke = Stroke {
        width: tokens::date_separator::LINE_WIDTH,
        ..Stroke::default()
    };
    let sep_stroke = Stroke::with_color(sep_stroke, sep_color);

    // Find first midnight at or after earliest
    let first_midnight = earliest.div_ceil(ONE_DAY_MS) * ONE_DAY_MS;

    let sep_path = Path::new(|builder| {
        let mut t = first_midnight;
        let mut iterations = 0;
        while t <= latest && iterations < 100 {
            let x = state.interval_to_x(t);
            builder.move_to(Point::new(x, region.y));
            builder.line_to(Point::new(x, region.y + region.height));
            t += ONE_DAY_MS;
            iterations += 1;
        }
    });
    frame.stroke(&sep_path, sep_stroke);
}

/// Calculate optimal tick step and starting value for grid lines.
/// Same algorithm as `linear::calc_optimal_ticks`.
fn calc_grid_ticks(highest: f32, lowest: f32, labels_can_fit: i32) -> (f32, f32) {
    let range = (highest - lowest).abs().max(f32::EPSILON);
    let labels = labels_can_fit.max(1) as f32;

    let base = 10.0f32.powf(range.log10().floor());

    let step = match range / base {
        r if r <= labels * 0.1 => 0.1 * base,
        r if r <= labels * 0.2 => 0.2 * base,
        r if r <= labels * 0.5 => 0.5 * base,
        r if r <= labels => base,
        r if r <= labels * 2.0 => 2.0 * base,
        _ => (range / labels).min(5.0 * base),
    };

    let rounded_highest = (highest / step).ceil() * step;
    (step, rounded_highest)
}

/// Round up to a "nice" number (1, 2, 5, 10, 20, 50, etc.)
fn round_up_nice(n: u64) -> u64 {
    if n <= 1 {
        return 1;
    }
    let mag = 10u64.pow((n as f64).log10().floor() as u32);
    let normalized = n as f64 / mag as f64;
    let nice = if normalized <= 1.0 {
        1
    } else if normalized <= 2.0 {
        2
    } else if normalized <= 5.0 {
        5
    } else {
        10
    };
    nice * mag
}
