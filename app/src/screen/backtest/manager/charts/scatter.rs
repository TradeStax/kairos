//! ScatterChart canvas program.

use super::{
    ChartHoverState, draw_crosshair_lines, draw_tooltip_box,
    grid_lines, handle_cursor_event, position_tooltip, tooltip_size,
};
use super::super::ManagerMessage;
use crate::style::tokens;
use iced::mouse;
use iced::widget::canvas::{self, Fill, Frame, Geometry, Path, Stroke, Text};
use iced::{Color, Point, Rectangle};

pub struct ScatterChart<'a> {
    /// (mae_ticks, mfe_ticks, is_winner, trade_idx)
    pub points: Vec<(i64, i64, bool, usize)>,
    pub cache: &'a canvas::Cache,
}

impl<'a> canvas::Program<ManagerMessage> for ScatterChart<'a> {
    type State = ChartHoverState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<ManagerMessage>> {
        // Click-to-select trade
        if matches!(
            event,
            canvas::Event::Mouse(
                mouse::Event::ButtonPressed(mouse::Button::Left),
            )
        ) && let Some(idx) = cursor
            .position_in(bounds)
            .and_then(|pos| self.find_nearest_point(pos, bounds))
        {
            let trade_idx = self.points[idx].3;
            return Some(
                canvas::Action::publish(
                    ManagerMessage::SelectTrade(Some(trade_idx)),
                )
                .and_capture(),
            );
        }
        handle_cursor_event(state, event, bounds, cursor)
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        if self.points.is_empty() {
            let frame = Frame::new(renderer, bounds.size());
            return vec![frame.into_geometry()];
        }

        let base = self.cache.draw(renderer, bounds.size(), |frame| {
            self.draw_base(frame, bounds);
        });

        let mut overlay = Frame::new(renderer, bounds.size());
        if let Some(cursor) = state.cursor {
            self.draw_overlay(&mut overlay, cursor, bounds);
        }

        vec![base, overlay.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) {
            let has_nearby = state.cursor.is_some_and(|pos| {
                self.find_nearest_point(pos, bounds).is_some()
            });
            if has_nearby {
                mouse::Interaction::Pointer
            } else {
                mouse::Interaction::Crosshair
            }
        } else {
            mouse::Interaction::default()
        }
    }
}

impl<'a> ScatterChart<'a> {
    fn scatter_params(
        &self,
        bounds: Rectangle,
    ) -> ScatterParams {
        let pad = 20.0_f32;
        let w = bounds.width;
        let h = bounds.height;

        let min_mae = self
            .points
            .iter()
            .map(|(m, _, _, _)| *m)
            .min()
            .unwrap_or(0);
        let max_mae = self
            .points
            .iter()
            .map(|(m, _, _, _)| *m)
            .max()
            .unwrap_or(1);
        let min_mfe = self
            .points
            .iter()
            .map(|(_, m, _, _)| *m)
            .min()
            .unwrap_or(0);
        let max_mfe = self
            .points
            .iter()
            .map(|(_, m, _, _)| *m)
            .max()
            .unwrap_or(1);

        let x_range = (max_mae - min_mae).max(1) as f64;
        let y_range = (max_mfe - min_mfe).max(1) as f64;
        let usable_w = (w - pad * 2.0) as f64;
        let usable_h = (h - pad * 2.0) as f64;

        ScatterParams {
            pad,
            w,
            h,
            min_mae,
            max_mae,
            min_mfe,
            max_mfe,
            x_range,
            y_range,
            usable_w,
            usable_h,
        }
    }

    fn mae_to_x(p: &ScatterParams, mae: i64) -> f32 {
        p.pad
            + ((mae - p.min_mae) as f64 / p.x_range * p.usable_w)
                as f32
    }

    fn mfe_to_y(p: &ScatterParams, mfe: i64) -> f32 {
        p.pad
            + ((p.max_mfe - mfe) as f64 / p.y_range * p.usable_h)
                as f32
    }

