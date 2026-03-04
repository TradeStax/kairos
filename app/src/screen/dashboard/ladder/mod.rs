//! Orderbook Ladder - Replay Mode
//!
//! Displays orderbook depth levels with prices and quantities in a ladder view.

#[cfg(feature = "heatmap")]
pub mod config;
#[cfg(feature = "heatmap")]
pub mod domain;
#[cfg(feature = "heatmap")]
mod render;
#[cfg(feature = "heatmap")]
pub mod types;

use std::time::Instant;

#[derive(Debug, Clone, Copy)]
pub enum Message {
    Scrolled(f32),
    ResetScroll,
    Invalidate(Option<Instant>),
}

// ── Feature-gated implementation ───────────────────────────────────

#[cfg(feature = "heatmap")]
use types::*;

#[cfg(feature = "heatmap")]
use crate::style::tokens;
#[cfg(feature = "heatmap")]
use data::{Depth, FuturesTickerInfo, Price, PriceExt, PriceStep, Side, Trade};
#[cfg(feature = "heatmap")]
use iced::{
    Element, padding,
    widget::{canvas, center, container, text},
};
#[cfg(feature = "heatmap")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "heatmap")]
use std::collections::BTreeMap;

/// Ladder runtime configuration.
#[cfg(feature = "heatmap")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub levels: usize,
    pub group_by_ticks: usize,
    pub show_chase: bool,
    pub show_chase_tracker: bool,
    pub show_spread: bool,
    pub trade_retention: std::time::Duration,
}

#[cfg(feature = "heatmap")]
impl Default for Config {
    fn default() -> Self {
        Self {
            levels: 20,
            group_by_ticks: 1,
            show_chase: true,
            show_chase_tracker: true,
            show_spread: true,
            trade_retention: std::time::Duration::from_secs(300),
        }
    }
}

#[cfg(feature = "heatmap")]
pub fn view(ladder: &Ladder, _timezone: crate::config::UserTimezone) -> Element<'_, Message> {
    if ladder.is_empty() {
        return center(text("Waiting for data...").size(tokens::text::HEADING)).into();
    }

    container(
        canvas(ladder)
            .height(iced::Length::Fill)
            .width(iced::Length::Fill),
    )
    .padding(
        padding::left(tokens::spacing::XXXS)
            .right(tokens::spacing::XXXS)
            .bottom(tokens::spacing::XXXS),
    )
    .into()
}

#[cfg(feature = "heatmap")]
pub fn update(ladder: &mut Ladder, message: Message) {
    match message {
        Message::Scrolled(delta) => {
            ladder.scroll(delta);
        }
        Message::ResetScroll => {
            ladder.reset_scroll();
        }
        Message::Invalidate(now) => {
            ladder.invalidate(now);
        }
    }
}

#[cfg(feature = "heatmap")]
pub struct Ladder {
    pub(super) ticker_info: FuturesTickerInfo,
    pub config: Config,
    pub(super) cache: canvas::Cache,
    last_tick: Instant,
    pub(super) tick_size: PriceStep,
    pub(super) scroll_px: f32,
    last_exchange_ts_ms: Option<u64>,
    pub(super) orderbook: [GroupedDepth; 2],
    pub(super) trades: TradeStore,
    pending_tick_size: Option<PriceStep>,
    pub(super) raw_price_spread: Option<Price>,
}

#[cfg(feature = "heatmap")]
impl Ladder {
    pub fn new(config: Option<Config>, ticker_info: FuturesTickerInfo, tick_size: f32) -> Self {
        Self {
            trades: TradeStore::new(),
            config: config.unwrap_or_default(),
            ticker_info,
            cache: canvas::Cache::default(),
            last_tick: Instant::now(),
            tick_size: PriceStep::from_f32(tick_size),
            scroll_px: 0.0,
            last_exchange_ts_ms: None,
            orderbook: [GroupedDepth::new(), GroupedDepth::new()],
            raw_price_spread: None,
            pending_tick_size: None,
        }
    }

    // ── Inlined Panel trait methods ────────────────────────────────

    pub fn scroll(&mut self, delta: f32) {
        self.scroll_px += delta;
        Ladder::invalidate(self, Some(Instant::now()));
    }

    pub fn reset_scroll(&mut self) {
        self.scroll_px = 0.0;
        Ladder::invalidate(self, Some(Instant::now()));
    }

    pub fn is_empty(&self) -> bool {
        if self.pending_tick_size.is_some() {
            return true;
        }
        self.orderbook[Side::Ask.idx()].orders.is_empty()
            && self.orderbook[Side::Bid.idx()].orders.is_empty()
            && self.trades.is_empty()
    }

    // ── Data update ───────────────────────────────────────────────

