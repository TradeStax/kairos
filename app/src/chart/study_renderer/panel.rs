//! Study Panel Canvas
//!
//! Renders panel-placement studies (RSI, MACD, Volume, etc.) in a
//! separate canvas below the main chart. Coordinates are in screen
//! space: X is derived from the chart's interval-to-x mapping with
//! the current pan/zoom applied; Y is mapped from the study's value
//! range to the panel's pixel height.

use super::chart_views::{PanelChartView, theme_from_palette};
use super::iced_canvas::IcedCanvas;
use crate::chart::core::PanelStudyInfo;
use crate::chart::scale::{AxisLabel, linear};
use crate::chart::{Message, ViewState};
use crate::components::primitives::AZERET_MONO;
use crate::style;
use iced::widget::canvas::{self, Cache, Event, Geometry, Path, Stroke};
use iced::{Point, Rectangle, Renderer, Size, Theme, Vector, mouse};
use study::output::StudyOutput;

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
                            panning.translation + (pos - panning.start) * (1.0 / state.scaling),
                        );
                        return Some(canvas::Action::publish(msg).and_capture());
                    }

                    // Crosshair: emit CrosshairMoved so main chart
                    // invalidates the crosshair cache
                    if cursor_position.is_some() {
                        return Some(canvas::Action::publish(Message::CrosshairMoved(
                            cursor_position,
                        )));
                    }

                    None
                }
                mouse::Event::WheelScrolled { delta } => {
                    let _pos = cursor_position?;

                    let y = match delta {
                        mouse::ScrollDelta::Lines { y, .. }
                        | mouse::ScrollDelta::Pixels { y, .. } => y,
                    };

                    let cursor_to_center = cursor_position
                        .map(|p| Point::new(p.x - bounds.width / 2.0, p.y - bounds.height / 2.0))?;

                    let is_wheel_scroll =
                        !matches!(self.state.layout.autoscale, Some(data::Autoscale::FitAll));

                    let message = if panel_state.shift_held {
                        Message::YScaling(y / 2.0, cursor_to_center.y, is_wheel_scroll)
                    } else {
                        Message::XScaling(y / 2.0, cursor_to_center.x, is_wheel_scroll)
                    };

                    Some(canvas::Action::publish(message).and_capture())
                }
                mouse::Event::CursorLeft => Some(canvas::Action::publish(Message::CursorLeft)),
                _ => None,
            },
            Event::Keyboard(key_event) => match key_event {
                iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Shift),
                    ..
                } => {
                    panel_state.shift_held = true;
                    None
                }
                iced::keyboard::Event::KeyReleased {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Shift),
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

        // The panel canvas may be wider than the main chart canvas when a
        // side panel is active (the panel spans the full width so it doesn't
        // appear cut off).  Data coordinates must still align with the candles
        // directly above, so we use state.bounds.width (the main chart canvas
        // width, set via BoundsChanged) as the X-axis reference.  Visual
        // elements that should span the full panel (separators, zero-lines)
        // continue to use bounds.width.
        let chart_width = if self.state.bounds.width > 0.0 {
            self.state.bounds.width
        } else {
            bounds.width
        };
        // Crosshair size uses chart_width for correct X positioning.
        let chart_size = Size::new(chart_width, bounds.height);

        let theme_colors = theme_from_palette(palette);

        let geo = self.cache.draw(renderer, bounds.size(), |frame| {
            let num = self.panels.len();
            let panel_h = bounds.height / num as f32;

            for (i, panel) in self.panels.iter().enumerate() {
                let y_off = i as f32 * panel_h;

                // Separator between panels spans the full panel width
                if i > 0 {
                    frame.fill_rectangle(
                        Point::new(0.0, y_off),
                        Size::new(bounds.width, 1.0),
                        palette.background.strong.color,
                    );
                }

                // Render study output via platform-agnostic renderer
                if let Some((min, max)) = study::output::render::panel_value_range(panel.output) {
                    let view = PanelChartView::new(
                        self.state,
                        chart_width,
                        y_off,
                        panel_h,
                        min,
                        max,
                        theme_colors,
                    );
                    let mut canvas = IcedCanvas::new(frame);
                    panel.output.render(
                        &mut canvas,
                        &view,
                        study::StudyPlacement::Panel,
                        Some(&self.state.basis),
                        true,
                    );
                }

                // Study name label
                frame.fill_text(canvas::Text {
                    content: panel.name.to_string(),
                    position: Point::new(4.0, y_off + 2.0),
                    size: iced::Pixels(style::tokens::text::TINY),
                    color: palette.background.base.text.scale_alpha(0.5),
                    font: AZERET_MONO,
                    ..canvas::Text::default()
                });
            }
        });

        // Crosshair layer — vertical line synced with the main chart.
        let crosshair_geo = self.crosshair_cache.draw(renderer, bounds.size(), |frame| {
            let dashed_line = style::dashed_line(theme);

            let interval = self
                .state
                .crosshair
                .interval
                .get()
                .or(self.state.crosshair.remote);

            if let Some(interval) = interval {
                draw_panel_remote_crosshair(
                    self.state,
                    frame,
                    chart_size, // use chart_width for X positioning
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

/// Compute the value range for a panel study output.
///
/// Delegates to the study crate's `panel_value_range`.
pub fn panel_value_range(output: &StudyOutput) -> Option<(f32, f32)> {
    study::output::render::panel_value_range(output)
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
        let text_size = style::tokens::text::TINY;

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

                let Some((lowest, highest)) = panel_value_range(panel.output) else {
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
                    sub_bounds, lowest, highest, text_size, text_color, None,
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
