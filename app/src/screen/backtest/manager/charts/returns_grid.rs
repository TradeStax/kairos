//! ReturnsGrid canvas program.

use super::super::ManagerMessage;
use super::{
    ChartHoverState, draw_tooltip_box, handle_cursor_event, position_tooltip, tooltip_size,
};
use crate::style::tokens;
use iced::mouse;
use iced::widget::canvas::{self, Fill, Frame, Geometry, Path, Stroke, Text};
use iced::{Color, Point, Rectangle, Size};

const MONTH_LABELS: [&str; 12] = ["J", "F", "M", "A", "M", "J", "J", "A", "S", "O", "N", "D"];

pub struct ReturnsGrid<'a> {
    /// (year, month 1-12, return_pct)
    pub monthly_returns: Vec<(u16, u8, f64)>,
    pub cache: &'a canvas::Cache,
}

impl<'a> canvas::Program<ManagerMessage> for ReturnsGrid<'a> {
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
        if self.monthly_returns.is_empty() {
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
            if self.hovered_cell(state, bounds).is_some() {
                mouse::Interaction::Pointer
            } else {
                mouse::Interaction::default()
            }
        } else {
            mouse::Interaction::default()
        }
    }
}

impl<'a> ReturnsGrid<'a> {
    fn grid_params(&self, bounds: Rectangle) -> GridParams {
        let min_year = self
            .monthly_returns
            .iter()
            .map(|(y, _, _)| *y)
            .min()
            .unwrap_or(2024);
        let max_year = self
            .monthly_returns
            .iter()
            .map(|(y, _, _)| *y)
            .max()
            .unwrap_or(2024);
        let n_years = (max_year - min_year + 1) as usize;

        let mut max_abs = 0.0_f64;
        let mut lookup = std::collections::HashMap::<(u16, u8), f64>::new();
        for &(year, month, ret) in &self.monthly_returns {
            max_abs = max_abs.max(ret.abs());
            lookup.insert((year, month), ret);
        }
        let max_abs = max_abs.max(0.01);

        let left_margin = 36.0_f32;
        let top_margin = 16.0_f32;
        let usable_w = bounds.width - left_margin - 4.0;
        let usable_h = bounds.height - top_margin - 4.0;
        let cell_w = (usable_w / 12.0).max(8.0);
        let cell_h = if n_years > 0 {
            (usable_h / n_years as f32).max(14.0)
        } else {
            14.0
        };
        let cell_gap = 1.0_f32;

        GridParams {
            min_year,
            n_years,
            max_abs,
            lookup,
            left_margin,
            top_margin,
            cell_w,
            cell_h,
            cell_gap,
        }
    }

    fn hovered_cell(&self, state: &ChartHoverState, bounds: Rectangle) -> Option<(u16, u8, f64)> {
        let cursor = state.cursor?;
        let gp = self.grid_params(bounds);

        for yr_idx in 0..gp.n_years {
            let year = gp.min_year + yr_idx as u16;
            let row_y = gp.top_margin + yr_idx as f32 * gp.cell_h;

            for m in 0..12_u8 {
                let month = m + 1;
                let cell_x = gp.left_margin + m as f32 * gp.cell_w;

                let in_cell = cursor.x >= cell_x
                    && cursor.x <= cell_x + gp.cell_w
                    && cursor.y >= row_y
                    && cursor.y <= row_y + gp.cell_h;
                if let Some(&ret) = in_cell.then(|| gp.lookup.get(&(year, month))).flatten() {
                    return Some((year, month, ret));
                }
            }
        }
        None
    }

