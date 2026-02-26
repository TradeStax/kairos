//! BarChart canvas program.

use super::super::ManagerMessage;
use super::{
    ChartHoverState, draw_crosshair_lines, draw_tooltip_box, format_currency, grid_lines,
    handle_cursor_event, position_tooltip, tooltip_size,
};
use crate::style::tokens;
use iced::mouse;
use iced::widget::canvas::{self, Fill, Frame, Geometry, Path, Stroke, Text};
use iced::{Color, Point, Rectangle, Size};

pub struct BarChart<'a> {
    /// (label, value)
    pub bars: Vec<(String, f64)>,
    pub cache: &'a canvas::Cache,
}

impl<'a> canvas::Program<ManagerMessage> for BarChart<'a> {
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
        if self.bars.is_empty() {
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

impl<'a> BarChart<'a> {
    fn bar_params(&self, bounds: Rectangle) -> BarParams {
        let pad = 20.0_f32;
        let w = bounds.width;
        let h = bounds.height;
        let n = self.bars.len();

        let max_abs = self
            .bars
            .iter()
            .map(|(_, v)| v.abs())
            .fold(0.0_f64, f64::max)
            .max(1.0);

        let has_negative = self.bars.iter().any(|(_, v)| *v < 0.0);
        let has_positive = self.bars.iter().any(|(_, v)| *v > 0.0);

        let usable_w = w - pad * 2.0;
        let usable_h = h - pad * 2.0;
        let is_compact = n >= 20;
        let bar_ratio = if is_compact { 0.9 } else { 0.8 };
        let bar_w = (usable_w / n as f32 * bar_ratio).clamp(4.0, 40.0);
        let gap = (usable_w / n as f32 * (1.0 - bar_ratio)).max(0.5);
        let label_step = if n >= 24 { 3 } else { 1 };

        let zero_y = if has_positive && has_negative {
            pad + usable_h * 0.5
        } else if has_negative {
            pad
        } else {
            pad + usable_h
        };

        let scale = if has_positive && has_negative {
            (usable_h * 0.5) / max_abs as f32
        } else {
            usable_h / max_abs as f32
        };

        BarParams {
            pad,
            w,
            h,
            n,
            bar_w,
            gap,
            zero_y,
            scale,
            label_step,
            has_positive,
            has_negative,
        }
    }

    fn hovered_bar(&self, state: &ChartHoverState, bounds: Rectangle) -> Option<usize> {
        let cursor = state.cursor?;
        let p = self.bar_params(bounds);
        for i in 0..p.n {
            let center_x = p.pad + (i as f32 + 0.5) * (p.bar_w + p.gap);
            let bar_x = center_x - p.bar_w * 0.5;
            if cursor.x >= bar_x && cursor.x <= bar_x + p.bar_w {
                return Some(i);
            }
        }
        None
    }

    fn draw_base(&self, frame: &mut Frame, bounds: Rectangle) {
        let p = self.bar_params(bounds);

        grid_lines(frame, bounds, p.pad, 3);

        for (i, (label, value)) in self.bars.iter().enumerate() {
            let center_x = p.pad + (i as f32 + 0.5) * (p.bar_w + p.gap);
            let bar_h = (value.abs() as f32 * p.scale).max(1.0);
            let color = if *value >= 0.0 {
                tokens::backtest::POSITIVE_RETURN
            } else {
                tokens::backtest::NEGATIVE_RETURN
            };

            let (bar_x, bar_y) = if *value >= 0.0 {
                (center_x - p.bar_w * 0.5, p.zero_y - bar_h)
            } else {
                (center_x - p.bar_w * 0.5, p.zero_y)
            };

            frame.fill_rectangle(
                Point::new(bar_x, bar_y),
                Size::new(p.bar_w, bar_h),
                Fill {
                    style: color.into(),
                    ..Default::default()
                },
            );

            if i % p.label_step == 0 {
                let label_text = Text {
                    content: label.clone(),
                    position: Point::new(center_x - p.bar_w * 0.5, p.h - p.pad * 0.5),
                    color: tokens::backtest::AXIS_TEXT,
                    size: iced::Pixels(9.0),
                    ..Default::default()
                };
                frame.fill_text(label_text);
            }
        }

        if p.has_positive && p.has_negative {
            let zero_line = Path::line(
                Point::new(p.pad, p.zero_y),
                Point::new(p.w - p.pad, p.zero_y),
            );
            frame.stroke(
                &zero_line,
                Stroke {
                    style: Color::from_rgba(1.0, 1.0, 1.0, 0.25).into(),
                    width: 1.0,
                    ..Default::default()
                },
            );
        }
    }

    fn draw_overlay(&self, frame: &mut Frame, cursor: Point, bounds: Rectangle) {
        let p = self.bar_params(bounds);

        let hovered = self.hovered_bar(
            &ChartHoverState {
                cursor: Some(cursor),
            },
            bounds,
        );

        if let Some(idx) = hovered {
            let (label, value) = &self.bars[idx];
            let center_x = p.pad + (idx as f32 + 0.5) * (p.bar_w + p.gap);
            let bar_h = (value.abs() as f32 * p.scale).max(1.0);
            let (bar_x, bar_y) = if *value >= 0.0 {
                (center_x - p.bar_w * 0.5, p.zero_y - bar_h)
            } else {
                (center_x - p.bar_w * 0.5, p.zero_y)
            };

            frame.fill_rectangle(
                Point::new(bar_x, bar_y),
                Size::new(p.bar_w, bar_h),
                Fill {
                    style: tokens::backtest::HOVER_HIGHLIGHT.into(),
                    ..Default::default()
                },
            );

            let lines = vec![
                format!("Hour: {}", label),
                format!("P&L: {}", format_currency(*value)),
            ];
            let (tw, th) = tooltip_size(&lines);
            let pos = position_tooltip(cursor, tw, th, bounds.size());
            draw_tooltip_box(frame, pos, &lines);
        } else {
            draw_crosshair_lines(frame, cursor, bounds.size(), p.pad);
        }
    }
}

struct BarParams {
    pad: f32,
    w: f32,
    h: f32,
    n: usize,
    bar_w: f32,
    gap: f32,
    zero_y: f32,
    scale: f32,
    label_step: usize,
    has_positive: bool,
    has_negative: bool,
}