    /// Update from replay engine with domain types
    pub fn update_from_replay(&mut self, depth: &Depth, trades: &[Trade]) {
        if let Some(next) = self.pending_tick_size.take() {
            self.tick_size = next;
            self.trades.rebuild_grouped(self.tick_size);
        }

        // Convert domain depth to internal processing format
        let mut ex_bids = BTreeMap::new();
        let mut ex_asks = BTreeMap::new();

        for (price, qty) in &depth.bids {
            ex_bids.insert(*price, *qty);
        }

        for (price, qty) in &depth.asks {
            ex_asks.insert(*price, *qty);
        }

        // Calculate raw spread
        let raw_best_bid = ex_bids.last_key_value().map(|(p, _)| Price::from_units(*p));
        let raw_best_ask = ex_asks
            .first_key_value()
            .map(|(p, _)| Price::from_units(*p));
        self.raw_price_spread = match (raw_best_bid, raw_best_ask) {
            (Some(bid), Some(ask)) => Some(ask - bid),
            _ => None,
        };

        let update_t = depth.time;

        // Update chase trackers
        if self.config.show_chase_tracker {
            let max_int = CHASE_MIN_INTERVAL.as_millis() as u64;
            self.chase_tracker_mut(Side::Bid).update(
                raw_best_bid.map(|p| p.units()),
                true,
                update_t,
                max_int,
            );
            self.chase_tracker_mut(Side::Ask).update(
                raw_best_ask.map(|p| p.units()),
                false,
                update_t,
                max_int,
            );
        } else {
            self.chase_tracker_mut(Side::Bid).reset();
            self.chase_tracker_mut(Side::Ask).reset();
        }

        // Insert trades (convert domain trades to internal format)
        let step = self.tick_size;
        for trade in trades {
            let ex_price = trade.price;
            let is_sell = trade.side == data::Side::Sell;
            self.trades.insert_trade(
                trade.time.to_millis(),
                ex_price,
                trade.quantity.0 as f32,
                is_sell,
                step,
            );
        }

        // Regroup depth from converted exchange types
        self.orderbook[Side::Ask.idx()].regroup_from_btree(&ex_asks, Side::Ask, step);
        self.orderbook[Side::Bid.idx()].regroup_from_btree(&ex_bids, Side::Bid, step);

        self.last_exchange_ts_ms = Some(update_t);

        if self.trades.maybe_cleanup(
            update_t,
            self.config.trade_retention.as_millis() as u64,
            self.tick_size,
        ) {
            self.invalidate(Some(Instant::now()));
        }
    }

    // ── Accessors ─────────────────────────────────────────────────

    pub(super) fn trade_qty_at(&self, price: Price) -> (f32, f32) {
        self.trades.trade_qty_at(price)
    }

    pub fn last_update(&self) -> Instant {
        self.last_tick
    }

    pub(super) fn grouped_asks(&self) -> &BTreeMap<Price, f32> {
        &self.orderbook[Side::Ask.idx()].orders
    }

    pub(super) fn grouped_bids(&self) -> &BTreeMap<Price, f32> {
        &self.orderbook[Side::Bid.idx()].orders
    }

    pub(super) fn chase_tracker(&self, side: Side) -> &ChaseTracker {
        &self.orderbook[side.idx()].chase
    }

    fn chase_tracker_mut(&mut self, side: Side) -> &mut ChaseTracker {
        &mut self.orderbook[side.idx()].chase
    }

    pub(super) fn best_price(&self, side: Side) -> Option<Price> {
        self.orderbook[side.idx()].best_price(side)
    }

    pub fn min_tick_size(&self) -> f32 {
        self.ticker_info.tick_size
    }

    // ── Mutators ──────────────────────────────────────────────────

    pub fn set_tick_size(&mut self, tick_size: f32) {
        let step = PriceStep::from_f32(tick_size);
        self.pending_tick_size = Some(step);
        self.invalidate(Some(Instant::now()));
    }

    pub fn set_show_chase_tracker(&mut self, enabled: bool) {
        if self.config.show_chase_tracker != enabled {
            self.config.show_chase_tracker = enabled;
            if !enabled {
                self.chase_tracker_mut(Side::Bid).reset();
                self.chase_tracker_mut(Side::Ask).reset();
            }

            self.invalidate(Some(Instant::now()));
        }
    }

    pub fn invalidate(&mut self, now: Option<Instant>) {
        self.cache.clear();
        if let Some(now) = now {
            self.last_tick = now;
        }
    }

    pub fn tick_size(&self) -> f32 {
        self.tick_size.to_f32_lossy()
    }

    pub(super) fn format_price(&self, price: Price) -> String {
        let precision_f32 = self.ticker_info.tick_size;
        let precision = data::MinTicksize::from(precision_f32);
        price.fmt_with_precision(precision)
    }

    pub(super) fn format_quantity(&self, qty: f32) -> String {
        data::util::formatting::abbr_large_numbers(qty)
    }
}
