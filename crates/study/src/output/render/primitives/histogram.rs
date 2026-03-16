//! Histogram renderer.
//!
//! Renders MACD-style histogram bars with a centered baseline at 0.

use super::super::canvas::Canvas;
use super::super::chart_view::ChartView;
use super::super::constants::ZERO_LINE_COLOR;
use super::super::types::LineStyle;
use crate::output::HistogramBar;

/// Render histogram bars.
pub fn render_histogram(canvas: &mut dyn Canvas, bars: &[HistogramBar], view: &dyn ChartView) {
    if bars.is_empty() {
        return;
    }

    let bar_width = view.cell_width() * 0.6;

    for bar in bars {
        let sx = view.interval_to_x(bar.x);
        let left = sx - bar_width / 2.0;

        let y_val = view.value_to_y(bar.value);
        let y_zero = view.value_to_y(0.0);

        let (top, height) = if bar.value >= 0.0 {
            (y_val, y_zero - y_val)
        } else {
            (y_zero, y_val - y_zero)
        };

        if height > 0.0 {
            canvas.fill_rect(left, top, bar_width, height, bar.color);
        }
    }

    // Draw a subtle zero-line baseline
    let y_zero = view.value_to_y(0.0);
    let height = view.bounds_height();
    if y_zero >= 0.0 && y_zero <= height {
        canvas.stroke_line(
            0.0,
            y_zero,
            view.bounds_width(),
            y_zero,
            ZERO_LINE_COLOR,
            1.0,
            LineStyle::Solid,
        );
    }
}
