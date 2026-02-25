//! MonteCarloChart canvas program.

use super::{
    ChartHoverState, draw_crosshair_lines, draw_tooltip_box,
    format_currency, grid_lines, handle_cursor_event,
    position_tooltip, tooltip_size,
};
use super::super::ManagerMessage;
use crate::style::tokens;
use iced::mouse;
use iced::widget::canvas::{self, Fill, Frame, Geometry, Path, Stroke, Text};
use iced::{Point, Rectangle};

pub struct MonteCarloChart<'a> {
    pub paths: Vec<Vec<f64>>,
    pub p5: Vec<f64>,
    pub p50: Vec<f64>,
    pub p95: Vec<f64>,
    pub original_equity: Vec<f64>,
    pub cache: &'a canvas::Cache,
}

impl<'a> canvas::Program<ManagerMessage> for MonteCarloChart<'a> {
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
        if self.paths.is_empty() || self.p50.is_empty() {
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

impl<'a> MonteCarloChart<'a> {
    fn mc_params(&self, bounds: Rectangle) -> Option<McParams> {
        let n = self.p50.len();
        if n < 2 {
            return None;
        }

        let pad = 20.0_f32;
        let w = bounds.width;
        let h = bounds.height;

        let mut g_min = f64::INFINITY;
        let mut g_max = f64::NEG_INFINITY;
        for path in &self.paths {
            for &v in path {
                g_min = g_min.min(v);
                g_max = g_max.max(v);
            }
        }
        for &v in &self.original_equity {
            g_min = g_min.min(v);
            g_max = g_max.max(v);
        }
        let val_range = (g_max - g_min).max(1.0);
        let pad_max = g_max + val_range * 0.05;
        let pad_range = val_range * 1.1;

        Some(McParams { pad, w, h, n, pad_max, pad_range })
    }

    fn step_to_x(p: &McParams, step: usize) -> f32 {
        p.pad
            + (step as f64 / (p.n - 1) as f64
                * (p.w - p.pad * 2.0) as f64) as f32
    }

    fn val_to_y(p: &McParams, v: f64) -> f32 {
        let norm = (p.pad_max - v) / p.pad_range;
        p.pad + (norm * (p.h - p.pad * 2.0) as f64) as f32
    }

    fn draw_base(&self, frame: &mut Frame, bounds: Rectangle) {
        let Some(p) = self.mc_params(bounds) else {
            return;
        };
        let n = p.n;

        grid_lines(frame, bounds, p.pad, 4);

        let path_color = tokens::backtest::MONTE_CARLO_PATH;
        for mc_path in &self.paths {
            let len = mc_path.len().min(n);
            if len < 2 {
                continue;
            }
            let line = Path::new(|b| {
                b.move_to(Point::new(
                    Self::step_to_x(&p, 0),
                    Self::val_to_y(&p, mc_path[0]),
                ));
                for (i, &val) in
                    mc_path[1..len].iter().enumerate()
                {
                    b.line_to(Point::new(
                        Self::step_to_x(&p, i + 1),
                        Self::val_to_y(&p, val),
                    ));
                }
            });
            frame.stroke(
                &line,
                Stroke {
                    style: path_color.into(),
                    width: 1.0,
                    ..Default::default()
                },
            );
        }

        let band_color = tokens::backtest::MONTE_CARLO_BAND;
        let band = Path::new(|b| {
            b.move_to(Point::new(
                Self::step_to_x(&p, 0),
                Self::val_to_y(&p, self.p95[0]),
            ));
            for i in 1..n {
                let p95_val = if i < self.p95.len() {
                    self.p95[i]
                } else {
                    *self.p95.last().unwrap_or(&0.0)
                };
                b.line_to(Point::new(
                    Self::step_to_x(&p, i),
                    Self::val_to_y(&p, p95_val),
                ));
            }
            for i in (0..n).rev() {
                let p5_val = if i < self.p5.len() {
                    self.p5[i]
                } else {
                    *self.p5.last().unwrap_or(&0.0)
                };
                b.line_to(Point::new(
                    Self::step_to_x(&p, i),
                    Self::val_to_y(&p, p5_val),
                ));
            }
            b.close();
        });
        frame.fill(
            &band,
            Fill {
                style: band_color.into(),
                ..Default::default()
            },
        );

        let median_line = Path::new(|b| {
            b.move_to(Point::new(
                Self::step_to_x(&p, 0),
                Self::val_to_y(&p, self.p50[0]),
            ));
            for i in 1..n {
                let v = if i < self.p50.len() {
                    self.p50[i]
                } else {
                    *self.p50.last().unwrap_or(&0.0)
                };
                b.line_to(Point::new(
                    Self::step_to_x(&p, i),
                    Self::val_to_y(&p, v),
                ));
            }
        });
        frame.stroke(
            &median_line,
            Stroke {
                style: tokens::backtest::MONTE_CARLO_MEDIAN.into(),
                width: 1.5,
                ..Default::default()
            },
        );

        if self.original_equity.len() >= 2 {
            let orig_len = self.original_equity.len().min(n);
            let orig_line = Path::new(|b| {
                b.move_to(Point::new(
                    Self::step_to_x(&p, 0),
                    Self::val_to_y(&p, self.original_equity[0]),
                ));
                for i in 1..orig_len {
                    b.line_to(Point::new(
                        Self::step_to_x(&p, i),
                        Self::val_to_y(&p, self.original_equity[i]),
                    ));
                }
            });
            frame.stroke(
                &orig_line,
                Stroke {
                    style: tokens::backtest::EQUITY_LINE.into(),
                    width: 2.0,
                    ..Default::default()
                },
            );
        }

        let n_labels = 5usize;
        for i in 0..n_labels {
            let frac = i as f64 / (n_labels - 1) as f64;
            let val = p.pad_max - frac * p.pad_range;
            let y = Self::val_to_y(&p, val);
            let label = Text {
                content: format_currency(val),
                position: Point::new(2.0, y - 4.0),
                color: tokens::backtest::AXIS_TEXT,
                size: iced::Pixels(9.0),
                ..Default::default()
            };
            frame.fill_text(label);
        }

        let last_step = n - 1;
        let end_x = Self::step_to_x(&p, last_step) + 4.0;
        let label_pairs = [
            ("P5", self.p5.get(last_step).copied()),
            ("Median", self.p50.get(last_step).copied()),
            ("P95", self.p95.get(last_step).copied()),
        ];
        for (name, val) in label_pairs {
            if let Some(v) = val {
                let y = Self::val_to_y(&p, v);
                let label = Text {
                    content: name.to_string(),
                    position: Point::new(end_x, y - 4.0),
                    color: tokens::backtest::AXIS_TEXT,
                    size: iced::Pixels(9.0),
                    ..Default::default()
                };
                frame.fill_text(label);
            }
        }
    }

    fn draw_overlay(
        &self,
        frame: &mut Frame,
        cursor: Point,
        bounds: Rectangle,
    ) {
        let Some(p) = self.mc_params(bounds) else {
            return;
        };
        let n = p.n;

        let mut best_step = 0;
        let mut best_dist = f32::INFINITY;
        for i in 0..n {
            let sx = Self::step_to_x(&p, i);
            let dist = (sx - cursor.x).abs();
            if dist < best_dist {
                best_dist = dist;
                best_step = i;
            }
        }

        let snap_x = Self::step_to_x(&p, best_step);

        draw_crosshair_lines(
            frame,
            Point::new(snap_x, cursor.y),
            bounds.size(),
            p.pad,
        );

        let p5_val = self.p5.get(best_step).copied().unwrap_or(0.0);
        let p50_val = self.p50.get(best_step).copied().unwrap_or(0.0);
        let p95_val = self.p95.get(best_step).copied().unwrap_or(0.0);
        let orig_val = self
            .original_equity
            .get(best_step)
            .copied()
            .unwrap_or(0.0);

        let lines = vec![
            format!("Step {}/{}", best_step, n - 1),
            format!("P95: {}", format_currency(p95_val)),
            format!("P50: {}", format_currency(p50_val)),
            format!("P5:  {}", format_currency(p5_val)),
            format!("Orig: {}", format_currency(orig_val)),
        ];
        let (tw, th) = tooltip_size(&lines);
        let pos = position_tooltip(cursor, tw, th, bounds.size());
        draw_tooltip_box(frame, pos, &lines);
    }
}

struct McParams {
    pad: f32,
    w: f32,
    h: f32,
    n: usize,
    pad_max: f64,
    pad_range: f64,
}
