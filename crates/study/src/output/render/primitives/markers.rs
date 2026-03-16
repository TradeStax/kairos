//! Trade markers renderer.
//!
//! Renders `TradeMarker` bubbles (Big Trades) on the chart overlay.
//! Each marker is drawn as a sized/colored shape at (time, vwap_price)
//! with an optional contract count label. Supports circle, square, cross,
//! and text-only shapes with linear contract-based sizing and per-marker
//! opacity.
//!
//! Scaling uses the geometric mean of X and Y zoom factors so markers
//! respond to both horizontal and vertical zoom. A density grid fades
//! overlapping markers to reduce visual clutter in fast markets.

use std::collections::HashMap;

use super::super::canvas::Canvas;
use super::super::chart_view::ChartView;
use super::super::constants::{
    DENSITY_BUCKET_PX, MAX_RADIUS_CELLS_X, MAX_RADIUS_CELLS_Y, MAX_RADIUS_SCREEN_PX,
    MIN_RADIUS_SCREEN_PX, MIN_TEXT_SCREEN_PX, REFERENCE_CELL_HEIGHT, REFERENCE_CELL_WIDTH,
};
use super::super::types::{FontHint, LineStyle};
use crate::output::{MarkerRenderConfig, MarkerShape, TradeMarker};
use data::Rgba;

/// Pre-computed visible marker data for the two-pass structure.
struct VisibleMarker {
    index: usize,
    screen_x: f32,
    screen_y: f32,
}

