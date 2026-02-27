//! Markers renderer
//!
//! Renders `TradeMarker` bubbles (Big Trades) on the chart overlay.
//! Each marker is drawn as a sized/colored shape at (time, vwap_price)
//! with an optional contract count label. Supports circle, square, and
//! text-only shapes with linear contract-based sizing and per-marker
//! opacity.
//!
//! Scaling uses the geometric mean of X and Y zoom factors so markers
//! respond to both horizontal and vertical zoom. A density grid fades
//! overlapping markers to reduce visual clutter in fast markets.

use std::collections::HashMap;

use crate::chart::ViewState;
use crate::components::primitives::AZERET_MONO;
use data::Price;
use iced::widget::canvas::{Frame, Path, Stroke, Text};
use iced::{Color, Point, Size};
use study::output::{MarkerRenderConfig, MarkerShape, TradeMarker};

/// Reference cell_width at default candlestick zoom.
const REFERENCE_CELL_WIDTH: f32 = 4.0;
/// Reference cell_height at default candlestick zoom.
/// Typical cell_height for ES ≈ 200.0 / y_ticks ≈ 1.0 at default zoom.
const REFERENCE_CELL_HEIGHT: f32 = 1.0;
/// Maximum marker radius in X-axis cell widths (~12 candle diameter).
const MAX_RADIUS_CELLS_X: f32 = 6.0;
/// Maximum marker radius in Y-axis cell heights (~80 ticks diameter).
const MAX_RADIUS_CELLS_Y: f32 = 40.0;
/// Minimum marker radius in screen pixels (visibility floor).
const MIN_RADIUS_SCREEN_PX: f32 = 3.0;
/// Maximum marker radius in screen pixels (absolute ceiling).
const MAX_RADIUS_SCREEN_PX: f32 = 60.0;
/// Minimum screen-pixel font size for text legibility.
const MIN_TEXT_SCREEN_PX: f32 = 9.0;
/// Coarse grid bucket size (screen pixels) for density detection.
const DENSITY_BUCKET_PX: f32 = 40.0;

/// Pre-computed visible marker data for the two-pass structure.
struct VisibleMarker {
    index: usize,
    screen_x: f32,
    screen_y: f32,
}

