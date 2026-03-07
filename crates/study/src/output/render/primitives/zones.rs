//! Zone renderer.
//!
//! Renders bounded rectangular zones (e.g. absorption zones).

use super::super::canvas::Canvas;
use super::super::chart_view::ChartView;
use super::super::constants::TINY_TEXT;
use super::super::types::FontHint;
use crate::output::ZoneRect;

/// Render bounded rectangular zones.
pub fn render_zones(
    canvas: &mut dyn Canvas,
    zones: &[ZoneRect],
    view: &dyn ChartView,
) {
    let region = view.visible_region();
    let vis_left = region.x;
    let vis_right = region.x + region.width;
    let bounds_height = view.bounds_height();

    for zone in zones {
        let left_x = view.interval_to_x(zone.start_x);
        let right_x = view.interval_to_x(zone.end_x);
        let span = (right_x - left_x).abs();
        let (left, right) = if span < 40.0 {
            let half_zone = span / 2.0;
            (left_x - half_zone, right_x + half_zone)
        } else {
            (left_x.min(right_x), left_x.max(right_x))
        };

        // Cull: entirely off-screen
        if left > vis_right || right < vis_left {
            continue;
        }

        let center_y =
            view.value_to_y(zone.center_price as f32);
        let edge_y = view
            .value_to_y((zone.center_price + zone.half_height) as f32);
        let half_px = (center_y - edge_y).abs().max(2.0);

        // Cull: zone is entirely above or below visible area
        if center_y + half_px < -20.0
            || center_y - half_px > bounds_height + 20.0
        {
            continue;
        }

        let raw_color = zone.color;

        // Fill
        canvas.fill_rect(
            left,
            center_y - half_px,
            right - left,
            half_px * 2.0,
            raw_color.scale_alpha(zone.fill_opacity),
        );

        // Border
        canvas.stroke_rect(
            left,
            center_y - half_px,
            right - left,
            half_px * 2.0,
            raw_color.scale_alpha(zone.border_opacity),
            1.0,
        );

        // Label
        if zone.show_label && !zone.label.is_empty() {
            let label_x = left.max(vis_left) + 3.0;
            let top = center_y - half_px;
            canvas.fill_text(
                label_x,
                top + 1.0,
                &zone.label,
                TINY_TEXT,
                raw_color.scale_alpha(zone.label_opacity),
                FontHint::Monospace,
            );
        }
    }
}