/// Render trade marker bubbles on the chart.
pub fn render_markers(
    canvas: &mut dyn Canvas,
    markers: &[TradeMarker],
    view: &dyn ChartView,
    config: &MarkerRenderConfig,
) {
    if markers.is_empty() {
        return;
    }

    let (earliest, latest) = view.visible_intervals();
    let scaling = view.scaling();
    let cell_width = view.cell_width();
    let cell_height = view.cell_height();

    // Linear scale range from the study's filter parameters
    let scale_range = config.scale_max - config.scale_min;

    // Biaxial zoom factor: geometric mean of X and Y zoom
    let x_zoom = cell_width / REFERENCE_CELL_WIDTH;
    let y_zoom = cell_height / REFERENCE_CELL_HEIGHT;
    let zoom_factor = (x_zoom * y_zoom).sqrt();

    // Biaxial clamping bounds
    let max_chart_radius = (MAX_RADIUS_CELLS_X * cell_width).min(MAX_RADIUS_CELLS_Y * cell_height);
    let min_chart_radius = MIN_RADIUS_SCREEN_PX / scaling;
    let max_screen_radius = MAX_RADIUS_SCREEN_PX / scaling;

    // --- Pass 1: Visibility culling + density grid ---
    let mut visible: Vec<VisibleMarker> = Vec::new();
    let mut density: HashMap<(i32, i32), u32> = HashMap::new();

    for (i, marker) in markers.iter().enumerate() {
        if marker.time < earliest || marker.time > latest {
            continue;
        }

        let x = view.interval_to_x(marker.time);
        let y = view.price_units_to_y(marker.price);

        // Screen-space position for density bucketing
        let sx = x * scaling;
        let sy = y * scaling;
        let bx = (sx / DENSITY_BUCKET_PX) as i32;
        let by = (sy / DENSITY_BUCKET_PX) as i32;
        *density.entry((bx, by)).or_insert(0) += 1;

        visible.push(VisibleMarker {
            index: i,
            screen_x: sx,
            screen_y: sy,
        });
    }

    // --- Pass 2: Render with density attenuation ---
    for vm in &visible {
        let marker = &markers[vm.index];

        let x = vm.screen_x / scaling;
        let y = vm.screen_y / scaling;

        // Area-proportional sizing: sqrt gives perceptually linear
        // area growth (standard bubble chart approach)
        let t_linear = if scale_range > 0.0 {
            ((marker.contracts - config.scale_min) / scale_range).clamp(0.0, 1.0) as f32
        } else {
            0.5
        };
        let t = t_linear.sqrt();

        let base_radius = lerp(config.min_size, config.max_size, t);
        // Scale by biaxial zoom and divide by scaling for canvas coords
        let radius = (base_radius / scaling) * zoom_factor;
        // Biaxial + screen-pixel clamping
        let radius = radius
            .max(min_chart_radius)
            .min(max_chart_radius)
            .min(max_screen_radius);

        // Per-marker opacity with density attenuation
        let base_opacity = lerp(config.min_opacity, config.max_opacity, t_linear);

        let bx = (vm.screen_x / DENSITY_BUCKET_PX) as i32;
        let by = (vm.screen_y / DENSITY_BUCKET_PX) as i32;
        let neighborhood_density = neighborhood_count(&density, bx, by);
        let density_factor = if neighborhood_density > 2 {
            (1.0 / (neighborhood_density as f32).sqrt()).max(0.4)
        } else {
            1.0
        };
        let opacity = base_opacity * density_factor;

        let color = marker.color.with_alpha(opacity);

        // Per-marker shape override or config-level default
        let shape = marker.shape_override.unwrap_or(config.shape);

        // Shape rendering
        match shape {
            MarkerShape::Circle => {
                if config.hollow {
                    let w = 2.0 / scaling;
                    canvas.stroke_circle(x, y, radius, color, w);
                } else {
                    canvas.fill_circle(x, y, radius, color);
                    let border_color = color.with_alpha((color.a + 0.2).min(1.0));
                    let w = 1.0 / scaling;
                    canvas.stroke_circle(x, y, radius, border_color, w);
                }
            }
            MarkerShape::Square => {
                let side = radius * 2.0;
                let top_left_x = x - radius;
                let top_left_y = y - radius;
                if config.hollow {
                    let w = 2.0 / scaling;
                    canvas.stroke_rect(top_left_x, top_left_y, side, side, color, w);
                } else {
                    canvas.fill_rect(top_left_x, top_left_y, side, side, color);
                    let border_color = color.with_alpha((color.a + 0.2).min(1.0));
                    let w = 1.0 / scaling;
                    canvas.stroke_rect(top_left_x, top_left_y, side, side, border_color, w);
                }
            }
            MarkerShape::Cross => {
                let arm = radius * 0.7;
                let w = (1.5 / scaling).max(0.5);
                canvas.stroke_line(x - arm, y, x + arm, y, color, w, LineStyle::Solid);
                canvas.stroke_line(x, y - arm, x, y + arm, color, w, LineStyle::Solid);
            }
            MarkerShape::TextOnly => {}
        }

        // Text label
        if config.show_text
            && let Some(ref label) = marker.label
        {
            let char_count = label.len().max(1) as f32;
            let max_font_for_width = (radius * 2.0 * 0.85) / (char_count * 0.6);
            let max_font_for_height = radius * 1.3;
            let font_size = max_font_for_width
                .min(max_font_for_height)
                .min(config.text_size / scaling);

            let effective_px = font_size * scaling;
            if effective_px >= MIN_TEXT_SCREEN_PX {
                let text_color = config.text_color;

                let text_width = char_count * font_size * 0.6;
                let text_x = x - text_width / 2.0;
                let text_y = y - font_size / 2.0;

                canvas.fill_text(
                    text_x,
                    text_y,
                    label,
                    font_size,
                    text_color,
                    FontHint::Monospace,
                );
            }
        }

        // Debug annotations
        if let Some(ref debug) = marker.debug {
            let debug_font_size = 9.0 / scaling;
            let debug_y = y + radius + debug_font_size * 0.5;

            let window_ms = debug.last_fill_time.saturating_sub(debug.first_fill_time);
            let debug_text = format!("{} fills | {}ms", debug.fill_count, window_ms);
            let debug_width = debug_text.len() as f32 * debug_font_size * 0.6;
            let debug_x = x - debug_width / 2.0;

            let debug_color = Rgba::new(0.7, 0.7, 0.7, 0.8);
            canvas.fill_text(
                debug_x,
                debug_y,
                &debug_text,
                debug_font_size,
                debug_color,
                FontHint::Monospace,
            );

            // Price range line (thin vertical from min to max price)
            if debug.price_min_units != debug.price_max_units {
                let y_min = view.price_units_to_y(debug.price_min_units);
                let y_max = view.price_units_to_y(debug.price_max_units);
                let range_x = x + radius + 2.0 / scaling;
                let range_color = Rgba::new(0.6, 0.6, 0.6, 0.6);
                let w = 1.0 / scaling;
                canvas.stroke_line(
                    range_x,
                    y_max,
                    range_x,
                    y_min,
                    range_color,
                    w,
                    LineStyle::Solid,
                );
            }
        }
    }
}

/// Sum marker counts in the 3x3 neighborhood around (bx, by).
fn neighborhood_count(density: &HashMap<(i32, i32), u32>, bx: i32, by: i32) -> u32 {
    let mut count = 0u32;
    for dx in -1..=1 {
        for dy in -1..=1 {
            if let Some(&c) = density.get(&(bx + dx, by + dy)) {
                count += c;
            }
        }
    }
    count
}

/// Linear interpolation between two values.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
