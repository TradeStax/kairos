//! Study Panel Canvas
//!
//! Renders panel-placement studies (RSI, MACD, Volume, etc.) in a
//! separate canvas below the main chart. Coordinates are in screen
//! space: X is derived from the chart's interval-to-x mapping with
//! the current pan/zoom applied; Y is mapped from the study's value
//! range to the panel's pixel height.

use super::coord;
use crate::chart::core::PanelStudyInfo;
use crate::chart::scale::{AxisLabel, linear};
use crate::chart::{Message, ViewState};
use crate::components::primitives::AZERET_MONO;
use iced::theme::palette::Extended;
use iced::widget::canvas::{self, Cache, Event, Geometry, Path, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, mouse};
use study::output::{BarSeries, HistogramBar, LineSeries, StudyOutput};

/// Canvas program that renders panel studies in screen coordinates.
pub struct StudyPanelCanvas<'a> {
    pub panels: Vec<PanelStudyInfo<'a>>,
    pub state: &'a ViewState,
    pub cache: &'a Cache,
}

impl<'a> canvas::Program<Message> for StudyPanelCanvas<'a> {
    type State = ();

    fn update(
        &self,
        _state: &mut (),
        _event: &Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        None
    }

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        if self.panels.is_empty() {
            return vec![];
        }

        let palette = theme.extended_palette();
        let geo = self.cache.draw(renderer, bounds.size(), |frame| {
            let num = self.panels.len();
            let panel_h = bounds.height / num as f32;

            for (i, panel) in self.panels.iter().enumerate() {
                let y_off = i as f32 * panel_h;

                // Separator between panels
                if i > 0 {
                    frame.fill_rectangle(
                        Point::new(0.0, y_off),
                        Size::new(bounds.width, 1.0),
                        palette.background.strong.color,
                    );
                }

                render_panel_content(
                    frame,
                    panel.output,
                    self.state,
                    bounds.width,
                    y_off,
                    panel_h,
                    palette,
                );

                // Study name label
                frame.fill_text(canvas::Text {
                    content: panel.name.to_string(),
                    position: Point::new(4.0, y_off + 2.0),
                    size: iced::Pixels(10.0),
                    color: palette.background.base.text.scale_alpha(0.5),
                    font: AZERET_MONO,
                    ..canvas::Text::default()
                });
            }
        });
        vec![geo]
    }
}

// ── Coordinate helpers ───────────────────────────────────────────────

/// Convert chart interval to screen X pixel.
fn interval_to_screen_x(
    state: &ViewState,
    interval: u64,
    canvas_width: f32,
) -> f32 {
    let chart_x = state.interval_to_x(interval);
    (chart_x + state.translation.x) * state.scaling + canvas_width / 2.0
}

/// Compute the value range for a panel study output.
///
/// Extracts all relevant values from the `StudyOutput` and returns
/// the (min, max) with 5% padding. Bars/Histogram ranges are floored
/// at 0.0 so the baseline is always visible.
pub fn panel_value_range(output: &StudyOutput) -> Option<(f32, f32)> {
    match output {
        StudyOutput::Lines(lines) => {
            coord::value_range(
                lines
                    .iter()
                    .flat_map(|s| s.points.iter().map(|(_, v)| *v)),
            )
        }
        StudyOutput::Bars(bars) => {
            coord::value_range(
                bars.iter()
                    .flat_map(|s| s.points.iter().map(|p| p.value)),
            )
            .map(|(lo, hi)| (lo.min(0.0), hi))
        }
        StudyOutput::Histogram(bars) => {
            coord::value_range(bars.iter().map(|b| b.value))
                .map(|(lo, hi)| (lo.min(0.0), hi.max(0.0)))
        }
        StudyOutput::Band {
            upper,
            middle,
            lower,
            ..
        } => coord::value_range(
            upper
                .points
                .iter()
                .chain(lower.points.iter())
                .chain(middle.iter().flat_map(|m| m.points.iter()))
                .map(|(_, v)| *v),
        ),
        _ => None,
    }
}

/// Map a study value to a Y pixel inside a panel.
#[inline]
fn value_to_y(
    value: f32,
    min: f32,
    max: f32,
    panel_height: f32,
    y_offset: f32,
) -> f32 {
    y_offset
        + coord::value_to_panel_y(value, min, max, panel_height)
}

// ── Dispatch ─────────────────────────────────────────────────────────

