//! Full session candlestick chart for the trade detail view.
//!
//! A `canvas::Program` that renders all session candles with
//! entry/exit markers, SL/TP lines, MAE/MFE bands, strategy
//! overlays, price labels, and an interactive crosshair with tooltip.

use super::super::ManagerMessage;
use super::super::charts::{
    ChartHoverState, draw_crosshair_lines, draw_tooltip_box, grid_lines, handle_cursor_event,
    position_tooltip, tooltip_size,
};
use super::strategy_context;
use crate::style::tokens;
use iced::mouse;
use iced::widget::canvas::{self, Fill, Frame, Geometry, Path, Stroke, Text};
use iced::{Color, Point, Rectangle, Size};

/// Chart padding: left (Y-axis labels), right (price labels), top, bottom.
const PAD_LEFT: f32 = 55.0;
const PAD_RIGHT: f32 = 80.0;
const PAD_TOP: f32 = 20.0;
const PAD_BOTTOM: f32 = 25.0;

/// Candle colors.
const CANDLE_UP: Color = Color::from_rgba(0.3, 0.8, 0.3, 0.85);
const CANDLE_DOWN: Color = Color::from_rgba(0.8, 0.3, 0.3, 0.85);

pub struct MiniTradeChart<'a> {
    pub trade: &'a backtest::TradeRecord,
    pub snapshot: Option<&'a backtest::TradeSnapshot>,
    pub tick_size: f64,
    pub cache: &'a canvas::Cache,
    pub strategy_id: &'a str,
}

