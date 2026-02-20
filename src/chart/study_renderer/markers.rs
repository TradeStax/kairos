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

    // Find max contracts for normalization
    let max_contracts = markers
        .iter()
        .map(|m| m.contracts)
        .fold(0.0f64, f64::max);

    if max_contracts <= 0.0 {
        return;
    }

    let visible_region = state.visible_region(bounds);
    let (earliest, latest) = state.interval_range(&visible_region);

    for marker in markers {
        // Viewport cull by time range
        if marker.time < earliest || marker.time > latest {
            continue;
        }

        let x = state.interval_to_x(marker.time);
        let y = state.price_to_y(Price::from_f32_lossy(marker.price as f32));

        // Compute radius: sqrt normalization for perceptual scaling
        let norm = (marker.contracts / max_contracts).sqrt() as f32;
        let raw_radius =
            BASE_RADIUS + norm * (MAX_RADIUS - BASE_RADIUS) * bubble_scale;
        // Divide by scaling for consistent screen-pixel size
        let radius = (raw_radius / state.scaling).max(MIN_RADIUS / state.scaling);

        let center = Point::new(x, y);
        let color: Color = marker.color.into();

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
    }
}
