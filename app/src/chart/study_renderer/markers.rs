//! Markers renderer
//!
//! Renders `TradeMarker` bubbles (Big Trades) on the chart overlay.
//! Each marker is drawn as a sized/colored shape at (time, vwap_price)
//! with an optional contract count label. Supports circle, square, and
//! text-only shapes with linear contract-based sizing and per-marker
//! opacity.

use crate::chart::ViewState;
use crate::components::primitives::AZERET_MONO;
use exchange::util::Price;
use iced::widget::canvas::{Frame, Path, Stroke, Text};
use iced::{Color, Point, Size};
use study::output::{MarkerRenderConfig, MarkerShape, TradeMarker};

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

    for marker in markers {
        // Viewport cull by time range
        if marker.time < earliest || marker.time > latest {
            continue;
        }

        let x = state.interval_to_x(marker.time);
        let y = state.price_to_y(Price::from_units(marker.price));

        // Linear sizing: map contracts in [scale_min, scale_max] → [0, 1]
        let t = if scale_range > 0.0 {
            ((marker.contracts - config.scale_min) / scale_range)
                .clamp(0.0, 1.0) as f32
        } else {
            0.5
        };

        let radius = lerp(config.min_size, config.max_size, t);
        // Divide by scaling for consistent screen-pixel size
        let radius = radius / state.scaling;

        // Per-marker opacity
        let opacity = lerp(config.min_opacity, config.max_opacity, t);
        let base_color: Color =
            crate::style::theme_bridge::rgba_to_iced_color(marker.color);
        let color = Color { a: opacity, ..base_color };

        let center = Point::new(x, y);

        // Shape rendering
        match config.shape {
            MarkerShape::Circle => {
                let circle = Path::circle(center, radius);
                if config.hollow {
                    let stroke = Stroke {
                        width: 2.0 / state.scaling,
                        ..Stroke::default()
                    };
                    frame.stroke(
                        &circle,
                        Stroke::with_color(stroke, color),
                    );
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
                    frame.stroke(
                        &circle,
                        Stroke::with_color(stroke, border_color),
                    );
                }
            }
            MarkerShape::Square => {
                let side = radius * 2.0;
                let top_left = Point::new(
                    center.x - radius,
                    center.y - radius,
                );
                let rect = Path::rectangle(
                    top_left,
                    Size::new(side, side),
                );
                if config.hollow {
                    let stroke = Stroke {
                        width: 2.0 / state.scaling,
                        ..Stroke::default()
                    };
                    frame.stroke(
                        &rect,
                        Stroke::with_color(stroke, color),
                    );
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
                    frame.stroke(
                        &rect,
                        Stroke::with_color(stroke, border_color),
                    );
                }
            }
            MarkerShape::TextOnly => {
                // No shape drawn, only the text label below
            }
        }

        // Text label
        if config.show_text
            && let Some(ref label) = marker.label
        {
            let font_size = config.text_size / state.scaling;
            let text_color: Color =
                crate::style::theme_bridge::rgba_to_iced_color(
                    config.text_color,
                );

            // Approximate centering
            let text_width =
                label.len() as f32 * font_size * 0.6;
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

        // Debug annotations
        if let Some(ref debug) = marker.debug {
            let debug_font_size = 9.0 / state.scaling;
            let debug_y = center.y + radius + debug_font_size * 0.5;

            // Fill count and time window
            let window_ms = debug
                .last_fill_time
                .saturating_sub(debug.first_fill_time);
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
                let y_min = state
                    .price_to_y(Price::from_units(debug.price_min_units));
                let y_max = state
                    .price_to_y(Price::from_units(debug.price_max_units));
                let range_line = Path::line(
                    Point::new(
                        center.x + radius + 2.0 / state.scaling,
                        y_max,
                    ),
                    Point::new(
                        center.x + radius + 2.0 / state.scaling,
                        y_min,
                    ),
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

/// Linear interpolation between two values.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
