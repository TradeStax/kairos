//! Monte Carlo paths chart for prop firm simulation.

use super::super::ManagerMessage;
use super::{
    ChartHoverState, draw_crosshair_lines, draw_tooltip_box, format_currency, grid_lines,
    handle_cursor_event, position_tooltip, tooltip_size,
};
use crate::screen::backtest::manager::computed::McPropFirmPath;
use crate::style::tokens;
use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke, Text};
use iced::{Color, Point, Rectangle};

pub struct PropFirmMonteCarloChart<'a> {
    pub paths: &'a [McPropFirmPath],
    pub account_size: f64,
    pub profit_target: f64,
    pub dd_limit: f64,
    pub cache: &'a canvas::Cache,
}

impl<'a> canvas::Program<ManagerMessage> for PropFirmMonteCarloChart<'a> {
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
        if self.paths.is_empty() {
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

struct McChartParams {
    pad: f32,
    right_pad: f32,
    w: f32,
    h: f32,
    max_steps: usize,
    pad_max: f64,
    pad_range: f64,
    target_eq: f64,
    dd_limit_eq: f64,
}

impl<'a> PropFirmMonteCarloChart<'a> {
    fn chart_params(&self, bounds: Rectangle) -> McChartParams {
        let pad = 24.0_f32;
        let right_pad = 64.0_f32;

        let max_steps = self
            .paths
            .iter()
            .map(|p| p.equity_curve.len())
            .max()
            .unwrap_or(1);

        let target_eq = self.account_size + self.profit_target;
        let dd_limit_eq = self.account_size - self.dd_limit;

        let mut data_min = dd_limit_eq;
        let mut data_max = target_eq;
        for path in self.paths {
            for &eq in &path.equity_curve {
                if eq < data_min {
                    data_min = eq;
                }
                if eq > data_max {
                    data_max = eq;
                }
            }
        }

        let range = (data_max - data_min).max(1.0);
        let pad_max = data_max + range * 0.05;
        let pad_min = data_min - range * 0.05;

        McChartParams {
            pad,
            right_pad,
            w: bounds.width,
            h: bounds.height,
            max_steps,
            pad_max,
            pad_range: pad_max - pad_min,
            target_eq,
            dd_limit_eq,
        }
    }

    fn idx_to_x(p: &McChartParams, idx: usize) -> f32 {
        let usable = p.w - p.pad - p.right_pad;
        let max_idx = (p.max_steps - 1).max(1) as f64;
        p.pad + (idx as f64 / max_idx * usable as f64) as f32
    }

    fn eq_to_y(p: &McChartParams, eq: f64) -> f32 {
        let norm = (p.pad_max - eq) / p.pad_range;
        p.pad + (norm * (p.h - p.pad * 2.0) as f64) as f32
    }

    fn draw_base(&self, frame: &mut Frame, bounds: Rectangle) {
        let p = self.chart_params(bounds);

        grid_lines(frame, bounds, p.pad, 3);

        // Reference lines
        self.draw_ref_line(
            frame,
            &p,
            p.target_eq,
            tokens::backtest::PROP_FIRM_TARGET,
            "Target",
        );
        self.draw_ref_line(
            frame,
            &p,
            p.dd_limit_eq,
            tokens::backtest::PROP_FIRM_LIMIT,
            "DD Limit",
        );
        self.draw_ref_line(
            frame,
            &p,
            self.account_size,
            Color::from_rgba(1.0, 1.0, 1.0, 0.15),
            "Start",
        );

        // Draw paths
        for path in self.paths {
            if path.equity_curve.len() < 2 {
                continue;
            }
            let color = if path.passed {
                tokens::backtest::PROP_FIRM_MC_PASS_PATH
            } else {
                tokens::backtest::PROP_FIRM_MC_FAIL_PATH
            };

            let line = Path::new(|b| {
                let first_y = Self::eq_to_y(&p, path.equity_curve[0]);
                b.move_to(Point::new(Self::idx_to_x(&p, 0), first_y));
                for (i, &eq) in path.equity_curve.iter().enumerate().skip(1) {
                    b.line_to(Point::new(Self::idx_to_x(&p, i), Self::eq_to_y(&p, eq)));
                }
            });
            frame.stroke(
                &line,
                Stroke {
                    style: color.into(),
                    width: 1.0,
                    ..Default::default()
                },
            );
        }
    }

    fn draw_ref_line(
        &self,
        frame: &mut Frame,
        p: &McChartParams,
        equity_level: f64,
        color: Color,
        label: &str,
    ) {
        let y = Self::eq_to_y(p, equity_level);
        if y < p.pad || y > p.h - p.pad {
            return;
        }

        let dash = 5.0_f32;
        let gap = 3.0_f32;
        let mut x = p.pad;
        while x < p.w - p.right_pad {
            let end = (x + dash).min(p.w - p.right_pad);
            let seg = Path::line(Point::new(x, y), Point::new(end, y));
            frame.stroke(
                &seg,
                Stroke {
                    style: color.into(),
                    width: 1.0,
                    ..Default::default()
                },
            );
            x += dash + gap;
        }

        let label_text = Text {
            content: label.to_string(),
            position: Point::new(p.w - p.right_pad + 4.0, y - 5.0),
            color,
            size: iced::Pixels(9.0),
            ..Default::default()
        };
        frame.fill_text(label_text);
    }

    fn draw_overlay(&self, frame: &mut Frame, cursor: Point, bounds: Rectangle) {
        let p = self.chart_params(bounds);

        draw_crosshair_lines(frame, cursor, bounds.size(), p.pad);

        // Find step index from x position
        let usable = p.w - p.pad - p.right_pad;
        let max_idx = (p.max_steps - 1).max(1);
        let raw = ((cursor.x - p.pad) / usable * max_idx as f32).round() as usize;
        let step = raw.min(max_idx);

        // Gather stats at this step
        let mut pass_count = 0;
        let mut fail_count = 0;
        let mut equities = Vec::new();
        for path in self.paths {
            if step < path.equity_curve.len() {
                equities.push(path.equity_curve[step]);
                if path.passed && path.completion_idx.is_some_and(|ci| step >= ci) {
                    pass_count += 1;
                } else if !path.passed && path.completion_idx.is_some_and(|ci| step >= ci) {
                    fail_count += 1;
                }
            }
        }

        let median = if !equities.is_empty() {
            equities.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            equities[equities.len() / 2]
        } else {
            self.account_size
        };

        let lines = vec![
            format!("Step {}", step),
            format!("Median: {}", format_currency(median)),
            format!("Passed: {} / Failed: {}", pass_count, fail_count),
        ];
        let (tw, th) = tooltip_size(&lines);
        let pos = position_tooltip(cursor, tw, th, bounds.size());
        draw_tooltip_box(frame, pos, &lines);
    }
}
