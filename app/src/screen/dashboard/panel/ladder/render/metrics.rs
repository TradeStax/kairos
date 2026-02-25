//! Price grid construction, visible row calculation, and render primitives.

use super::super::types::*;
use super::super::Ladder;
use crate::components::primitives::AZERET_MONO;

use exchange::util::Price as ExPrice;

use iced::widget::canvas::{Path, Stroke, Text};
use iced::{Alignment, Point, Rectangle, Size};

impl Ladder {
    pub(super) fn draw_row(
        &self,
        frame: &mut iced::widget::canvas::Frame,
        y: f32,
        price: ExPrice,
        order_qty: f32,
        is_bid: bool,
        side_color: iced::Color,
        text_color: iced::Color,
        max_order_qty: f32,
        trade_buy_qty: f32,
        trade_sell_qty: f32,
        max_trade_qty: f32,
        trade_buy_color: iced::Color,
        trade_sell_color: iced::Color,
        cols: &ColumnRanges,
    ) {
        if is_bid {
            Self::fill_bar(
                frame,
                cols.bid_order,
                y,
                ROW_HEIGHT,
                order_qty,
                max_order_qty,
                side_color,
                true,
                0.20,
            );
            let qty_txt = self.format_quantity(order_qty);
            let x_text = cols.bid_order.0 + 6.0;
            Self::draw_cell_text(
                frame, &qty_txt, x_text, y, text_color, Alignment::Start,
            );
        } else {
            Self::fill_bar(
                frame,
                cols.ask_order,
                y,
                ROW_HEIGHT,
                order_qty,
                max_order_qty,
                side_color,
                false,
                0.20,
            );
            let qty_txt = self.format_quantity(order_qty);
            let x_text = cols.ask_order.1 - 6.0;
            Self::draw_cell_text(
                frame, &qty_txt, x_text, y, text_color, Alignment::End,
            );
        }

        // Sell trades (right-to-left)
        Self::fill_bar(
            frame,
            cols.sell,
            y,
            ROW_HEIGHT,
            trade_sell_qty,
            max_trade_qty,
            trade_sell_color,
            false,
            0.30,
        );
        let sell_txt = if trade_sell_qty > 0.0 {
            self.format_quantity(trade_sell_qty)
        } else {
            "".into()
        };
        Self::draw_cell_text(
            frame,
            &sell_txt,
            cols.sell.1 - 6.0,
            y,
            text_color,
            Alignment::End,
        );

        // Buy trades (left-to-right)
        Self::fill_bar(
            frame,
            cols.buy,
            y,
            ROW_HEIGHT,
            trade_buy_qty,
            max_trade_qty,
            trade_buy_color,
            true,
            0.30,
        );
        let buy_txt = if trade_buy_qty > 0.0 {
            self.format_quantity(trade_buy_qty)
        } else {
            "".into()
        };
        Self::draw_cell_text(
            frame,
            &buy_txt,
            cols.buy.0 + 6.0,
            y,
            text_color,
            Alignment::Start,
        );

        // Price
        let price_text = self.format_price(price);
        let price_x_center = (cols.price.0 + cols.price.1) * 0.5;
        Self::draw_cell_text(
            frame,
            &price_text,
            price_x_center,
            y,
            side_color,
            Alignment::Center,
        );
    }

    pub(super) fn fill_bar(
        frame: &mut iced::widget::canvas::Frame,
        (x_start, x_end): (f32, f32),
        y: f32,
        height: f32,
        value: f32,
        scale_value_max: f32,
        color: iced::Color,
        from_left: bool,
        alpha: f32,
    ) {
        if scale_value_max <= 0.0 || value <= 0.0 {
            return;
        }
        let col_width = x_end - x_start;

        let mut bar_width = (value / scale_value_max) * col_width.max(1.0);
        bar_width = bar_width.min(col_width);
        let bar_x = if from_left {
            x_start
        } else {
            x_end - bar_width
        };

        frame.fill_rectangle(
            Point::new(bar_x, y),
            Size::new(bar_width, height),
            iced::Color { a: alpha, ..color },
        );
    }

    pub(super) fn draw_cell_text(
        frame: &mut iced::widget::canvas::Frame,
        text: &str,
        x_anchor: f32,
        y: f32,
        color: iced::Color,
        align: Alignment,
    ) {
        frame.fill_text(Text {
            content: text.to_string(),
            position: Point::new(x_anchor, y + ROW_HEIGHT / 2.0),
            color,
            size: TEXT_SIZE.into(),
            font: AZERET_MONO,
            align_x: align.into(),
            align_y: Alignment::Center.into(),
            ..Default::default()
        });
    }

