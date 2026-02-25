//! DrawdownChart canvas program.

use super::{
    ChartHoverState, draw_crosshair_lines, draw_snap_dot,
    draw_tooltip_box, format_date, grid_lines,
    handle_cursor_event, position_tooltip, tooltip_size,
};
use super::super::ManagerMessage;
use iced::mouse;
use iced::widget::canvas::{self, Fill, Frame, Geometry, Path, Stroke};
use iced::{Color, Point, Rectangle};
use std::sync::Arc;

pub struct DrawdownChart<'a> {
    pub result: Arc<backtest::BacktestResult>,
    pub cache: &'a canvas::Cache,
}

impl<'a> canvas::Program<ManagerMessage> for DrawdownChart<'a> {
    type State = ChartHoverState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<ManagerMessage>> {
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
        let curve = &self.result.equity_curve;
        if curve.points.len() < 2 {
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
        _state: &Self::State,
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

impl<'a> DrawdownChart<'a> {
    fn compute_dd_pcts(&self) -> (Vec<(u64, f64)>, f64) {
        let curve = &self.result.equity_curve;
        let mut peak = curve.initial_equity_usd;
        let mut dd_pcts: Vec<(u64, f64)> =
            Vec::with_capacity(curve.points.len());
        let mut max_dd = 0.0_f64;

        for point in &curve.points {
            if point.total_equity_usd > peak {
                peak = point.total_equity_usd;
            }
            let dd = if peak > 0.0 {
                (peak - point.total_equity_usd) / peak * 100.0
            } else {
                0.0
            };
            max_dd = max_dd.max(dd);
            dd_pcts.push((point.timestamp.0, dd));
        }
        (dd_pcts, max_dd.max(1.0))
    }

    fn draw_base(&self, frame: &mut Frame, bounds: Rectangle) {
        let (dd_pcts, max_dd) = self.compute_dd_pcts();
        if dd_pcts.is_empty() {
            return;
        }

        let pad = 20.0_f32;
        let w = bounds.width;
        let h = bounds.height;

        let min_ts = dd_pcts.first().map(|(t, _)| *t).unwrap_or(0);
        let max_ts = dd_pcts.last().map(|(t, _)| *t).unwrap_or(1);
        let ts_range = (max_ts - min_ts).max(1) as f64;

        let ts_to_x = |ts: u64| -> f32 {
            pad + ((ts - min_ts) as f64 / ts_range
                * (w - pad * 2.0) as f64) as f32
        };
        let dd_to_y = |dd: f64| -> f32 {
            pad + (dd / max_dd * (h - pad * 2.0) as f64) as f32
        };

        grid_lines(frame, bounds, pad, 3);

        let fill_color = Color::from_rgba(0.8, 0.2, 0.2, 0.3);
        let fill_path = Path::new(|b| {
            let (first_ts, _) = dd_pcts[0];
            b.move_to(Point::new(ts_to_x(first_ts), dd_to_y(0.0)));
            for &(ts, dd) in &dd_pcts {
                b.line_to(Point::new(ts_to_x(ts), dd_to_y(dd)));
            }
            let (last_ts, _) = *dd_pcts.last().unwrap();
            b.line_to(Point::new(ts_to_x(last_ts), dd_to_y(0.0)));
            b.close();
        });
        frame.fill(
            &fill_path,
            Fill {
                style: fill_color.into(),
                ..Default::default()
            },
        );

        let line_path = Path::new(|b| {
            let (first_ts, first_dd) = dd_pcts[0];
            b.move_to(Point::new(
                ts_to_x(first_ts),
                dd_to_y(first_dd),
            ));
            for &(ts, dd) in &dd_pcts[1..] {
                b.line_to(Point::new(ts_to_x(ts), dd_to_y(dd)));
            }
        });
        frame.stroke(
            &line_path,
            Stroke {
                style: Color::from_rgba(0.8, 0.2, 0.2, 0.6).into(),
                width: 1.0,
                ..Default::default()
            },
        );
    }

    fn draw_overlay(
        &self,
        frame: &mut Frame,
        cursor: Point,
        bounds: Rectangle,
    ) {
        let (dd_pcts, max_dd) = self.compute_dd_pcts();
        if dd_pcts.is_empty() {
            return;
        }

        let pad = 20.0_f32;
        let w = bounds.width;
        let h = bounds.height;

        let min_ts = dd_pcts.first().map(|(t, _)| *t).unwrap_or(0);
        let max_ts = dd_pcts.last().map(|(t, _)| *t).unwrap_or(1);
        let ts_range = (max_ts - min_ts).max(1) as f64;

        let ts_to_x = |ts: u64| -> f32 {
            pad + ((ts - min_ts) as f64 / ts_range
                * (w - pad * 2.0) as f64) as f32
        };
        let dd_to_y = |dd: f64| -> f32 {
            pad + (dd / max_dd * (h - pad * 2.0) as f64) as f32
        };

        let mut best_idx = 0;
        let mut best_dist = f32::INFINITY;
        for (i, &(ts, _)) in dd_pcts.iter().enumerate() {
            let px = ts_to_x(ts);
            let dist = (px - cursor.x).abs();
            if dist < best_dist {
                best_dist = dist;
                best_idx = i;
            }
        }

        let (snap_ts, snap_dd) = dd_pcts[best_idx];
        let snap_x = ts_to_x(snap_ts);
        let snap_y = dd_to_y(snap_dd);

        draw_crosshair_lines(
            frame,
            Point::new(snap_x, cursor.y),
            bounds.size(),
            pad,
        );
        draw_snap_dot(frame, Point::new(snap_x, snap_y), 3.5);

        let lines = vec![
            format_date(snap_ts),
            format!("DD: {:.2}%", snap_dd),
        ];
        let (tw, th) = tooltip_size(&lines);
        let pos = position_tooltip(cursor, tw, th, bounds.size());
        draw_tooltip_box(frame, pos, &lines);
    }
}
