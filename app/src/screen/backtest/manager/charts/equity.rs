//! EquityChart and PropFirmEquityChart canvas programs.

use super::super::ManagerMessage;
use super::{
    ChartHoverState, draw_crosshair_lines, draw_snap_dot,
    draw_tooltip_box, format_currency, format_date,
    grid_lines, handle_cursor_event, position_tooltip,
    tooltip_size,
};
use crate::config::UserTimezone;
use crate::style::tokens;
use iced::mouse;
use iced::widget::canvas::{self, Fill, Frame, Geometry, Path, Stroke, Text};
use iced::{Color, Point, Rectangle, Size};
use std::sync::Arc;

// ── 1. EquityChart ─────────────────────────────────────────────────

pub struct EquityChart<'a> {
    pub result: Arc<backtest::BacktestResult>,
    pub selected_trade_idx: Option<usize>,
    pub cache: &'a canvas::Cache,
    pub timezone: UserTimezone,
}

impl<'a> canvas::Program<ManagerMessage> for EquityChart<'a> {
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
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left),)
        ) && let Some(idx) = cursor
            .position_in(bounds)
            .and_then(|pos| self.find_nearest_trade(pos, bounds))
        {
            return Some(
                canvas::Action::publish(ManagerMessage::SelectTrade(Some(idx))).and_capture(),
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
        let curve = &self.result.equity_curve;
        if curve.points.len() < 2 {
            let frame = Frame::new(renderer, bounds.size());
            return vec![frame.into_geometry()];
        }

        // Base layer (cached)
        let base = self.cache.draw(renderer, bounds.size(), |frame| {
            self.draw_base(frame, bounds);
        });

        // Overlay layer (fresh each frame)
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
            if state.cursor.is_some() {
                mouse::Interaction::Crosshair
            } else {
                mouse::Interaction::default()
            }
        } else {
            mouse::Interaction::default()
        }
    }
}

impl<'a> EquityChart<'a> {
    fn chart_params(&self, bounds: Rectangle) -> Option<EquityChartParams> {
        let curve = &self.result.equity_curve;
        if curve.points.len() < 2 {
            return None;
        }

        let pad = 20.0_f32;
        let w = bounds.width;
        let h = bounds.height;

        let min_ts = curve.points.first().map(|p| p.timestamp.0).unwrap_or(0);
        let max_ts = curve.points.last().map(|p| p.timestamp.0).unwrap_or(1);
        let ts_range = (max_ts - min_ts).max(1) as f64;

        let min_eq = curve
            .points
            .iter()
            .map(|p| p.total_equity_usd)
            .fold(f64::INFINITY, f64::min);
        let max_eq = curve
            .points
            .iter()
            .map(|p| p.total_equity_usd)
            .fold(f64::NEG_INFINITY, f64::max);
        let eq_range = (max_eq - min_eq).max(1.0);
        let pad_max = max_eq + eq_range * 0.05;
        let pad_range = eq_range * 1.1;

        Some(EquityChartParams {
            pad,
            w,
            h,
            min_ts,
            ts_range,
            pad_max,
            pad_range,
        })
    }

    fn ts_to_x(p: &EquityChartParams, ts: u64) -> f32 {
        p.pad + ((ts - p.min_ts) as f64 / p.ts_range * (p.w - p.pad * 2.0) as f64) as f32
    }

    fn eq_to_y(p: &EquityChartParams, eq: f64) -> f32 {
        let norm = (p.pad_max - eq) / p.pad_range;
        p.pad + (norm * (p.h - p.pad * 2.0) as f64) as f32
    }

    fn draw_base(&self, frame: &mut Frame, bounds: Rectangle) {
        let curve = &self.result.equity_curve;
        let Some(p) = self.chart_params(bounds) else {
            return;
        };

        grid_lines(frame, bounds, p.pad, 4);

        // Drawdown shading
        let mut peak = curve.initial_equity_usd;
        for point in &curve.points {
            if point.total_equity_usd > peak {
                peak = point.total_equity_usd;
            }
            if peak > point.total_equity_usd {
                let x = Self::ts_to_x(&p, point.timestamp.0);
                let peak_y = Self::eq_to_y(&p, peak);
                let eq_y = Self::eq_to_y(&p, point.total_equity_usd);
                let dd_h = eq_y - peak_y;
                if dd_h > 0.5 {
                    frame.fill_rectangle(
                        Point::new(x - 1.0, peak_y),
                        Size::new(2.0, dd_h),
                        Fill {
                            style: tokens::backtest::DRAWDOWN_FILL.into(),
                            ..Default::default()
                        },
                    );
                }
            }
        }

        // Selected trade highlight
        if let Some(trade) = self
            .selected_trade_idx
            .and_then(|idx| self.result.trades.get(idx))
        {
            let x1 = Self::ts_to_x(&p, trade.entry_time.0);
            let x2 = Self::ts_to_x(&p, trade.exit_time.0);
            frame.fill_rectangle(
                Point::new(x1, p.pad),
                Size::new((x2 - x1).max(2.0), p.h - p.pad * 2.0),
                Fill {
                    style: tokens::backtest::SELECTED_FILL.into(),
                    ..Default::default()
                },
            );
        }

        // Equity line
        let path = Path::new(|b| {
            let first = &curve.points[0];
            b.move_to(Point::new(
                Self::ts_to_x(&p, first.timestamp.0),
                Self::eq_to_y(&p, first.total_equity_usd),
            ));
            for point in &curve.points[1..] {
                b.line_to(Point::new(
                    Self::ts_to_x(&p, point.timestamp.0),
                    Self::eq_to_y(&p, point.total_equity_usd),
                ));
            }
        });
        frame.stroke(
            &path,
            Stroke {
                style: tokens::backtest::EQUITY_LINE.into(),
                width: 1.5,
                ..Default::default()
            },
        );
    }

