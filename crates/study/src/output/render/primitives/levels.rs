//! Price levels renderer.
//!
//! Renders horizontal price level lines (Fibonacci, Support/Resistance).

use super::super::canvas::Canvas;
use super::super::chart_view::ChartView;
use super::super::constants::{LABEL_SPACING, TINY_TEXT, ZONE_STRIP_ALPHAS, ZONE_STRIP_WIDTHS};
use super::super::types::{FontHint, LineStyle};
use crate::output::PriceLevel;

/// Render horizontal price levels.
pub fn render_levels(canvas: &mut dyn Canvas, levels: &[PriceLevel], view: &dyn ChartView) {
    let region = view.visible_region();
    let vis_left = region.x;
    let vis_right = region.x + region.width;
    let bounds_height = view.bounds_height();

    for level in levels {
        let y = view.value_to_y(level.price as f32);

        // Cull: level price is outside the visible vertical range
        let margin = 20.0;
        if y < -margin || y > bounds_height + margin {
            continue;
        }

        // When start_x is set, draw a ray from the anchor rightward.
        // Otherwise draw a full-width line.
        let left = match level.start_x {
            Some(x) => view.interval_to_x(x),
            None => vis_left - view.bounds_width(),
        };

        // Cull: ray starts past the right edge
        if left > vis_right {
            continue;
        }

        let right = match level.end_x {
            Some(x) => view.interval_to_x(x),
            None => vis_right + view.bounds_width(),
        };

        // Cull: bounded level ends before the visible area
        if right < vis_left {
            continue;
        }

        let raw_color = level.color;
        let color = raw_color.scale_alpha(level.opacity);

        // Fill above if configured
        if let Some((fill_color, fill_opacity)) = &level.fill_above {
            let fill = fill_color.scale_alpha(*fill_opacity);
            let top_y = -bounds_height;
            let fill_height = y - top_y;
            if fill_height > 0.0 {
                canvas.fill_rect(left, top_y, right - left, fill_height, fill);
            }
        }

        // Fill below if configured
        if let Some((fill_color, fill_opacity)) = &level.fill_below {
            let fill = fill_color.scale_alpha(*fill_opacity);
            canvas.fill_rect(left, y, right - left, bounds_height * 2.0, fill);
        }

        // Zone rendering
        let is_bounded_zone = level.zone_half_width.is_some() && level.end_x.is_some();

        if let Some(zone_hw) = level.zone_half_width {
            let y_above = view.value_to_y((level.price + zone_hw) as f32);
            let full_half = (y - y_above).abs().max(2.0);

            if is_bounded_zone {
                // Clean bounded rectangle: single fill + border
                let fill_color = raw_color.scale_alpha(0.18);
                canvas.fill_rect(
                    left,
                    y - full_half,
                    right - left,
                    full_half * 2.0,
                    fill_color,
                );
                canvas.stroke_rect(
                    left,
                    y - full_half,
                    right - left,
                    full_half * 2.0,
                    raw_color.scale_alpha(0.45),
                    1.0,
                );
            } else {
                // Feathered strips for level_analyzer rays
                for i in 0..ZONE_STRIP_WIDTHS.len() {
                    let strip_half = full_half * ZONE_STRIP_WIDTHS[i];
                    let strip_color = color.scale_alpha(ZONE_STRIP_ALPHAS[i]);
                    canvas.fill_rect(
                        left,
                        y - strip_half,
                        right - left,
                        strip_half * 2.0,
                        strip_color,
                    );
                }
            }
        }

        // Center line: skip for bounded zones (border is enough)
        if !is_bounded_zone {
            let (line_width, line_color) = if level.zone_half_width.is_some() {
                (0.5_f32, color.scale_alpha(0.35))
            } else {
                (level.width, color)
            };
            let style = LineStyle::from(&level.style);
            canvas.stroke_line(left, y, right, y, line_color, line_width, style);
        }

        // Draw label if enabled
        if level.show_label && !level.label.is_empty() {
            let label_x = left.max(vis_left) + 3.0;
            let (label_y, label_color) = if is_bounded_zone {
                let zone_hw = level.zone_half_width.unwrap();
                let y_above = view.value_to_y((level.price + zone_hw) as f32);
                let top = y - (y - y_above).abs().max(2.0);
                (top + 1.0, raw_color.scale_alpha(0.8))
            } else {
                (y - LABEL_SPACING, color)
            };
            canvas.fill_text(
                label_x,
                label_y,
                &level.label,
                TINY_TEXT,
                label_color,
                FontHint::Monospace,
            );
        }
    }
}
