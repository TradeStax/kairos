//! Study candle renderer — OHLC mini-candlesticks for study output.

use super::super::canvas::Canvas;
use super::super::chart_view::ChartView;
use super::super::types::LineStyle;
use crate::output::StudyCandleSeries;

/// Render study candle series onto a chart canvas.
pub fn render_study_candles(
    canvas: &mut dyn Canvas,
    series: &[StudyCandleSeries],
    view: &dyn ChartView,
) {
    if series.is_empty() {
        return;
    }

    let bar_width = view.cell_width() * 0.8;
    let half_w = bar_width / 2.0;

    for s in series {
        for pt in &s.points {
            let sx = view.interval_to_x(pt.x);

            let y_high = view.value_to_y(pt.high);
            let y_low = view.value_to_y(pt.low);
            let y_open = view.value_to_y(pt.open);
            let y_close = view.value_to_y(pt.close);

            // Wick: high -> low
            if (y_low - y_high).abs() > 0.0 {
                canvas.stroke_line(
                    sx,
                    y_high,
                    sx,
                    y_low,
                    pt.border_color,
                    1.0,
                    LineStyle::Solid,
                );
            }

            // Body: open -> close
            let body_top = y_open.min(y_close);
            let body_h = (y_open - y_close).abs().max(1.0);

            if bar_width > 0.0 {
                canvas.fill_rect(
                    sx - half_w,
                    body_top,
                    bar_width,
                    body_h,
                    pt.body_color,
                );
            }
        }
    }
}