    fn draw_overlay(&self, frame: &mut Frame, cursor: Point, bounds: Rectangle) {
        let curve = &self.result.equity_curve;
        let Some(p) = self.chart_params(bounds) else {
            return;
        };

        let snap_idx = self.find_nearest_point_by_x(cursor.x, &p);
        let snap_point = &curve.points[snap_idx];
        let snap_x = Self::ts_to_x(&p, snap_point.timestamp.0);
        let snap_y = Self::eq_to_y(&p, snap_point.total_equity_usd);

        draw_crosshair_lines(frame, Point::new(snap_x, cursor.y), bounds.size(), p.pad);

        draw_snap_dot(frame, Point::new(snap_x, snap_y), 3.5);

        let mut peak = curve.initial_equity_usd;
        for pt in &curve.points[..=snap_idx] {
            if pt.total_equity_usd > peak {
                peak = pt.total_equity_usd;
            }
        }
        let dd_pct = if peak > 0.0 {
            (peak - snap_point.total_equity_usd) / peak * 100.0
        } else {
            0.0
        };

        let lines = vec![
            format_date(snap_point.timestamp.0, self.timezone),
            format!(
                "Equity: {}",
                format_currency(snap_point.total_equity_usd)
            ),
            format!("DD: {:.1}%", dd_pct),
            format!("Trade #{}", snap_idx + 1),
        ];
        let (tw, th) = tooltip_size(&lines);
        let pos = position_tooltip(cursor, tw, th, bounds.size());
        draw_tooltip_box(frame, pos, &lines);
    }

    fn find_nearest_point_by_x(&self, x: f32, p: &EquityChartParams) -> usize {
        let curve = &self.result.equity_curve;
        let mut best_idx = 0;
        let mut best_dist = f32::INFINITY;
        for (i, pt) in curve.points.iter().enumerate() {
            let px = Self::ts_to_x(p, pt.timestamp.0);
            let dist = (px - x).abs();
            if dist < best_dist {
                best_dist = dist;
                best_idx = i;
            }
        }
        best_idx
    }

    fn find_nearest_trade(&self, pos: Point, bounds: Rectangle) -> Option<usize> {
        let curve = &self.result.equity_curve;
        if curve.points.len() < 2 {
            return None;
        }
        let p = self.chart_params(bounds)?;
        let idx = self.find_nearest_point_by_x(pos.x, &p);
        let pt = &curve.points[idx];
        let px = Self::ts_to_x(&p, pt.timestamp.0);
        let py = Self::eq_to_y(&p, pt.total_equity_usd);
        let dist = ((px - pos.x).powi(2) + (py - pos.y).powi(2)).sqrt();
        if dist < 20.0 { Some(idx) } else { None }
    }
}

struct EquityChartParams {
    pad: f32,
    w: f32,
    h: f32,
    min_ts: u64,
    ts_range: f64,
    pad_max: f64,
    pad_range: f64,
}

// ── PropFirmEquityChart ────────────────────────────────────────────

pub struct PropFirmEquityChart<'a> {
    pub equity_curve: &'a [f64],
    pub account_size: f64,
    pub profit_target_pct: f64,
    pub max_drawdown_pct: f64,
    pub breach_trade_idx: Option<usize>,
    pub cache: &'a canvas::Cache,
}