/// Render trade marker bubbles on the chart.
pub fn render_markers(
    frame: &mut Frame,
    markers: &[TradeMarker],
    state: &ViewState,
    bounds: Size,
    config: &MarkerRenderConfig,
) {
    if markers.is_empty() {
        return;
    }

    let visible_region = state.visible_region(bounds);
    let (earliest, latest) = state.interval_range(&visible_region);

    // Linear scale range from the study's filter parameters
    let scale_range = config.scale_max - config.scale_min;

    // Biaxial zoom factor: geometric mean of X and Y zoom
    let x_zoom = state.cell_width / REFERENCE_CELL_WIDTH;
    let y_zoom = state.cell_height / REFERENCE_CELL_HEIGHT;
    let zoom_factor = (x_zoom * y_zoom).sqrt();

    // Biaxial clamping bounds
    let max_chart_radius =
        (MAX_RADIUS_CELLS_X * state.cell_width).min(MAX_RADIUS_CELLS_Y * state.cell_height);
    let min_chart_radius = MIN_RADIUS_SCREEN_PX / state.scaling;
    let max_screen_radius = MAX_RADIUS_SCREEN_PX / state.scaling;

    // --- Pass 1: Visibility culling + density grid ---
    let mut visible: Vec<VisibleMarker> = Vec::new();
    let mut density: HashMap<(i32, i32), u32> = HashMap::new();

    for (i, marker) in markers.iter().enumerate() {
        if marker.time < earliest || marker.time > latest {
            continue;
        }

        let x = state.interval_to_x(marker.time);
        let y = state.price_to_y(Price::from_units(marker.price));

        // Screen-space position for density bucketing
        let sx = x * state.scaling;
        let sy = y * state.scaling;
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

        let x = vm.screen_x / state.scaling;
        let y = vm.screen_y / state.scaling;

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
        let radius = (base_radius / state.scaling) * zoom_factor;
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

        let base_color: Color = crate::style::theme::rgba_to_iced_color(marker.color);
        let color = Color {
            a: opacity,
            ..base_color
        };

        let center = Point::new(x, y);

        // Per-marker shape override or config-level default
        let shape = marker.shape_override.unwrap_or(config.shape);

        // Shape rendering
        match shape {
            MarkerShape::Circle => {
                let circle = Path::circle(center, radius);
                if config.hollow {
                    let stroke = Stroke {
                        width: 2.0 / state.scaling,
                        ..Stroke::default()
                    };
                    frame.stroke(&circle, Stroke::with_color(stroke, color));
                } else {
                    frame.fill(&circle, color);
                    // 1px border stroke
                    let border_color = Color {
                        a: (color.a + 0.2).min(1.0),
                        ..color
                    };
                    let stroke = Stroke {
                        width: 1.0 / state.scaling,
                        ..Stroke::default()
                    };
                    frame.stroke(&circle, Stroke::with_color(stroke, border_color));
                }
            }
            MarkerShape::Square => {
                let side = radius * 2.0;
                let top_left = Point::new(center.x - radius, center.y - radius);
                let rect = Path::rectangle(top_left, Size::new(side, side));
                if config.hollow {
                    let stroke = Stroke {
                        width: 2.0 / state.scaling,
                        ..Stroke::default()
                    };
                    frame.stroke(&rect, Stroke::with_color(stroke, color));
                } else {
                    frame.fill(&rect, color);
                    let border_color = Color {
                        a: (color.a + 0.2).min(1.0),
                        ..color
                    };
                    let stroke = Stroke {
                        width: 1.0 / state.scaling,
                        ..Stroke::default()
                    };
                    frame.stroke(&rect, Stroke::with_color(stroke, border_color));
                }
            }
            MarkerShape::Cross => {
                // Small crosshair: horizontal + vertical lines
                let arm = radius * 0.7;
                let stroke = Stroke {
                    width: (1.5 / state.scaling).max(0.5),
                    ..Stroke::default()
                };
                let h_line = Path::line(
                    Point::new(center.x - arm, center.y),
                    Point::new(center.x + arm, center.y),
                );
                let v_line = Path::line(
                    Point::new(center.x, center.y - arm),
                    Point::new(center.x, center.y + arm),
                );
                frame.stroke(
                    &h_line,
                    Stroke::with_color(stroke, color),
                );
                frame.stroke(
                    &v_line,
                    Stroke::with_color(stroke, color),
                );
            }
            MarkerShape::TextOnly => {
                // No shape drawn, only the text label below
            }
        }

        // Text label — scale with marker, hide if it won't fit
        if config.show_text
            && let Some(ref label) = marker.label
        {
            // Scale font to fit inside the marker's diameter.
            // Use ~60% of char width as the monospace advance ratio.
            let char_count = label.len().max(1) as f32;
            let max_font_for_width = (radius * 2.0 * 0.85) / (char_count * 0.6);
            let max_font_for_height = radius * 1.3;
            let font_size = max_font_for_width
                .min(max_font_for_height)
                .min(config.text_size / state.scaling);

            // Only render if the computed font is legible on screen
            let effective_px = font_size * state.scaling;
            if effective_px >= MIN_TEXT_SCREEN_PX {
                let text_color: Color = crate::style::theme::rgba_to_iced_color(config.text_color);

                let text_width = char_count * font_size * 0.6;
                let text_x = center.x - text_width / 2.0;
                let text_y = center.y - font_size / 2.0;

                frame.fill_text(Text {
                    content: label.clone(),
                    position: Point::new(text_x, text_y),
                    size: iced::Pixels(font_size),
                    color: text_color,
                    font: AZERET_MONO,
                    ..Text::default()
                });
            }
        }

        // Debug annotations
        if let Some(ref debug) = marker.debug {
            let debug_font_size = 9.0 / state.scaling;
            let debug_y = center.y + radius + debug_font_size * 0.5;

            // Fill count and time window
            let window_ms = debug.last_fill_time.saturating_sub(debug.first_fill_time);
            let debug_text = format!("{} fills | {}ms", debug.fill_count, window_ms);
            let debug_width = debug_text.len() as f32 * debug_font_size * 0.6;
            let debug_x = center.x - debug_width / 2.0;

            frame.fill_text(Text {
                content: debug_text,
                position: Point::new(debug_x, debug_y),
                size: iced::Pixels(debug_font_size),
                color: Color {
                    r: 0.7,
                    g: 0.7,
                    b: 0.7,
                    a: 0.8,
                },
                font: AZERET_MONO,
                ..Text::default()
            });

            // Price range line (thin vertical from min to max price)
            if debug.price_min_units != debug.price_max_units {
                let y_min = state.price_to_y(Price::from_units(debug.price_min_units));
                let y_max = state.price_to_y(Price::from_units(debug.price_max_units));
                let range_line = Path::line(
                    Point::new(center.x + radius + 2.0 / state.scaling, y_max),
                    Point::new(center.x + radius + 2.0 / state.scaling, y_min),
                );
                let range_stroke = Stroke {
                    width: 1.0 / state.scaling,
                    ..Stroke::default()
                };
                frame.stroke(
                    &range_line,
                    Stroke::with_color(
                        range_stroke,
                        Color {
                            r: 0.6,
                            g: 0.6,
                            b: 0.6,
                            a: 0.6,
                        },
                    ),
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