fn render_panel_content(
    frame: &mut canvas::Frame,
    output: &StudyOutput,
    state: &ViewState,
    w: f32,
    y_off: f32,
    h: f32,
    _palette: &Extended,
) {
    match output {
        StudyOutput::Lines(lines) => {
            render_panel_lines(frame, lines, state, w, y_off, h);
        }
        StudyOutput::Bars(bars) => {
            render_panel_bars(frame, bars, state, w, y_off, h);
        }
        StudyOutput::Histogram(bars) => {
            render_panel_histogram(frame, bars, state, w, y_off, h);
        }
        StudyOutput::Band {
            upper,
            middle,
            lower,
            ..
        } => {
            // Render band lines (no fill for simplicity in panels)
            let mut all: Vec<&LineSeries> = vec![upper, lower];
            if let Some(m) = middle.as_ref() {
                all.push(m);
            }
            render_panel_lines(frame, &[], state, w, y_off, h);
            // Draw each series individually with shared range
            let all_values = upper
                .points
                .iter()
                .chain(lower.points.iter())
                .chain(
                    middle
                        .iter()
                        .flat_map(|m| m.points.iter()),
                )
                .map(|(_, v)| *v);
            if let Some(range) = coord::value_range(all_values) {
                for series in all {
                    render_line_with_range(
                        frame, series, state, w, y_off, h, range,
                    );
                }
            }
        }
        _ => {}
    }
}

// ── Lines ────────────────────────────────────────────────────────────

fn render_panel_lines(
    frame: &mut canvas::Frame,
    lines: &[LineSeries],
    state: &ViewState,
    canvas_width: f32,
    y_offset: f32,
    panel_height: f32,
) {
    if lines.is_empty() {
        return;
    }

    let all_values =
        lines.iter().flat_map(|s| s.points.iter().map(|(_, v)| *v));
    let range = match coord::value_range(all_values) {
        Some(r) => r,
        None => return,
    };

    for series in lines {
        render_line_with_range(
            frame,
            series,
            state,
            canvas_width,
            y_offset,
            panel_height,
            range,
        );
    }
}

fn render_line_with_range(
    frame: &mut canvas::Frame,
    series: &LineSeries,
    state: &ViewState,
    canvas_width: f32,
    y_offset: f32,
    panel_height: f32,
    range: (f32, f32),
) {
    if series.points.len() < 2 {
        return;
    }

    let color: Color =
        crate::style::theme_bridge::rgba_to_iced_color(series.color);
    let stroke = Stroke::with_color(
        Stroke {
            width: series.width,
            line_dash: coord::line_dash_for_style(&series.style),
            ..Stroke::default()
        },
        color,
    );

    let mut prev: Option<Point> = None;
    for &(x_val, y_val) in &series.points {
        let sx = interval_to_screen_x(state, x_val, canvas_width);
        let sy =
            value_to_y(y_val, range.0, range.1, panel_height, y_offset);
        let pt = Point::new(sx, sy);
        if let Some(p) = prev {
            frame.stroke(&Path::line(p, pt), stroke);
        }
        prev = Some(pt);
    }
}

// ── Bars ─────────────────────────────────────────────────────────────

fn render_panel_bars(
    frame: &mut canvas::Frame,
    bars: &[BarSeries],
    state: &ViewState,
    canvas_width: f32,
    y_offset: f32,
    panel_height: f32,
) {
    if bars.is_empty() {
        return;
    }

    let all_values =
        bars.iter().flat_map(|s| s.points.iter().map(|p| p.value));
    let range = match coord::value_range(all_values) {
        Some(r) => (r.0.min(0.0), r.1),
        None => return,
    };

    let bar_w = state.cell_width * state.scaling * 0.8;

    for series in bars {
        for point in &series.points {
            let sx =
                interval_to_screen_x(state, point.x, canvas_width);
            let left = sx - bar_w / 2.0;
            let color: Color =
                crate::style::theme_bridge::rgba_to_iced_color(
                    point.color,
                );

            let y_val = value_to_y(
                point.value,
                range.0,
                range.1,
                panel_height,
                y_offset,
            );
            let y_base = value_to_y(
                0.0_f32.clamp(range.0, range.1),
                range.0,
                range.1,
                panel_height,
                y_offset,
            );

            let (top, height) = if y_val < y_base {
                (y_val, y_base - y_val)
            } else {
                (y_base, y_val - y_base)
            };

            if height > 0.0 && bar_w > 0.0 {
                frame.fill_rectangle(
                    Point::new(left, top),
                    Size::new(bar_w, height),
                    color,
                );
            }

            // Delta overlay
            if let Some(overlay_val) = point.overlay {
                let ov_abs = overlay_val.abs();
                let y_ov = value_to_y(
                    ov_abs, range.0, range.1, panel_height, y_offset,
                );
                let h = (y_base - y_ov).abs();
                if h > 0.0 {
                    let top = y_ov.min(y_base);
                    frame.fill_rectangle(
                        Point::new(left, top),
                        Size::new(bar_w, h),
                        color.scale_alpha(0.7),
                    );
                }
            }
        }
    }
}

