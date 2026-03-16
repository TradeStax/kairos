//! Line series renderer.
//!
//! Renders `LineSeries` as connected polylines on the chart canvas.

use super::super::canvas::Canvas;
use super::super::chart_view::ChartView;
use super::super::coord::effective_line_width;
use super::super::types::LineStyle;
use crate::output::LineSeries;

/// Render one or more line series.
pub fn render_lines(canvas: &mut dyn Canvas, lines: &[LineSeries], view: &dyn ChartView) {
    for series in lines {
        if series.points.len() < 2 {
            continue;
        }

        let width = effective_line_width(series.width, view.scaling()).max(0.5);
        let style = LineStyle::from(&series.style);

        let points: Vec<(f32, f32)> = series
            .points
            .iter()
            .map(|&(x_val, y_val)| (view.interval_to_x(x_val), view.value_to_y(y_val)))
            .collect();

        canvas.stroke_polyline(&points, series.color, width, style);
    }
}
