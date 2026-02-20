//! Markers renderer
//!
//! Renders `TradeMarker` bubbles (Big Trades) on the chart overlay.
//! Each marker is drawn as a sized/colored circle at (time, vwap_price)
//! with an optional contract count label inside.

use crate::chart::ViewState;
use crate::components::primitives::AZERET_MONO;
use exchange::util::Price;
use iced::widget::canvas::{Frame, Path, Stroke, Text};
use iced::{Color, Point, Size};
use study::output::TradeMarker;

const MIN_RADIUS: f32 = 6.0;
const MAX_RADIUS: f32 = 40.0;
const BASE_RADIUS: f32 = 8.0;

/// Render trade marker bubbles on the chart.
pub fn render_markers(
    frame: &mut Frame,
    markers: &[TradeMarker],
    state: &ViewState,
    bounds: Size,
    bubble_scale: f32,
) {
    if markers.is_empty() {
        return;
    }

    let visible_region = state.visible_region(bounds);
    let (earliest, latest) = state.interval_range(&visible_region);

    // Visible-range normalization: compute max from only visible markers
    let max_contracts = markers
        .iter()
        .filter(|m| m.time >= earliest && m.time <= latest)
        .map(|m| m.contracts)
        .fold(0.0f64, f64::max);

    if max_contracts <= 0.0 {
        return;
    }

    for marker in markers {
        // Viewport cull by time range
        if marker.time < earliest || marker.time > latest {
            continue;
        }

        let x = state.interval_to_x(marker.time);
        let y = state.price_to_y(Price::from_units(marker.price));

        // Compute radius: sqrt normalization for perceptual scaling
        let norm = (marker.contracts / max_contracts).sqrt() as f32;
        let raw_radius =
            BASE_RADIUS + norm * (MAX_RADIUS - BASE_RADIUS) * bubble_scale;
        // Divide by scaling for consistent screen-pixel size
        let radius = (raw_radius / state.scaling).max(MIN_RADIUS / state.scaling);

        let center = Point::new(x, y);
        let color: Color = crate::style::theme_bridge::rgba_to_iced_color(marker.color);

        // Filled circle
        let circle = Path::circle(center, radius);
        frame.fill(&circle, color);

        // 1px border stroke with slightly stronger alpha
        let border_color = Color {
            a: (color.a + 0.2).min(1.0),
            ..color
        };
        let stroke = Stroke {
            width: 1.0 / state.scaling,
            ..Stroke::default()
        };
        frame.stroke(&circle, Stroke::with_color(stroke, border_color));

        // Label text
        if let Some(ref label) = marker.label {
            let font_size = (radius * state.scaling * 0.7).clamp(8.0, 14.0);
            let scaled_font_size = font_size / state.scaling;

            // Approximate centering: offset by half text width and height
            let text_width =
                label.len() as f32 * scaled_font_size * 0.6;
            let text_x = center.x - text_width / 2.0;
            let text_y = center.y - scaled_font_size / 2.0;

            frame.fill_text(Text {
                content: label.clone(),
                position: Point::new(text_x, text_y),
                size: iced::Pixels(scaled_font_size),
                color: Color::WHITE,
                font: AZERET_MONO,
                ..Text::default()
            });
        }

        // Debug annotations
        if let Some(ref debug) = marker.debug {
            let debug_font_size = 9.0 / state.scaling;
            let debug_y = center.y + radius + debug_font_size * 0.5;

            // Fill count and time window
            let window_ms =
                debug.last_fill_time.saturating_sub(debug.first_fill_time);
            let debug_text =
                format!("{} fills | {}ms", debug.fill_count, window_ms);
            let debug_width =
                debug_text.len() as f32 * debug_font_size * 0.6;
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
                let y_min =
                    state.price_to_y(Price::from_units(debug.price_min_units));
                let y_max =
                    state.price_to_y(Price::from_units(debug.price_max_units));
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
                    Stroke::with_color(range_stroke, Color {
                        r: 0.6,
                        g: 0.6,
                        b: 0.6,
                        a: 0.6,
                    }),
                );
            }
        }
    }
}