// ── Histogram ────────────────────────────────────────────────────────

fn render_panel_histogram(
    frame: &mut canvas::Frame,
    bars: &[HistogramBar],
    state: &ViewState,
    canvas_width: f32,
    y_offset: f32,
    panel_height: f32,
) {
    if bars.is_empty() {
        return;
    }

    let range = match coord::value_range(bars.iter().map(|b| b.value)) {
        Some(r) => (r.0.min(0.0), r.1.max(0.0)),
        None => return,
    };

    let bar_w = state.cell_width * state.scaling * 0.6;

    for bar in bars {
        let sx = interval_to_screen_x(state, bar.x, canvas_width);
        let left = sx - bar_w / 2.0;
        let color: Color =
            crate::style::theme_bridge::rgba_to_iced_color(bar.color);

        let y_val = value_to_y(
            bar.value,
            range.0,
            range.1,
            panel_height,
            y_offset,
        );
        let y_zero = value_to_y(
            0.0, range.0, range.1, panel_height, y_offset,
        );

        let (top, height) = if bar.value >= 0.0 {
            (y_val, y_zero - y_val)
        } else {
            (y_zero, y_val - y_zero)
        };

        if height > 0.0 && bar_w > 0.0 {
            frame.fill_rectangle(
                Point::new(left, top),
                Size::new(bar_w, height),
                color,
            );
        }
    }
}

// ── Panel Y-axis labels ─────────────────────────────────────────────

/// Canvas program that renders Y-axis labels for panel studies.
///
/// Each panel gets its own auto-scaled tick labels derived from the
/// study output's value range, using `linear::generate_labels()`.
pub struct PanelAxisLabelsY<'a> {
    pub panels: Vec<PanelStudyInfo<'a>>,
    pub cache: &'a Cache,
}

impl canvas::Program<Message> for PanelAxisLabelsY<'_> {
    type State = ();

    fn update(
        &self,
        _state: &mut (),
        _event: &Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        None
    }

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        if self.panels.is_empty() {
            return vec![];
        }

        let palette = theme.extended_palette();
        let text_color = palette.background.base.text;
        let text_size = 10.0;

        let geo = self.cache.draw(renderer, bounds.size(), |frame| {
            let num = self.panels.len();
            let panel_h = bounds.height / num as f32;

            for (i, panel) in self.panels.iter().enumerate() {
                let y_off = i as f32 * panel_h;

                // Separator line between panels
                if i > 0 {
                    frame.fill_rectangle(
                        Point::new(0.0, y_off),
                        Size::new(bounds.width, 1.0),
                        palette.background.strong.color,
                    );
                }

                let Some((lowest, highest)) =
                    panel_value_range(panel.output)
                else {
                    continue;
                };

                // Build a sub-bounds rectangle for this panel
                let sub_bounds = Rectangle {
                    x: bounds.x,
                    y: 0.0,
                    width: bounds.width,
                    height: panel_h,
                };

                let labels = linear::generate_labels(
                    sub_bounds,
                    lowest,
                    highest,
                    text_size,
                    text_color,
                    None,
                );

                // Offset labels into the panel's vertical slice
                let offset_labels: Vec<AxisLabel> = labels
                    .into_iter()
                    .map(|label| match label {
                        AxisLabel::Y {
                            bounds: r,
                            value_label,
                            timer_label,
                        } => AxisLabel::Y {
                            bounds: Rectangle {
                                y: r.y + y_off,
                                ..r
                            },
                            value_label,
                            timer_label,
                        },
                        other => other,
                    })
                    .collect();

                AxisLabel::filter_and_draw(&offset_labels, frame);
            }
        });
        vec![geo]
    }
}
