//! Bar series renderer.
//!
//! Renders `BarSeries` as vertical bars from baseline to value.

use super::super::canvas::Canvas;
use super::super::chart_view::ChartView;
use crate::output::BarSeries;

/// Render one or more bar series.
pub fn render_bars(canvas: &mut dyn Canvas, bars: &[BarSeries], view: &dyn ChartView) {
    if bars.is_empty() {
        return;
    }

    let bar_width = view.cell_width() * 0.8;

    for series in bars {
        for point in &series.points {
            let sx = view.interval_to_x(point.x);
            let left = sx - bar_width / 2.0;

            let y_val = view.value_to_y(point.value);
            let y_base = view.value_to_y(0.0);

            let (top, height) = if y_val < y_base {
                (y_val, y_base - y_val)
            } else {
                (y_base, y_val - y_base)
            };

            if height > 0.0 {
                canvas.fill_rect(left, top, bar_width, height, point.color);
            }

            // Render overlay (e.g. delta overlay on volume bars)
            if let Some(overlay_val) = point.overlay {
                let overlay_abs = overlay_val.abs();
                let y_ov = view.value_to_y(overlay_abs);
                let ov_height = (y_base - y_ov).abs();

                if ov_height > 0.0 {
                    let ov_top = y_ov.min(y_base);
                    canvas.fill_rect(
                        left,
                        ov_top,
                        bar_width,
                        ov_height,
                        point.color.scale_alpha(0.7),
                    );
                }
            }
        }
    }
}