impl<'a> canvas::Program<ManagerMessage> for MiniTradeChart<'a> {
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
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let base = self.cache.draw(renderer, bounds.size(), |frame| {
            grid_lines(frame, bounds, PAD_LEFT.min(PAD_TOP), 4);

            if let Some(snapshot) = self.snapshot
                && !snapshot.candles.is_empty()
            {
                self.draw_with_candles(frame, bounds, snapshot);
                return;
            }
            self.draw_fallback(frame, bounds);
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
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) && state.cursor.is_some() {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

// ── Layout ──────────────────────────────────────────────────────────

struct ChartLayout {
    usable_h: f32,
    price_min: f64,
    price_range: f64,
    n_candles: usize,
    cell_w: f32,
}

impl ChartLayout {
    fn price_to_y(&self, price: f64) -> f32 {
        if self.price_range == 0.0 {
            return PAD_TOP + self.usable_h / 2.0;
        }
        PAD_TOP
            + ((self.price_min + self.price_range - price) / self.price_range) as f32
                * self.usable_h
    }

    fn y_to_price(&self, y: f32) -> f64 {
        if self.price_range == 0.0 {
            return self.price_min;
        }
        let frac = ((y - PAD_TOP) / self.usable_h).clamp(0.0, 1.0);
        self.price_min + self.price_range - frac as f64 * self.price_range
    }

    fn candle_x(&self, idx: usize) -> f32 {
        PAD_LEFT + idx as f32 * self.cell_w + self.cell_w / 2.0
    }

    fn x_to_candle_idx(&self, x: f32) -> usize {
        ((x - PAD_LEFT) / self.cell_w)
            .floor()
            .clamp(0.0, (self.n_candles.saturating_sub(1)) as f32) as usize
    }
}

// ── Drawing ─────────────────────────────────────────────────────────

impl<'a> MiniTradeChart<'a> {
    fn build_layout(&self, bounds: Rectangle, candles: &[data::Candle]) -> ChartLayout {
        let n = candles.len();
        let entry = self.trade.entry_price.to_f64();
        let exit = self.trade.exit_price.to_f64();
        let sl = self.trade.initial_stop_loss.to_f64();
        let tp = self.trade.initial_take_profit.map(|p| p.to_f64());

        let mut min_p = candles
            .iter()
            .map(|c| c.low.to_f64())
            .fold(f64::MAX, f64::min);
        let mut max_p = candles
            .iter()
            .map(|c| c.high.to_f64())
            .fold(f64::MIN, f64::max);
        min_p = min_p.min(sl).min(entry).min(exit);
        max_p = max_p.max(entry).max(exit);
        if let Some(t) = tp {
            min_p = min_p.min(t);
            max_p = max_p.max(t);
        }
        // Add 2% padding
        let range = max_p - min_p;
        let pad_price = range * 0.02;
        min_p -= pad_price;
        max_p += pad_price;

        let usable_w = bounds.width - PAD_LEFT - PAD_RIGHT;
        let usable_h = bounds.height - PAD_TOP - PAD_BOTTOM;

        ChartLayout {
            usable_h,
            price_min: min_p,
            price_range: max_p - min_p,
            n_candles: n,
            cell_w: if n > 0 { usable_w / n as f32 } else { usable_w },
        }
    }

    fn draw_with_candles(
        &self,
        frame: &mut Frame,
        bounds: Rectangle,
        snapshot: &backtest::TradeSnapshot,
    ) {
        let candles = &snapshot.candles;
        let layout = self.build_layout(bounds, candles);

        let x_start = PAD_LEFT;
        let x_end = bounds.width - PAD_RIGHT;

        // Strategy overlays (behind candles)
        if !snapshot.context.is_empty() {
            strategy_context::draw_strategy_overlays(
                frame,
                self.strategy_id,
                &snapshot.context,
                &|p| layout.price_to_y(p),
                x_start,
                x_end,
            );
        }

        // MAE/MFE bands
        self.draw_excursion_bands(frame, &layout, snapshot);

        // SL / TP horizontal dashed lines with right-side price pills
        self.draw_sl_tp_lines(frame, &layout, bounds);

        // Entry / exit horizontal lines (subtle)
        self.draw_entry_exit_lines(frame, &layout, bounds);

        // Candlesticks
        for (i, candle) in candles.iter().enumerate() {
            let cx = layout.candle_x(i);
            let o = candle.open.to_f64();
            let c = candle.close.to_f64();
            let h = candle.high.to_f64();
            let l = candle.low.to_f64();
            let is_up = c >= o;
            let color = if is_up { CANDLE_UP } else { CANDLE_DOWN };

            let body_top = layout.price_to_y(if is_up { c } else { o });
            let body_bot = layout.price_to_y(if is_up { o } else { c });
            let wick_top = layout.price_to_y(h);
            let wick_bot = layout.price_to_y(l);

            let body_h = (body_bot - body_top).max(1.0);
            let bar_w = (layout.cell_w * 0.6).max(3.0);

            // Wick
            let wick = Path::line(Point::new(cx, wick_top), Point::new(cx, wick_bot));
            frame.stroke(
                &wick,
                Stroke {
                    style: color.into(),
                    width: 1.0,
                    ..Default::default()
                },
            );

            // Body
            let body = Path::rectangle(
                Point::new(cx - bar_w / 2.0, body_top),
                Size::new(bar_w, body_h),
            );
            frame.fill(
                &body,
                Fill {
                    style: color.into(),
                    ..Default::default()
                },
            );
        }

        // Entry marker
        if let Some(idx) = snapshot.entry_candle_idx {
            let x = layout.candle_x(idx);
            let entry = self.trade.entry_price.to_f64();
            let y = layout.price_to_y(entry);
            let is_long = self.trade.side.is_buy();
            self.draw_entry_marker(frame, x, y, is_long, tokens::backtest::ENTRY_MARKER);
        }

        // Exit marker
        if let Some(idx) = snapshot.exit_candle_idx {
            let x = layout.candle_x(idx);
            let exit = self.trade.exit_price.to_f64();
            let y = layout.price_to_y(exit);
            let is_win = self.trade.pnl_net_usd >= 0.0;
            let color = if is_win {
                tokens::backtest::EXIT_MARKER_WIN
            } else {
                tokens::backtest::EXIT_MARKER_LOSS
            };
            self.draw_exit_marker(frame, x, y, color);
        }

        // Y-axis price labels
        self.draw_y_labels(frame, &layout, bounds);
    }

    fn draw_excursion_bands(
        &self,
        frame: &mut Frame,
        layout: &ChartLayout,
        snapshot: &backtest::TradeSnapshot,
    ) {
        let entry = self.trade.entry_price.to_f64();
        let is_long = self.trade.side.is_buy();

        let entry_idx = snapshot.entry_candle_idx.unwrap_or(0);
        let exit_idx = snapshot
            .exit_candle_idx
            .unwrap_or(layout.n_candles.saturating_sub(1));
        let x_start = layout.candle_x(entry_idx) - layout.cell_w / 2.0;
        let x_end = layout.candle_x(exit_idx) + layout.cell_w / 2.0;
        let y_entry = layout.price_to_y(entry);

        // MAE band
        let mae_price = if is_long {
            entry - self.trade.mae_ticks as f64 * self.tick_size
        } else {
            entry + self.trade.mae_ticks as f64 * self.tick_size
        };
        let y_mae = layout.price_to_y(mae_price);
        let (y_top, y_bot) = ordered(y_entry, y_mae);
        let band = Path::rectangle(
            Point::new(x_start, y_top),
            Size::new(x_end - x_start, (y_bot - y_top).max(1.0)),
        );
        frame.fill(
            &band,
            Fill {
                style: tokens::backtest::MAE_BAND.into(),
                ..Default::default()
            },
        );

        // MFE band
        let mfe_price = if is_long {
            entry + self.trade.mfe_ticks as f64 * self.tick_size
        } else {
            entry - self.trade.mfe_ticks as f64 * self.tick_size
        };
        let y_mfe = layout.price_to_y(mfe_price);
        let (y_top, y_bot) = ordered(y_entry, y_mfe);
        let band = Path::rectangle(
            Point::new(x_start, y_top),
            Size::new(x_end - x_start, (y_bot - y_top).max(1.0)),
        );
        frame.fill(
            &band,
            Fill {
                style: tokens::backtest::MFE_BAND.into(),
                ..Default::default()
            },
        );
    }

    fn draw_sl_tp_lines(&self, frame: &mut Frame, layout: &ChartLayout, bounds: Rectangle) {
        let x_start = PAD_LEFT;
        let x_end = bounds.width - PAD_RIGHT;

        // Stop Loss
        let sl = self.trade.initial_stop_loss.to_f64();
        let y_sl = layout.price_to_y(sl);
        draw_dashed_h_line(
            frame,
            x_start,
            x_end,
            y_sl,
            tokens::backtest::STOP_LOSS_LINE,
        );
        draw_price_pill(
            frame,
            x_end + 4.0,
            y_sl,
            &format!("SL {:.2}", sl),
            tokens::backtest::STOP_LOSS_LINE,
        );

        // Take Profit
        if let Some(tp) = self.trade.initial_take_profit {
            let tp_f = tp.to_f64();
            let y_tp = layout.price_to_y(tp_f);
            draw_dashed_h_line(
                frame,
                x_start,
                x_end,
                y_tp,
                tokens::backtest::TAKE_PROFIT_LINE,
            );
            draw_price_pill(
                frame,
                x_end + 4.0,
                y_tp,
                &format!("TP {:.2}", tp_f),
                tokens::backtest::TAKE_PROFIT_LINE,
            );
        }
    }

    fn draw_entry_exit_lines(&self, frame: &mut Frame, layout: &ChartLayout, bounds: Rectangle) {
        let x_start = PAD_LEFT;
        let x_end = bounds.width - PAD_RIGHT;
        let entry = self.trade.entry_price.to_f64();
        let exit = self.trade.exit_price.to_f64();
        let is_win = self.trade.pnl_net_usd >= 0.0;

        // Entry line
        let y_entry = layout.price_to_y(entry);
        let entry_color = Color {
            a: 0.4,
            ..tokens::backtest::ENTRY_MARKER
        };
        let entry_line = Path::line(Point::new(x_start, y_entry), Point::new(x_end, y_entry));
        frame.stroke(
            &entry_line,
            Stroke {
                style: entry_color.into(),
                width: 0.5,
                ..Default::default()
            },
        );
        draw_price_pill(
            frame,
            x_end + 4.0,
            y_entry,
            &format!("{:.2}", entry),
            tokens::backtest::ENTRY_MARKER,
        );

        // Exit line
        let y_exit = layout.price_to_y(exit);
        let exit_base = if is_win {
            tokens::backtest::EXIT_MARKER_WIN
        } else {
            tokens::backtest::EXIT_MARKER_LOSS
        };
        let exit_color = Color {
            a: 0.4,
            ..exit_base
        };
        let exit_line = Path::line(Point::new(x_start, y_exit), Point::new(x_end, y_exit));
        frame.stroke(
            &exit_line,
            Stroke {
                style: exit_color.into(),
                width: 0.5,
                ..Default::default()
            },
        );
        draw_price_pill(
            frame,
            x_end + 4.0,
            y_exit,
            &format!("{:.2}", exit),
            exit_base,
        );
    }

    fn draw_entry_marker(&self, frame: &mut Frame, x: f32, y: f32, is_long: bool, color: Color) {
        // Triangle arrow: ▲ for long, ▼ for short
        let size = 5.0_f32;
        let triangle = if is_long {
            Path::new(|b| {
                b.move_to(Point::new(x, y - size));
                b.line_to(Point::new(x - size, y + size));
                b.line_to(Point::new(x + size, y + size));
                b.close();
            })
        } else {
            Path::new(|b| {
                b.move_to(Point::new(x, y + size));
                b.line_to(Point::new(x - size, y - size));
                b.line_to(Point::new(x + size, y - size));
                b.close();
            })
        };
        frame.fill(
            &triangle,
            Fill {
                style: color.into(),
                ..Default::default()
            },
        );

        // Vertical dashed line
        draw_dashed_v_line(frame, x, PAD_TOP, y - size - 2.0, color);
    }

    fn draw_exit_marker(&self, frame: &mut Frame, x: f32, y: f32, color: Color) {
        // X mark at exit
        let size = 4.0_f32;
        let line1 = Path::line(
            Point::new(x - size, y - size),
            Point::new(x + size, y + size),
        );
        let line2 = Path::line(
            Point::new(x + size, y - size),
            Point::new(x - size, y + size),
        );
        let stroke = Stroke {
            style: color.into(),
            width: 2.0,
            ..Default::default()
        };
        frame.stroke(&line1, stroke);
        frame.stroke(&line2, stroke);

        // Vertical dashed line
        draw_dashed_v_line(frame, x, PAD_TOP, y - size - 2.0, color);
    }

    fn draw_y_labels(&self, frame: &mut Frame, layout: &ChartLayout, bounds: Rectangle) {
        let color = tokens::backtest::AXIS_TEXT;

        // Generate nice round price levels for the Y-axis
        if layout.price_range <= 0.0 {
            return;
        }

        let step = nice_step(layout.price_range, 5);
        if step <= 0.0 {
            return;
        }

        let first = (layout.price_min / step).ceil() * step;
        let mut price = first;
        while price <= layout.price_min + layout.price_range {
            let y = layout.price_to_y(price);
            if y >= PAD_TOP && y <= bounds.height - PAD_BOTTOM {
                frame.fill_text(Text {
                    content: format!("{:.2}", price),
                    position: Point::new(2.0, y - 5.0),
                    color,
                    size: iced::Pixels(9.0),
                    ..Default::default()
                });
            }
            price += step;
        }
    }

    fn draw_fallback(&self, frame: &mut Frame, bounds: Rectangle) {
        let entry = self.trade.entry_price.to_f64();
        let exit = self.trade.exit_price.to_f64();
        let sl = self.trade.initial_stop_loss.to_f64();
        let tp = self.trade.initial_take_profit.map(|p| p.to_f64());

        let mut min_p = entry.min(exit).min(sl);
        let mut max_p = entry.max(exit).max(sl);
        if let Some(t) = tp {
            min_p = min_p.min(t);
            max_p = max_p.max(t);
        }
        let range = max_p - min_p;
        let pad_price = range * 0.05;
        min_p -= pad_price;
        max_p += pad_price;

        let layout = ChartLayout {
            usable_h: bounds.height - PAD_TOP - PAD_BOTTOM,
            price_min: min_p,
            price_range: max_p - min_p,
            n_candles: 1,
            cell_w: bounds.width - PAD_LEFT - PAD_RIGHT,
        };

        // SL / TP lines
        self.draw_sl_tp_lines(frame, &layout, bounds);
        self.draw_entry_exit_lines(frame, &layout, bounds);
        self.draw_y_labels(frame, &layout, bounds);

        // "No candle data" label
        frame.fill_text(Text {
            content: "No candle data available".to_string(),
            position: Point::new(bounds.width / 2.0 - 60.0, bounds.height / 2.0),
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.3),
            size: iced::Pixels(11.0),
            ..Default::default()
        });
    }

    fn draw_overlay(&self, frame: &mut Frame, cursor: Point, bounds: Rectangle) {
        // Only draw within chart area
        if cursor.x < PAD_LEFT
            || cursor.x > bounds.width - PAD_RIGHT
            || cursor.y < PAD_TOP
            || cursor.y > bounds.height - PAD_BOTTOM
        {
            return;
        }

        draw_crosshair_lines(frame, cursor, bounds.size(), PAD_LEFT.min(PAD_TOP));

        if let Some(snapshot) = self.snapshot
            && !snapshot.candles.is_empty()
        {
            let candles = &snapshot.candles;
            let layout = self.build_layout(bounds, candles);

            let price_at_y = layout.y_to_price(cursor.y);
            let candle_idx = layout.x_to_candle_idx(cursor.x);
            let candle = &candles[candle_idx];

            // Price label at cursor Y (right side)
            draw_price_pill(
                frame,
                bounds.width - PAD_RIGHT + 4.0,
                cursor.y,
                &format!("{:.2}", price_at_y),
                Color::from_rgba(1.0, 1.0, 1.0, 0.6),
            );

            // OHLCV tooltip
            let lines = vec![
                format!(
                    "O: {:.2}  H: {:.2}",
                    candle.open.to_f64(),
                    candle.high.to_f64()
                ),
                format!(
                    "L: {:.2}  C: {:.2}",
                    candle.low.to_f64(),
                    candle.close.to_f64()
                ),
                format!("Vol: {}", candle.volume()),
            ];

            let (tw, th) = tooltip_size(&lines);
            let pos = position_tooltip(cursor, tw, th, bounds.size());
            draw_tooltip_box(frame, pos, &lines);
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

fn ordered(a: f32, b: f32) -> (f32, f32) {
    if a < b { (a, b) } else { (b, a) }
}

fn draw_dashed_h_line(frame: &mut Frame, x_start: f32, x_end: f32, y: f32, color: Color) {
    let dash = 4.0_f32;
    let gap = 3.0_f32;
    let mut x = x_start;
    while x < x_end {
        let end = (x + dash).min(x_end);
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
}

fn draw_dashed_v_line(frame: &mut Frame, x: f32, y_start: f32, y_end: f32, color: Color) {
    let dash = 4.0_f32;
    let gap = 3.0_f32;
    let (y_start, y_end) = ordered(y_start, y_end);
    let mut y = y_start;
    while y < y_end {
        let end = (y + dash).min(y_end);
        let seg = Path::line(Point::new(x, y), Point::new(x, end));
        frame.stroke(
            &seg,
            Stroke {
                style: color.into(),
                width: 1.0,
                ..Default::default()
            },
        );
        y += dash + gap;
    }
}

/// Draw a small text label on a semi-transparent dark pill background.
fn draw_price_pill(frame: &mut Frame, x: f32, y: f32, label: &str, color: Color) {
    let font_size = 9.0_f32;
    let pill_h = font_size + 4.0;
    let pill_w = label.len() as f32 * 5.5 + 6.0;
    let pill_y = y - pill_h / 2.0;

    // Background pill
    let bg = Path::rectangle(Point::new(x, pill_y), Size::new(pill_w, pill_h));
    frame.fill(
        &bg,
        Fill {
            style: Color::from_rgba(0.08, 0.08, 0.1, 0.85).into(),
            ..Default::default()
        },
    );

    // Text
    frame.fill_text(Text {
        content: label.to_string(),
        position: Point::new(x + 3.0, pill_y + 1.0),
        color,
        size: iced::Pixels(font_size),
        ..Default::default()
    });
}

/// Compute a nice round step size for grid labels.
fn nice_step(range: f64, target_lines: usize) -> f64 {
    let raw = range / target_lines as f64;
    let magnitude = 10.0_f64.powf(raw.log10().floor());
    let normalized = raw / magnitude;
    let nice = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice * magnitude
}
