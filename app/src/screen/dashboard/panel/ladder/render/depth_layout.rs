//! Layout arithmetic for the ladder price column and column ranges.

use super::super::types::*;
use super::super::Ladder;

impl Ladder {
    // [BidOrderQty][SellQty][ Price ][BuyQty][AskOrderQty]
    pub(super) const NUMBER_OF_COLUMN_GAPS: f32 = 4.0;

    pub(super) fn price_sample_text(&self, grid: &PriceGrid) -> String {
        let a = self.format_price(grid.best_ask);
        let b = self.format_price(grid.best_bid);
        if a.len() >= b.len() { a } else { b }
    }

    pub(super) fn mono_text_width_px(text_len: usize) -> f32 {
        (text_len as f32) * TEXT_SIZE * MONO_CHAR_ADVANCE
    }

    pub(super) fn price_layout_for(
        &self,
        total_width: f32,
        grid: &PriceGrid,
    ) -> PriceLayout {
        let sample = self.price_sample_text(grid);
        let text_px = Self::mono_text_width_px(sample.len());

        let desired_total_gap = CHASE_CIRCLE_RADIUS * 2.0 + 4.0;
        let inside_pad_px = PRICE_TEXT_SIDE_PAD_MIN
            .max(desired_total_gap - COL_PADDING)
            .max(0.0);

        let price_px = (text_px + 2.0 * inside_pad_px).min(total_width.max(0.0));

        PriceLayout {
            price_px,
            inside_pad_px,
        }
    }

    pub(super) fn column_ranges(
        &self,
        width: f32,
        price_px: f32,
    ) -> ColumnRanges {
        let total_gutter_width = COL_PADDING * Self::NUMBER_OF_COLUMN_GAPS;
        let usable_width = (width - total_gutter_width).max(0.0);

        let price_width = price_px.min(usable_width);

        let rest = (usable_width - price_width).max(0.0);
        let rest_ratio = ORDER_QTY_COLS_WIDTH + TRADE_QTY_COLS_WIDTH; // 0.80

        let order_share = if rest_ratio > 0.0 {
            (ORDER_QTY_COLS_WIDTH / rest_ratio) * rest
        } else {
            0.0
        };
        let trade_share = if rest_ratio > 0.0 {
            (TRADE_QTY_COLS_WIDTH / rest_ratio) * rest
        } else {
            0.0
        };

        let bid_order_width = order_share * 0.5;
        let sell_trades_width = trade_share * 0.5;
        let buy_trades_width = trade_share * 0.5;
        let ask_order_width = order_share * 0.5;

        let mut cursor_x = 0.0;

        let bid_order_end = cursor_x + bid_order_width;
        let bid_order_range = (cursor_x, bid_order_end);
        cursor_x = bid_order_end + COL_PADDING;

        let sell_trades_end = cursor_x + sell_trades_width;
        let sell_trades_range = (cursor_x, sell_trades_end);
        cursor_x = sell_trades_end + COL_PADDING;

        let price_end = cursor_x + price_width;
        let price_range = (cursor_x, price_end);
        cursor_x = price_end + COL_PADDING;

        let buy_trades_end = cursor_x + buy_trades_width;
        let buy_trades_range = (cursor_x, buy_trades_end);
        cursor_x = buy_trades_end + COL_PADDING;

        let ask_order_end = cursor_x + ask_order_width;
        let ask_order_range = (cursor_x, ask_order_end);

        ColumnRanges {
            bid_order: bid_order_range,
            sell: sell_trades_range,
            price: price_range,
            buy: buy_trades_range,
            ask_order: ask_order_range,
        }
    }
}
