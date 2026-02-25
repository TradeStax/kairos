//! Orderbook Ladder Panel - Replay Mode
//!
//! Displays orderbook depth levels with prices and quantities in a ladder view.
//! Refactored for futures historical/replay mode with clean domain types.

mod render;
pub mod types;

use types::*;

use data::config::panel::ladder::Config;
use data::{DepthSnapshot, Side, Trade};
use exchange::TickerInfo;
use exchange::util::{Price as ExPrice, PriceExt, PriceStep};

use iced::widget::canvas;

use std::collections::BTreeMap;
use std::time::Instant;

impl super::Panel for Ladder {
    fn scroll(&mut self, delta: f32) {
        self.scroll_px += delta;
        Ladder::invalidate(self, Some(Instant::now()));
    }

    fn reset_scroll(&mut self) {
        self.scroll_px = 0.0;
        Ladder::invalidate(self, Some(Instant::now()));
    }

    fn invalidate(&mut self, now: Option<Instant>) -> Option<super::Action> {
        Ladder::invalidate(self, now)
    }

    fn is_empty(&self) -> bool {
        if self.pending_tick_size.is_some() {
            return true;
        }
        self.orderbook[Side::Ask.idx()].orders.is_empty()
            && self.orderbook[Side::Bid.idx()].orders.is_empty()
            && self.trades.is_empty()
    }
}

pub struct Ladder {
    pub(super) ticker_info: TickerInfo,
    pub config: Config,
    pub(super) cache: canvas::Cache,
    last_tick: Instant,
    pub(super) tick_size: PriceStep,
    pub(super) scroll_px: f32,
    last_exchange_ts_ms: Option<u64>,
    pub(super) orderbook: [GroupedDepth; 2],
    pub(super) trades: TradeStore,
    pending_tick_size: Option<PriceStep>,
    pub(super) raw_price_spread: Option<ExPrice>,
}

impl Ladder {
    pub fn new(
        config: Option<Config>,
        ticker_info: TickerInfo,
        tick_size: f32,
    ) -> Self {
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

    /// Update from replay engine with domain types
    pub fn update_from_replay(
        &mut self,
        depth: &DepthSnapshot,
        trades: &[Trade],
    ) {
        if let Some(next) = self.pending_tick_size.take() {
            self.tick_size = next;
            self.trades.rebuild_grouped(self.tick_size);
        }

        // Convert domain depth to exchange util types for internal processing
        let mut ex_bids = BTreeMap::new();
        let mut ex_asks = BTreeMap::new();

        for (price, qty) in &depth.bids {
            let ex_price = ExPrice::from(*price);
            ex_bids.insert(ex_price.units(), qty.0 as f32);
        }

        for (price, qty) in &depth.asks {
            let ex_price = ExPrice::from(*price);
            ex_asks.insert(ex_price.units(), qty.0 as f32);
        }

        // Calculate raw spread
        let raw_best_bid = ex_bids
            .last_key_value()
            .map(|(p, _)| ExPrice::from_units(*p));
        let raw_best_ask = ex_asks
            .first_key_value()
            .map(|(p, _)| ExPrice::from_units(*p));
        self.raw_price_spread = match (raw_best_bid, raw_best_ask) {
            (Some(bid), Some(ask)) => Some(ask - bid),
            _ => None,
        };

        let update_t = depth.time.to_millis();

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
            let ex_price = ExPrice::from(trade.price);
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
        self.orderbook[Side::Ask.idx()]
            .regroup_from_btree(&ex_asks, Side::Ask, step);
        self.orderbook[Side::Bid.idx()]
            .regroup_from_btree(&ex_bids, Side::Bid, step);

        self.last_exchange_ts_ms = Some(update_t);

        if self.trades.maybe_cleanup(
            update_t,
            self.config.trade_retention.as_millis() as u64,
            self.tick_size,
        ) {
            self.invalidate(Some(Instant::now()));
        }
    }

    pub(super) fn trade_qty_at(&self, price: ExPrice) -> (f32, f32) {
        self.trades.trade_qty_at(price)
    }

    pub fn last_update(&self) -> Instant {
        self.last_tick
    }

    pub(super) fn grouped_asks(&self) -> &BTreeMap<ExPrice, f32> {
        &self.orderbook[Side::Ask.idx()].orders
    }

    pub(super) fn grouped_bids(&self) -> &BTreeMap<ExPrice, f32> {
        &self.orderbook[Side::Bid.idx()].orders
    }

    pub(super) fn chase_tracker(&self, side: Side) -> &ChaseTracker {
        &self.orderbook[side.idx()].chase
    }

    fn chase_tracker_mut(&mut self, side: Side) -> &mut ChaseTracker {
        &mut self.orderbook[side.idx()].chase
    }

    pub(super) fn best_price(&self, side: Side) -> Option<ExPrice> {
        self.orderbook[side.idx()].best_price(side)
    }

    pub fn min_tick_size(&self) -> f32 {
        self.ticker_info.min_ticksize.to_f32_lossy()
    }

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

    pub fn invalidate(
        &mut self,
        now: Option<Instant>,
    ) -> Option<super::Action> {
        self.cache.clear();
        if let Some(now) = now {
            self.last_tick = now;
        }
        None
    }

    pub fn tick_size(&self) -> f32 {
        self.tick_size.to_f32_lossy()
    }

    pub(super) fn format_price(&self, price: ExPrice) -> String {
        let precision_f32 = self.ticker_info.min_ticksize.to_f32_lossy();
        let precision = exchange::util::MinTicksize::from(precision_f32);
        price.fmt_with_precision(precision)
    }

    pub(super) fn format_quantity(&self, qty: f32) -> String {
        data::util::formatting::abbr_large_numbers(qty)
    }
}