    pub(super) fn draw_chase_trail(
        &self,
        frame: &mut iced::widget::canvas::Frame,
        grid: &PriceGrid,
        bounds: Rectangle,
        tracker: &ChaseTracker,
        pos_x: f32,
        best_offer_y: Option<f32>,
        color: iced::Color,
        is_bid: bool,
    ) {
        let radius = CHASE_CIRCLE_RADIUS;
        if let Some((start_p_units, end_p_units, alpha)) = tracker.segment() {
            let start_p = ExPrice::from_units(start_p_units)
                .round_to_side_step(is_bid, grid.tick.into());
            let end_p = ExPrice::from_units(end_p_units)
                .round_to_side_step(is_bid, grid.tick.into());

            let color = color.scale_alpha(alpha);
            let stroke_w = 2.0;
            let pad_to_circle = radius + stroke_w * 0.5;

            let start_y = self.price_to_screen_y(start_p, grid, bounds.height);
            let end_y = self
                .price_to_screen_y(end_p, grid, bounds.height)
                .or(best_offer_y);

            if let Some(end_y) = end_y {
                if let Some(start_y) = start_y {
                    let dy = end_y - start_y;
                    if dy.abs() > pad_to_circle {
                        let line_end_y =
                            end_y - dy.signum() * pad_to_circle;
                        let line_path = Path::line(
                            Point::new(pos_x, start_y),
                            Point::new(pos_x, line_end_y),
                        );
                        frame.stroke(
                            &line_path,
                            Stroke::default()
                                .with_color(color)
                                .with_width(stroke_w),
                        );
                    }
                }

                let circle = &Path::circle(Point::new(pos_x, end_y), radius);
                frame.fill(circle, color);
            }
        }
    }

    pub(super) fn build_price_grid(&self) -> Option<PriceGrid> {
        let best_bid =
            match (self.best_price(data::Side::Bid), self.best_price(data::Side::Ask)) {
                (Some(bb), _) => bb,
                (None, Some(ba)) => {
                    ba.add_steps(-1, self.tick_size.into())
                }
                (None, None) => {
                    let (min_t, max_t) = self.trades.price_range()?;
                    let steps = ExPrice::steps_between_inclusive(
                        min_t,
                        max_t,
                        self.tick_size.into(),
                    )
                    .unwrap_or(1);
                    max_t.add_steps(
                        -(steps as i64 / 2),
                        self.tick_size.into(),
                    )
                }
            };
        let best_ask = best_bid.add_steps(1, self.tick_size.into());

        Some(PriceGrid {
            best_bid,
            best_ask,
            tick: self.tick_size,
        })
    }

    pub(super) fn visible_rows(
        &self,
        bounds: Rectangle,
        grid: &PriceGrid,
    ) -> (Vec<VisibleRow>, Maxima) {
        let asks_grouped = self.grouped_asks();
        let bids_grouped = self.grouped_bids();

        let mut visible: Vec<VisibleRow> = Vec::new();
        let mut maxima = Maxima::default();

        let mid_screen_y = bounds.height * 0.5;
        let scroll = self.scroll_px;

        let y0 = mid_screen_y + PriceGrid::top_y(0) - scroll;
        let idx_top = ((0.0 - y0) / ROW_HEIGHT).floor() as i32;

        let rows_needed = (bounds.height / ROW_HEIGHT).ceil() as i32 + 1;
        let idx_bottom = idx_top + rows_needed;

        for idx in idx_top..=idx_bottom {
            if idx == 0 {
                let top_y_screen =
                    mid_screen_y + PriceGrid::top_y(0) - scroll;
                if top_y_screen < bounds.height
                    && top_y_screen + ROW_HEIGHT > 0.0
                {
                    let row = if self.config.show_spread {
                        DomRow::Spread
                    } else {
                        DomRow::CenterDivider
                    };

                    visible.push(VisibleRow {
                        row,
                        y: top_y_screen,
                        buy_t: 0.0,
                        sell_t: 0.0,
                    });
                }
                continue;
            }

            let Some(price) = grid.index_to_price(idx) else {
                continue;
            };

            let is_bid = idx > 0;
            let order_qty = if is_bid {
                bids_grouped.get(&price).copied().unwrap_or(0.0)
            } else {
                asks_grouped.get(&price).copied().unwrap_or(0.0)
            };

            let top_y_screen =
                mid_screen_y + PriceGrid::top_y(idx) - scroll;
            if top_y_screen >= bounds.height
                || top_y_screen + ROW_HEIGHT <= 0.0
            {
                continue;
            }

            maxima.vis_max_order_qty =
                maxima.vis_max_order_qty.max(order_qty);
            let (buy_t, sell_t) = self.trade_qty_at(price);
            maxima.vis_max_trade_qty =
                maxima.vis_max_trade_qty.max(buy_t.max(sell_t));

            let row = if is_bid {
                DomRow::Bid {
                    price,
                    qty: order_qty,
                }
            } else {
                DomRow::Ask {
                    price,
                    qty: order_qty,
                }
            };

            visible.push(VisibleRow {
                row,
                y: top_y_screen,
                buy_t,
                sell_t,
            });
        }

        visible.sort_by(|a, b| a.y.total_cmp(&b.y));
        (visible, maxima)
    }

    pub(super) fn price_to_screen_y(
        &self,
        price: ExPrice,
        grid: &PriceGrid,
        bounds_height: f32,
    ) -> Option<f32> {
        let mid_screen_y = bounds_height * 0.5;
        let scroll = self.scroll_px;

        let idx = if price >= grid.best_ask {
            let steps = ExPrice::steps_between_inclusive(
                grid.best_ask,
                price,
                grid.tick.into(),
            )?;
            -(steps as i32)
        } else if price <= grid.best_bid {
            let steps = ExPrice::steps_between_inclusive(
                price,
                grid.best_bid,
                grid.tick.into(),
            )?;
            steps as i32
        } else {
            return Some(mid_screen_y - scroll);
        };

        let y = mid_screen_y + PriceGrid::top_y(idx) - scroll
            + ROW_HEIGHT / 2.0;
        Some(y)
    }
}
