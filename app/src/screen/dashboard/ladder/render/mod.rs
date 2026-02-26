mod depth_layout;
mod metrics;

use super::Ladder;
use super::types::*;
use crate::style;

use data::PriceExt;
use iced::widget::canvas::{self, Text};
use iced::{Alignment, Event, Point, Rectangle, Renderer, Size, Theme, mouse};

impl canvas::Program<super::Message> for Ladder {
    type State = ();

    fn update(
        &self,
        _state: &mut Self::State,
        event: &iced::Event,
        bounds: iced::Rectangle,
        cursor: iced_core::mouse::Cursor,
    ) -> Option<canvas::Action<super::Message>> {
        let _cursor_position = cursor.position_in(bounds)?;

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Middle | mouse::Button::Left | mouse::Button::Right,
            )) => Some(canvas::Action::publish(super::Message::ResetScroll).and_capture()),
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let scroll_amount = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => -(*y) * ROW_HEIGHT,
                    mouse::ScrollDelta::Pixels { y, .. } => -*y,
                };

                Some(
                    canvas::Action::publish(super::Message::Scrolled(scroll_amount))
                        .and_capture(),
                )
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: iced_core::mouse::Cursor,
    ) -> Vec<iced::widget::canvas::Geometry<Renderer>> {
        let palette = theme.extended_palette();

        let text_color = palette.background.base.text;
        let bid_color = palette.success.base.color;
        let ask_color = palette.danger.base.color;

        let divider_color = style::split_ruler(theme).color;

        let orderbook_visual = self.cache.draw(renderer, bounds.size(), |frame| {
            if let Some(grid) = self.build_price_grid() {
                let layout = self.price_layout_for(bounds.width, &grid);
                let cols = self.column_ranges(bounds.width, layout.price_px);

                let (visible_rows, maxima) = self.visible_rows(bounds, &grid);

                let mut spread_row: Option<(f32, f32)> = None;
                let mut best_bid_y: Option<f32> = None;
                let mut best_ask_y: Option<f32> = None;

                for visible_row in visible_rows.iter() {
                    match visible_row.row {
                        DomRow::Ask { price, .. }
                            if Some(price)
                                == self.grouped_asks().first_key_value().map(|(p, _)| *p) =>
                        {
                            best_ask_y = Some(visible_row.y);
                        }
                        DomRow::Bid { price, .. }
                            if Some(price)
                                == self.grouped_bids().last_key_value().map(|(p, _)| *p) =>
                        {
                            best_bid_y = Some(visible_row.y);
                        }
                        _ => {}
                    }

                    match visible_row.row {
                        DomRow::Ask { price, qty } => {
                            self.draw_row(
                                frame,
                                visible_row.y,
                                price,
                                qty,
                                false,
                                ask_color,
                                text_color,
                                maxima.vis_max_order_qty,
                                visible_row.buy_t,
                                visible_row.sell_t,
                                maxima.vis_max_trade_qty,
                                bid_color,
                                ask_color,
                                &cols,
                            );
                        }
                        DomRow::Bid { price, qty } => {
                            self.draw_row(
                                frame,
                                visible_row.y,
                                price,
                                qty,
                                true,
                                bid_color,
                                text_color,
                                maxima.vis_max_order_qty,
                                visible_row.buy_t,
                                visible_row.sell_t,
                                maxima.vis_max_trade_qty,
                                bid_color,
                                ask_color,
                                &cols,
                            );
                        }
                        DomRow::Spread => {
                            if let Some(spread) = self.raw_price_spread {
                                let min_ticksize_f32 = self.ticker_info.tick_size;
                                let min_ticksize = data::MinTicksize::from(min_ticksize_f32);
                                spread_row = Some((visible_row.y, visible_row.y + ROW_HEIGHT));

                                let spread = spread.round_to_min_tick(min_ticksize);
                                let content =
                                    format!("Spread: {}", spread.fmt_with_precision(min_ticksize));
                                frame.fill_text(Text {
                                    content,
                                    position: Point::new(
                                        bounds.width / 2.0,
                                        visible_row.y + ROW_HEIGHT / 2.0,
                                    ),
                                    color: palette.secondary.strong.color,
                                    size: (TEXT_SIZE - 1.0).into(),
                                    font: crate::components::primitives::AZERET_MONO,
                                    align_x: Alignment::Center.into(),
                                    align_y: Alignment::Center.into(),
                                    ..Default::default()
                                });
                            }
                        }
                        DomRow::CenterDivider => {
                            let y_mid = visible_row.y + ROW_HEIGHT / 2.0 - 0.5;

                            frame.fill_rectangle(
                                Point::new(0.0, y_mid),
                                Size::new(bounds.width, 1.0),
                                divider_color,
                            );
                        }
                    }
                }

                if self.config.show_chase_tracker {
                    let left_gap_mid_x = cols.sell.1 + (layout.inside_pad_px + COL_PADDING) * 0.5;
                    let right_gap_mid_x = cols.buy.0 - (layout.inside_pad_px + COL_PADDING) * 0.5;

                    self.draw_chase_trail(
                        frame,
                        &grid,
                        bounds,
                        self.chase_tracker(data::Side::Bid),
                        right_gap_mid_x,
                        best_ask_y.map(|y| y + ROW_HEIGHT / 2.0),
                        palette.success.weak.color,
                        true, // is_bid
                    );
                    self.draw_chase_trail(
                        frame,
                        &grid,
                        bounds,
                        self.chase_tracker(data::Side::Ask),
                        left_gap_mid_x,
                        best_bid_y.map(|y| y + ROW_HEIGHT / 2.0),
                        palette.danger.weak.color,
                        false,
                    );
                }

                // Price column vertical dividers with a gap over the spread row
                let mut draw_vsplit = |x: f32, gap: Option<(f32, f32)>| {
                    let x = x.floor() + 0.5;
                    match gap {
                        Some((top, bottom)) => {
                            if top > 0.0 {
                                frame.fill_rectangle(
                                    Point::new(x, 0.0),
                                    Size::new(1.0, top.max(0.0)),
                                    divider_color,
                                );
                            }
                            if bottom < bounds.height {
                                frame.fill_rectangle(
                                    Point::new(x, bottom),
                                    Size::new(1.0, (bounds.height - bottom).max(0.0)),
                                    divider_color,
                                );
                            }
                        }
                        None => {
                            frame.fill_rectangle(
                                Point::new(x, 0.0),
                                Size::new(1.0, bounds.height),
                                divider_color,
                            );
                        }
                    }
                };
                draw_vsplit(cols.sell.1, spread_row);
                draw_vsplit(cols.buy.0, spread_row);

                if let Some((top, bottom)) = spread_row {
                    let y_top: f32 = top.floor() + 0.5;
                    let y_bot = bottom.floor() + 0.5;

                    frame.fill_rectangle(
                        Point::new(0.0, y_top),
                        Size::new(cols.sell.1, 1.0),
                        divider_color,
                    );
                    frame.fill_rectangle(
                        Point::new(0.0, y_bot),
                        Size::new(cols.sell.1, 1.0),
                        divider_color,
                    );

                    frame.fill_rectangle(
                        Point::new(cols.buy.0, y_top),
                        Size::new(bounds.width - cols.buy.0, 1.0),
                        divider_color,
                    );
                    frame.fill_rectangle(
                        Point::new(cols.buy.0, y_bot),
                        Size::new(bounds.width - cols.buy.0, 1.0),
                        divider_color,
                    );
                }
            }
        });

        vec![orderbook_visual]
    }
}
