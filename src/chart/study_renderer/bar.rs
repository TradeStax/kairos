//! Bar series renderer
//!
//! Renders `BarSeries` as vertical bars. For panel studies, Y maps to value range
//! (not price). Each bar is a rectangle from baseline to value.

use super::{value_range, value_to_panel_y};
use crate::chart::ViewState;
use exchange::util::Price;
use iced::widget::canvas::Frame;
use iced::{Color, Point, Size};
use study::StudyPlacement;
use study::output::BarSeries;

/// Render one or more bar series.
pub fn render_bars(
    frame: &mut Frame,
    bars: &[BarSeries],
    state: &ViewState,
    bounds: Size,
    placement: StudyPlacement,
) {
    if bars.is_empty() {
        return;
    }

    // For panel placement, compute value range from all points across all series
    let panel_range = if placement == StudyPlacement::Panel {
        let all_values = bars.iter().flat_map(|s| s.points.iter().map(|p| p.value));
        // Include zero in the range for bar charts
        let range = value_range(all_values);
        range.map(|(min, max)| (min.min(0.0), max))
    } else {
        None
    };

    let bar_width = state.cell_width * 0.8;

    for series in bars {
        for point in &series.points {
            let sx = state.interval_to_x(point.x);
            let left = sx - bar_width / 2.0;
            let color: Color = crate::style::theme_bridge::rgba_to_iced_color(point.color);

            match placement {
                StudyPlacement::Overlay | StudyPlacement::Background => {
                    let y_val = state.price_to_y(Price::from_f32_lossy(point.value));
                    let y_base = state.price_to_y(Price::from_f32_lossy(0.0));

                    let (top, height) = if y_val < y_base {
                        (y_val, y_base - y_val)
                    } else {
                        (y_base, y_val - y_base)
                    };

                    if height > 0.0 {
                        frame.fill_rectangle(
                            Point::new(left, top),
                            Size::new(bar_width, height),
                            color,
                        );
                    }
                }
                StudyPlacement::Panel => {
                    if let Some((min, max)) = panel_range {
                        let y_val = value_to_panel_y(point.value, min, max, bounds.height);
                        let y_base =
                            value_to_panel_y(0.0_f32.clamp(min, max), min, max, bounds.height);

                        let (top, height) = if y_val < y_base {
                            (y_val, y_base - y_val)
                        } else {
                            (y_base, y_val - y_base)
                        };

                        if height > 0.0 {
                            frame.fill_rectangle(
                                Point::new(left, top),
                                Size::new(bar_width, height),
                                color,
                            );
                        }
                    }
                }
            }

            // Render overlay (e.g. delta overlay on volume bars) if present
            if let Some(overlay_val) = point.overlay {
                let overlay_abs = overlay_val.abs();
                match placement {
                    StudyPlacement::Overlay | StudyPlacement::Background => {
                        let y_ov = state.price_to_y(Price::from_f32_lossy(overlay_abs));
                        let y_base = state.price_to_y(Price::from_f32_lossy(0.0));
                        let height = (y_base - y_ov).abs();

                        if height > 0.0 {
                            let top = y_ov.min(y_base);
                            frame.fill_rectangle(
                                Point::new(left, top),
                                Size::new(bar_width, height),
                                color.scale_alpha(0.7),
                            );
                        }
                    }
                    StudyPlacement::Panel => {
                        if let Some((min, max)) = panel_range {
                            let y_ov = value_to_panel_y(overlay_abs, min, max, bounds.height);
                            let y_base =
                                value_to_panel_y(0.0_f32.clamp(min, max), min, max, bounds.height);
                            let height = (y_base - y_ov).abs();

                            if height > 0.0 {
                                let top = y_ov.min(y_base);
                                frame.fill_rectangle(
                                    Point::new(left, top),
                                    Size::new(bar_width, height),
                                    color.scale_alpha(0.7),
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
