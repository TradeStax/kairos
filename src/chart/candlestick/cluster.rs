use data::util::abbr_large_numbers;
use exchange::util::{Price, PriceStep};
use iced::theme::palette::Extended;
use iced::widget::canvas;
use iced::{Alignment, Point, Size};
use std::collections::BTreeMap;

use super::TradeGroup;
use super::footprint::{BidAskArea, ContentGaps, draw_cluster_text, draw_imbalance_markers};

/// Shared renderer for BidAsk-style cluster layouts (BidAsk, Delta, Volume, Trades).
/// Deduplicates ~120 lines that were previously copy-pasted between cluster_kind arms.
#[allow(clippy::too_many_arguments)]
pub fn draw_bidask_clusters(
    frame: &mut canvas::Frame,
    price_to_y: &impl Fn(Price) -> f32,
    area: &BidAskArea,
    cell_height: f32,
    max_cluster_qty: f32,
    palette: &Extended,
    text_size: f32,
    tick_size: f32,
    show_text: bool,
    imbalance: Option<(usize, Option<usize>, bool)>,
    footprint: &BTreeMap<Price, TradeGroup>,
    spacing: ContentGaps,
) {
    let text_color = palette.background.weakest.text;
    let bar_alpha = if show_text { 0.25 } else { 1.0 };

    let imb_marker_reserve = if imbalance.is_some() {
        ((area.imb_marker_width - 1.0) / 2.0).max(1.0)
    } else {
        0.0
    };

    let right_max_x = area.bid_area_right - imb_marker_reserve - (2.0 * spacing.marker_to_bars);
    let right_area_width = (right_max_x - area.bid_area_left).max(0.0);

    let left_min_x = area.ask_area_left + imb_marker_reserve + (2.0 * spacing.marker_to_bars);
    let left_area_width = (area.ask_area_right - left_min_x).max(0.0);

    for (price, group) in footprint {
        let y = price_to_y(*price);

        if group.buy_qty > 0.0 && right_area_width > 0.0 {
            if show_text {
                draw_cluster_text(
                    frame,
                    &abbr_large_numbers(group.buy_qty),
                    Point::new(area.bid_area_left, y),
                    text_size,
                    text_color,
                    Alignment::Start,
                    Alignment::Center,
                );
            }

            let bar_width = (group.buy_qty / max_cluster_qty) * right_area_width;
            if bar_width > 0.0 {
                frame.fill_rectangle(
                    Point::new(area.bid_area_left, y - (cell_height / 2.0)),
                    Size::new(bar_width, cell_height),
                    palette.success.base.color.scale_alpha(bar_alpha),
                );
            }
        }
        if group.sell_qty > 0.0 && left_area_width > 0.0 {
            if show_text {
                draw_cluster_text(
                    frame,
                    &abbr_large_numbers(group.sell_qty),
                    Point::new(area.ask_area_right, y),
                    text_size,
                    text_color,
                    Alignment::End,
                    Alignment::Center,
                );
            }

            let bar_width = (group.sell_qty / max_cluster_qty) * left_area_width;
            if bar_width > 0.0 {
                frame.fill_rectangle(
                    Point::new(area.ask_area_right, y - (cell_height / 2.0)),
                    Size::new(-bar_width, cell_height),
                    palette.danger.base.color.scale_alpha(bar_alpha),
                );
            }
        }

        if let Some((threshold, color_scale, ignore_zeros)) = imbalance
            && area.imb_marker_width > 0.0
        {
            let step = PriceStep::from_f32(tick_size);
            let higher_price = Price::from_f32(price.to_f32() + tick_size).round_to_step(step);

            let rect_width = ((area.imb_marker_width - 1.0) / 2.0).max(1.0);

            let buyside_x = area.bid_area_right - rect_width - spacing.marker_to_bars;
            let sellside_x = area.ask_area_left + spacing.marker_to_bars;

            draw_imbalance_markers(
                frame,
                price_to_y,
                footprint,
                *price,
                group.sell_qty,
                higher_price,
                threshold as u8,
                color_scale.is_some(),
                ignore_zeros,
                cell_height,
                palette,
                buyside_x,
                sellside_x,
                rect_width,
            );
        }
    }
}
