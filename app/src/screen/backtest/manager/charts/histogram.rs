//! HistogramChart canvas program.

use super::{
    ChartHoverState, draw_crosshair_lines, draw_tooltip_box,
    format_currency, grid_lines, handle_cursor_event,
    position_tooltip, tooltip_size,
};
use super::super::ManagerMessage;
use crate::style::tokens;
use iced::mouse;
use iced::widget::canvas::{self, Fill, Frame, Geometry, Path, Stroke};
use iced::{Color, Point, Rectangle, Size};

pub struct HistogramChart<'a> {
    /// (bin_center, count)
    pub bins: Vec<(f64, usize)>,
    pub cache: &'a canvas::Cache,
}

impl<'a> canvas::Program<ManagerMessage> for HistogramChart<'a> {
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
        if self.bins.is_empty() {
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
            if self.hovered_bar(state, bounds).is_some() {
                mouse::Interaction::Pointer
            } else {
                mouse::Interaction::Crosshair
            }
        } else {
            mouse::Interaction::default()
        }
    }
}

impl<'a> HistogramChart<'a> {
    fn hist_params(&self, bounds: Rectangle) -> HistParams {
        let pad = 20.0_f32;
        let n = self.bins.len();
        let usable_w = bounds.width - pad * 2.0;
        let usable_h = bounds.height - pad * 2.0;
        let bar_w = (usable_w / n as f32).max(2.0);

        let max_count = self
            .bins
            .iter()
            .map(|(_, c)| *c)
            .max()
            .unwrap_or(1)
            .max(1);

        let min_center = self
            .bins
            .iter()
            .map(|(c, _)| *c)
            .fold(f64::INFINITY, f64::min);
        let max_center = self
            .bins
            .iter()
            .map(|(c, _)| *c)
            .fold(f64::NEG_INFINITY, f64::max);
        let center_range = (max_center - min_center).max(1.0);

        HistParams {
            pad,
            usable_w,
            usable_h,
            bar_w,
            max_count,
            min_center,
            center_range,
        }
    }

    fn center_to_x(p: &HistParams, c: f64) -> f32 {
        p.pad
            + ((c - p.min_center) / p.center_range
                * (p.usable_w - p.bar_w) as f64) as f32
    }

    fn hovered_bar(
        &self,
        state: &ChartHoverState,
        bounds: Rectangle,
    ) -> Option<usize> {
        let cursor = state.cursor?;
        let p = self.hist_params(bounds);
        for (i, &(center, _)) in self.bins.iter().enumerate() {
            let x = Self::center_to_x(&p, center);
            if cursor.x >= x && cursor.x <= x + p.bar_w {
                return Some(i);
            }
        }
        None
    }

    fn draw_base(&self, frame: &mut Frame, bounds: Rectangle) {
        let p = self.hist_params(bounds);
        grid_lines(frame, bounds, p.pad, 3);

        for &(center, count) in &self.bins {
            let bar_h_frac =
                count as f32 / p.max_count as f32 * p.usable_h;
            let x = Self::center_to_x(&p, center);
            let y = p.pad + p.usable_h - bar_h_frac;

            let color = if center >= 0.0 {
                tokens::backtest::POSITIVE_RETURN
            } else {
                tokens::backtest::NEGATIVE_RETURN
            };

            frame.fill_rectangle(
                Point::new(x, y),
                Size::new(p.bar_w - 1.0, bar_h_frac),
                Fill {
                    style: color.into(),
                    ..Default::default()
                },
            );
        }

        let min_center = p.min_center;
        let max_center = min_center + p.center_range;
        if min_center < 0.0 && max_center > 0.0 {
            let zero_x =
                Self::center_to_x(&p, 0.0) + p.bar_w * 0.5;
            let zero_line = Path::line(
                Point::new(zero_x, p.pad),
                Point::new(zero_x, bounds.height - p.pad),
            );
            frame.stroke(
                &zero_line,
                Stroke {
                    style: Color::from_rgba(1.0, 1.0, 1.0, 0.4)
                        .into(),
                    width: 1.5,
                    ..Default::default()
                },
            );
        }
    }

    fn draw_overlay(
        &self,
        frame: &mut Frame,
        cursor: Point,
        bounds: Rectangle,
    ) {
        let p = self.hist_params(bounds);

        let mut hovered_idx = None;
        for (i, &(center, _)) in self.bins.iter().enumerate() {
            let x = Self::center_to_x(&p, center);
            if cursor.x >= x && cursor.x <= x + p.bar_w {
                hovered_idx = Some(i);
                break;
            }
        }

        if let Some(idx) = hovered_idx {
            let (center, count) = self.bins[idx];
            let x = Self::center_to_x(&p, center);
            let bar_h_frac =
                count as f32 / p.max_count as f32 * p.usable_h;
            let y = p.pad + p.usable_h - bar_h_frac;

            frame.fill_rectangle(
                Point::new(x, y),
                Size::new(p.bar_w - 1.0, bar_h_frac),
                Fill {
                    style: tokens::backtest::HOVER_HIGHLIGHT.into(),
                    ..Default::default()
                },
            );

            let lines = vec![
                format!("P&L: {}", format_currency(center)),
                format!("Count: {}", count),
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

struct HistParams {
    pad: f32,
    usable_w: f32,
    usable_h: f32,
    bar_w: f32,
    max_count: usize,
    min_center: f64,
    center_range: f64,
}