    fn draw_base(&self, frame: &mut Frame, bounds: Rectangle) {
        let gp = self.grid_params(bounds);

        for (m, &month_label) in MONTH_LABELS.iter().enumerate() {
            let x = gp.left_margin + m as f32 * gp.cell_w + gp.cell_w * 0.3;
            let label = Text {
                content: month_label.to_string(),
                position: Point::new(x, 2.0),
                color: tokens::backtest::AXIS_TEXT,
                size: iced::Pixels(10.0),
                ..Default::default()
            };
            frame.fill_text(label);
        }

        for yr_idx in 0..gp.n_years {
            let year = gp.min_year + yr_idx as u16;
            let row_y = gp.top_margin + yr_idx as f32 * gp.cell_h;

            let year_label = Text {
                content: year.to_string(),
                position: Point::new(2.0, row_y + 2.0),
                color: tokens::backtest::AXIS_TEXT,
                size: iced::Pixels(10.0),
                ..Default::default()
            };
            frame.fill_text(year_label);

            for m in 0..12_u8 {
                let month = m + 1;
                let cell_x = gp.left_margin + m as f32 * gp.cell_w;

                if let Some(&ret) = gp.lookup.get(&(year, month)) {
                    let intensity = (ret.abs() / gp.max_abs).clamp(0.0, 1.0) as f32;
                    let color = if ret >= 0.0 {
                        Color::from_rgba(0.1, 0.6 * intensity + 0.15, 0.15, 0.3 + intensity * 0.5)
                    } else {
                        Color::from_rgba(0.6 * intensity + 0.15, 0.1, 0.1, 0.3 + intensity * 0.5)
                    };

                    frame.fill_rectangle(
                        Point::new(cell_x + gp.cell_gap * 0.5, row_y + gp.cell_gap * 0.5),
                        Size::new(gp.cell_w - gp.cell_gap, gp.cell_h - gp.cell_gap),
                        Fill {
                            style: color.into(),
                            ..Default::default()
                        },
                    );

                    let ret_str = if ret.abs() >= 10.0 {
                        format!("{:.0}%", ret)
                    } else {
                        format!("{:.1}%", ret)
                    };
                    let text_x = cell_x + gp.cell_gap + 1.0;
                    let text_y = row_y + gp.cell_gap + 1.0;
                    let cell_text = Text {
                        content: ret_str,
                        position: Point::new(text_x, text_y),
                        color: tokens::backtest::AXIS_TEXT,
                        size: iced::Pixels(9.0),
                        ..Default::default()
                    };
                    frame.fill_text(cell_text);
                } else {
                    frame.fill_rectangle(
                        Point::new(cell_x + gp.cell_gap * 0.5, row_y + gp.cell_gap * 0.5),
                        Size::new(gp.cell_w - gp.cell_gap, gp.cell_h - gp.cell_gap),
                        Fill {
                            style: Color::from_rgba(1.0, 1.0, 1.0, 0.02).into(),
                            ..Default::default()
                        },
                    );
                }
            }
        }
    }

    fn draw_overlay(&self, frame: &mut Frame, cursor: Point, bounds: Rectangle) {
        let state = ChartHoverState {
            cursor: Some(cursor),
        };
        let Some((year, month, ret)) = self.hovered_cell(&state, bounds) else {
            return;
        };

        let gp = self.grid_params(bounds);
        let yr_idx = (year - gp.min_year) as usize;
        let row_y = gp.top_margin + yr_idx as f32 * gp.cell_h;
        let cell_x = gp.left_margin + (month - 1) as f32 * gp.cell_w;

        let rect = Path::rectangle(
            Point::new(cell_x + gp.cell_gap * 0.5, row_y + gp.cell_gap * 0.5),
            Size::new(gp.cell_w - gp.cell_gap, gp.cell_h - gp.cell_gap),
        );
        frame.stroke(
            &rect,
            Stroke {
                style: tokens::backtest::SNAP_DOT.into(),
                width: 1.5,
                ..Default::default()
            },
        );

        let month_name = match month {
            1 => "Jan",
            2 => "Feb",
            3 => "Mar",
            4 => "Apr",
            5 => "May",
            6 => "Jun",
            7 => "Jul",
            8 => "Aug",
            9 => "Sep",
            10 => "Oct",
            11 => "Nov",
            12 => "Dec",
            _ => "???",
        };

        let lines = vec![
            format!("{} {}", month_name, year),
            format!("Return: {:.2}%", ret),
        ];
        let (tw, th) = tooltip_size(&lines);
        let pos = position_tooltip(cursor, tw, th, bounds.size());
        draw_tooltip_box(frame, pos, &lines);
    }
}

struct GridParams {
    min_year: u16,
    n_years: usize,
    max_abs: f64,
    lookup: std::collections::HashMap<(u16, u8), f64>,
    left_margin: f32,
    top_margin: f32,
    cell_w: f32,
    cell_h: f32,
    cell_gap: f32,
}