impl<'a> canvas::Program<ManagerMessage> for PropFirmEquityChart<'a> {
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
        if self.equity_curve.len() < 2 {
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

impl<'a> PropFirmEquityChart<'a> {
    fn chart_params(&self, bounds: Rectangle) -> PropFirmParams {
        let pad = 24.0_f32;
        let right_pad = 64.0_f32;
        let w = bounds.width;
        let h = bounds.height;
        let n = self.equity_curve.len();

        let target_eq = self.account_size * (1.0 + self.profit_target_pct / 100.0);
        let dd_limit_eq = self.account_size * (1.0 - self.max_drawdown_pct / 100.0);

        let data_min = self
            .equity_curve
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min)
            .min(dd_limit_eq);
        let data_max = self
            .equity_curve
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
            .max(target_eq);

        let range = (data_max - data_min).max(1.0);
        let pad_min = data_min - range * 0.05;
        let pad_max = data_max + range * 0.05;
        let pad_range = pad_max - pad_min;

        PropFirmParams {
            pad,
            right_pad,
            w,
            h,
            n,
            pad_min,
            pad_max,
            pad_range,
            target_eq,
            dd_limit_eq,
        }
    }

    fn idx_to_x(p: &PropFirmParams, idx: usize) -> f32 {
        let usable = p.w - p.pad - p.right_pad;
        let max_idx = (p.n - 1).max(1) as f64;
        p.pad + (idx as f64 / max_idx * usable as f64) as f32
    }

    fn eq_to_y(p: &PropFirmParams, eq: f64) -> f32 {
        let norm = (p.pad_max - eq) / p.pad_range;
        p.pad + (norm * (p.h - p.pad * 2.0) as f64) as f32
    }

    fn draw_base(&self, frame: &mut Frame, bounds: Rectangle) {
        let p = self.chart_params(bounds);

        grid_lines(frame, bounds, p.pad, 3);

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
            Color::from_rgba(1.0, 1.0, 1.0, 0.2),
            "Start",
        );

        let path = Path::new(|b| {
            let first_y = Self::eq_to_y(&p, self.equity_curve[0]);
            b.move_to(Point::new(Self::idx_to_x(&p, 0), first_y));
            for (i, &eq) in self.equity_curve.iter().enumerate().skip(1) {
                b.line_to(Point::new(Self::idx_to_x(&p, i), Self::eq_to_y(&p, eq)));
            }
        });
        frame.stroke(
            &path,
            Stroke {
                style: tokens::backtest::EQUITY_LINE.into(),
                width: 1.5,
                ..Default::default()
            },
        );

        if let Some(idx) = self.breach_trade_idx {
            let curve_idx = (idx + 1).min(p.n - 1);
            let bx = Self::idx_to_x(&p, curve_idx);
            let by = Self::eq_to_y(&p, self.equity_curve[curve_idx]);
            let circle = Path::circle(Point::new(bx, by), 4.0);
            frame.fill(
                &circle,
                Fill {
                    style: tokens::backtest::PROP_FIRM_LIMIT.into(),
                    ..Default::default()
                },
            );
            let ring = Path::circle(Point::new(bx, by), 6.0);
            frame.stroke(
                &ring,
                Stroke {
                    style: tokens::backtest::PROP_FIRM_LIMIT.into(),
                    width: 1.0,
                    ..Default::default()
                },
            );
        }
    }

    fn draw_ref_line(
        &self,
        frame: &mut Frame,
        p: &PropFirmParams,
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

        let usable = p.w - p.pad - p.right_pad;
        let max_idx = (p.n - 1).max(1);
        let raw_idx = ((cursor.x - p.pad) / usable * max_idx as f32).round() as usize;
        let snap_idx = raw_idx.min(p.n - 1);
        let snap_eq = self.equity_curve[snap_idx];
        let snap_x = Self::idx_to_x(&p, snap_idx);
        let snap_y = Self::eq_to_y(&p, snap_eq);

        draw_crosshair_lines(frame, Point::new(snap_x, cursor.y), bounds.size(), p.pad);
        draw_snap_dot(frame, Point::new(snap_x, snap_y), 3.5);

        let mut peak = self.equity_curve[0];
        for &eq in &self.equity_curve[..=snap_idx] {
            if eq > peak {
                peak = eq;
            }
        }
        let dd_pct = if peak > 0.0 {
            (peak - snap_eq) / self.account_size * 100.0
        } else {
            0.0
        };

        let pnl = snap_eq - self.account_size;
        let trade_label = if snap_idx == 0 {
            "Start".to_string()
        } else {
            format!("Trade #{}", snap_idx)
        };

        let lines = vec![
            trade_label,
            format!("Equity: {}", format_currency(snap_eq)),
            format!(
                "P&L: {}{}",
                if pnl >= 0.0 { "+" } else { "" },
                format_currency(pnl)
            ),
            format!("DD: {:.1}%", dd_pct),
        ];
        let (tw, th) = tooltip_size(&lines);
        let pos = position_tooltip(cursor, tw, th, bounds.size());
        draw_tooltip_box(frame, pos, &lines);
    }
}

struct PropFirmParams {
    pad: f32,
    right_pad: f32,
    w: f32,
    h: f32,
    n: usize,
    #[allow(dead_code)]
    pad_min: f64,
    pad_max: f64,
    pad_range: f64,
    target_eq: f64,
    dd_limit_eq: f64,
}
