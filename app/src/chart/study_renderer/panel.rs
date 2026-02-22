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
use crate::style;
use iced::theme::palette::Extended;
use iced::widget::canvas::{self, Cache, Event, Geometry, Path, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, Vector, mouse};
use study::output::{BarSeries, HistogramBar, LineSeries, StudyOutput};

/// Canvas program that renders panel studies in screen coordinates.
pub struct StudyPanelCanvas<'a> {
    pub panels: Vec<PanelStudyInfo<'a>>,
    pub state: &'a ViewState,
    pub cache: &'a Cache,
    pub crosshair_cache: &'a Cache,
}

/// Interaction state for the study panel canvas.
#[derive(Default, Debug, Clone)]
pub struct PanelInteraction {
    /// Active panning state
    panning: Option<PanelPanning>,
    /// Whether shift is held
    shift_held: bool,
}

#[derive(Debug, Clone, Copy)]
struct PanelPanning {
    translation: Vector,
    start: Point,
}

impl<'a> canvas::Program<Message> for StudyPanelCanvas<'a> {
    type State = PanelInteraction;

    fn update(
        &self,
        panel_state: &mut PanelInteraction,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let cursor_position = cursor.position_in(bounds);

        // Handle button release — end panning
        if let Event::Mouse(mouse::Event::ButtonReleased(_)) = event
            && panel_state.panning.is_some()
        {
            panel_state.panning = None;
        }

        match event {
            Event::Mouse(mouse_event) => match mouse_event {
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    let pos = cursor_position?;
                    panel_state.panning = Some(PanelPanning {
                        translation: self.state.translation,
                        start: pos,
                    });
                    Some(canvas::Action::capture())
                }
                mouse::Event::CursorMoved { .. } => {
                    if let Some(panning) = panel_state.panning {
                        let pos = cursor_position?;
                        let state = self.state;
                        let msg = Message::Translated(
                            panning.translation
                                + (pos - panning.start)
                                    * (1.0 / state.scaling),
                        );
                        return Some(
                            canvas::Action::publish(msg).and_capture(),
                        );
                    }

                    // Crosshair: emit CrosshairMoved so main chart
                    // invalidates the crosshair cache
                    if cursor_position.is_some() {
                        return Some(canvas::Action::publish(
                            Message::CrosshairMoved(cursor_position),
                        ));
                    }

                    None
                }
                mouse::Event::WheelScrolled { delta } => {
                    let _pos = cursor_position?;

                    let y = match delta {
                        mouse::ScrollDelta::Lines { y, .. }
                        | mouse::ScrollDelta::Pixels { y, .. } => y,
                    };

                    let cursor_to_center = cursor_position.map(|p| {
                        Point::new(
                            p.x - bounds.width / 2.0,
                            p.y - bounds.height / 2.0,
                        )
                    })?;

                    let is_wheel_scroll = !matches!(
                        self.state.layout.autoscale,
                        Some(data::Autoscale::FitAll)
                    );

                    let message = if panel_state.shift_held {
                        Message::YScaling(
                            y / 2.0,
                            cursor_to_center.y,
                            is_wheel_scroll,
                        )
                    } else {
                        Message::XScaling(
                            y / 2.0,
                            cursor_to_center.x,
                            is_wheel_scroll,
                        )
                    };

                    Some(canvas::Action::publish(message).and_capture())
                }
                mouse::Event::CursorLeft => Some(
                    canvas::Action::publish(Message::CursorLeft),
                ),
                _ => None,
            },
            Event::Keyboard(key_event) => match key_event {
                iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(
                        iced::keyboard::key::Named::Shift,
                    ),
                    ..
                } => {
                    panel_state.shift_held = true;
                    None
                }
                iced::keyboard::Event::KeyReleased {
                    key: iced::keyboard::Key::Named(
                        iced::keyboard::key::Named::Shift,
                    ),
                    ..
                } => {
                    panel_state.shift_held = false;
                    None
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &PanelInteraction,
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

        // Crosshair layer — draws a vertical dashed line synced with
        // the main chart's crosshair position.  Uses crosshair_interval
        // (set by both main chart and panel cursor) so the line appears
        // regardless of which canvas the cursor is in.
        let crosshair_geo =
            self.crosshair_cache.draw(renderer, bounds.size(), |frame| {
                let dashed_line = style::dashed_line(theme);

                let interval = self
                    .state
                    .crosshair_interval
                    .get()
                    .or(self.state.remote_crosshair);

                if let Some(interval) = interval {
                    draw_panel_remote_crosshair(
                        self.state,
                        frame,
                        bounds.size(),
                        interval,
                        dashed_line,
                    );
                }
            });

        vec![geo, crosshair_geo]
    }
}

/// Draw a remote crosshair vertical line in the study panel.
fn draw_panel_remote_crosshair(
    state: &ViewState,
    frame: &mut canvas::Frame,
    bounds: Size,
    interval: u64,
    dashed_line: Stroke<'_>,
) {
    let region = state.visible_region(bounds);

    match state.basis {
        data::ChartBasis::Time(_) => {
            let chart_x = state.interval_to_x(interval);
            let x_min = region.x;
            let range = region.width;
            if range.abs() < f32::EPSILON {
                return;
            }
            let screen_x = ((chart_x - x_min) / range) * bounds.width;
            if screen_x < 0.0 || screen_x > bounds.width {
                return;
            }
            frame.stroke(
                &Path::line(
                    Point::new(screen_x, 0.0),
                    Point::new(screen_x, bounds.height),
                ),
                dashed_line,
            );
        }
        data::ChartBasis::Tick(aggregation) => {
            let agg = u64::from(aggregation);
            if agg == 0 {
                return;
            }
            let cell_index = -(interval as f32 / agg as f32);
            let chart_x = cell_index * state.cell_width;
            let x_min = region.x;
            let range = region.width;
            if range.abs() < f32::EPSILON {
                return;
            }
            let screen_x = ((chart_x - x_min) / range) * bounds.width;
            if screen_x < 0.0 || screen_x > bounds.width {
                return;
            }
            frame.stroke(
                &Path::line(
                    Point::new(screen_x, 0.0),
                    Point::new(screen_x, bounds.height),
                ),
                dashed_line,
            );
        }
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
        StudyOutput::Composite(outputs) => {
            let mut min = f32::MAX;
            let mut max = f32::MIN;
            let mut found = false;
            for sub in outputs {
                if let Some((lo, hi)) = panel_value_range(sub) {
                    min = min.min(lo);
                    max = max.max(hi);
                    found = true;
                }
            }
            if found { Some((min, max)) } else { None }
        }
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
        StudyOutput::Composite(outputs) => {
            let range = match panel_value_range(output) {
                Some(r) => r,
                None => return,
            };
            for sub in outputs {
                match sub {
                    StudyOutput::Lines(lines) => {
                        for series in lines {
                            render_line_with_range(
                                frame, series, state, w, y_off,
                                h, range,
                            );
                        }
                    }
                    StudyOutput::Histogram(bars) => {
                        draw_histogram_bars(
                            frame, bars, state, w, y_off, h,
                            range,
                        );
                    }
                    _ => {}
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

    draw_histogram_bars(
        frame, bars, state, canvas_width, y_offset, panel_height,
        range,
    );
}

fn draw_histogram_bars(
    frame: &mut canvas::Frame,
    bars: &[HistogramBar],
    state: &ViewState,
    canvas_width: f32,
    y_offset: f32,
    panel_height: f32,
    range: (f32, f32),
) {
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