    fn find_nearest_point(
        &self,
        pos: Point,
        bounds: Rectangle,
    ) -> Option<usize> {
        let p = self.scatter_params(bounds);
        let radius = 4.0_f32;
        let max_dist = radius + 6.0;
        let mut best_idx = None;
        let mut best_dist = f32::INFINITY;

        for (i, &(mae, mfe, _, _)) in self.points.iter().enumerate()
        {
            let x = Self::mae_to_x(&p, mae);
            let y = Self::mfe_to_y(&p, mfe);
            let dist =
                ((x - pos.x).powi(2) + (y - pos.y).powi(2)).sqrt();
            if dist < max_dist && dist < best_dist {
                best_dist = dist;
                best_idx = Some(i);
            }
        }
        best_idx
    }

    fn draw_base(&self, frame: &mut Frame, bounds: Rectangle) {
        let p = self.scatter_params(bounds);

        grid_lines(frame, bounds, p.pad, 4);

        {
            let diag_color = Color {
                a: 0.15,
                ..tokens::backtest::GRID_LINE
            };
            let shared_min = p.min_mae.max(p.min_mfe);
            let shared_max = p.max_mae.min(p.max_mfe);
            if shared_min <= shared_max {
                let diag = Path::line(
                    Point::new(
                        Self::mae_to_x(&p, shared_min),
                        Self::mfe_to_y(&p, shared_min),
                    ),
                    Point::new(
                        Self::mae_to_x(&p, shared_max),
                        Self::mfe_to_y(&p, shared_max),
                    ),
                );
                frame.stroke(
                    &diag,
                    Stroke {
                        style: diag_color.into(),
                        width: 1.0,
                        ..Default::default()
                    },
                );
            }
        }

        let radius = 4.0_f32;
        for &(mae, mfe, is_winner, _) in &self.points {
            let x = Self::mae_to_x(&p, mae);
            let y = Self::mfe_to_y(&p, mfe);
            let color = if is_winner {
                tokens::backtest::SCATTER_WIN
            } else {
                tokens::backtest::SCATTER_LOSS
            };
            let circle = Path::circle(Point::new(x, y), radius);
            frame.fill(
                &circle,
                Fill {
                    style: color.into(),
                    ..Default::default()
                },
            );
        }

        let mae_label = Text {
            content: "MAE (ticks)".to_string(),
            position: Point::new(
                p.w * 0.5 - 30.0,
                p.h - p.pad * 0.5,
            ),
            color: tokens::backtest::AXIS_TEXT,
            size: iced::Pixels(10.0),
            ..Default::default()
        };
        frame.fill_text(mae_label);

        let mfe_label = Text {
            content: "MFE (ticks)".to_string(),
            position: Point::new(2.0, p.pad + 10.0),
            color: tokens::backtest::AXIS_TEXT,
            size: iced::Pixels(10.0),
            ..Default::default()
        };
        frame.fill_text(mfe_label);
    }

    fn draw_overlay(
        &self,
        frame: &mut Frame,
        cursor: Point,
        bounds: Rectangle,
    ) {
        let p = self.scatter_params(bounds);

        if let Some(idx) =
            self.find_nearest_point(cursor, bounds)
        {
            let (mae, mfe, is_winner, _trade_idx) =
                self.points[idx];
            let x = Self::mae_to_x(&p, mae);
            let y = Self::mfe_to_y(&p, mfe);

            let highlight =
                Path::circle(Point::new(x, y), 7.0);
            frame.stroke(
                &highlight,
                Stroke {
                    style: tokens::backtest::SNAP_DOT.into(),
                    width: 2.0,
                    ..Default::default()
                },
            );

            let outcome =
                if is_winner { "Win" } else { "Loss" };
            let lines = vec![
                format!("MAE: {} ticks", mae),
                format!("MFE: {} ticks", mfe),
                outcome.to_string(),
            ];
            let (tw, th) = tooltip_size(&lines);
            let pos =
                position_tooltip(cursor, tw, th, bounds.size());
            draw_tooltip_box(frame, pos, &lines);
        } else {
            draw_crosshair_lines(
                frame,
                cursor,
                bounds.size(),
                p.pad,
            );
        }
    }
}

struct ScatterParams {
    pad: f32,
    w: f32,
    h: f32,
    min_mae: i64,
    max_mae: i64,
    min_mfe: i64,
    max_mfe: i64,
    x_range: f64,
    y_range: f64,
    usable_w: f64,
    usable_h: f64,
}
