use exchange::util::{Price as ExPrice, PriceStep};

use std::collections::BTreeMap;
use std::time::Duration;

use data::Side;

pub const TEXT_SIZE: f32 = crate::style::tokens::text::SMALL;
pub const ROW_HEIGHT: f32 = crate::style::tokens::layout::PANEL_ROW_HEIGHT;

// Total width ratios must sum to 1.0
/// Uses half of the width for each side of the order quantity columns
pub const ORDER_QTY_COLS_WIDTH: f32 = 0.60;
/// Uses half of the width for each side of the trade quantity columns
pub const TRADE_QTY_COLS_WIDTH: f32 = 0.20;

pub const COL_PADDING: f32 = crate::style::tokens::spacing::XS;
/// Used for calculating layout with texts inside the price column
pub const MONO_CHAR_ADVANCE: f32 = 0.62;
/// Minimum padding on each side of the price text inside the price column
pub const PRICE_TEXT_SIDE_PAD_MIN: f32 = 12.0;

pub const CHASE_CIRCLE_RADIUS: f32 = 4.0;
/// Maximum interval between chase updates to consider them part of the same chase
pub const CHASE_MIN_INTERVAL: Duration = Duration::from_millis(200);

pub enum DomRow {
    Ask { price: ExPrice, qty: f32 },
    Spread,
    CenterDivider,
    Bid { price: ExPrice, qty: f32 },
}

#[derive(Default)]
pub struct Maxima {
    pub vis_max_order_qty: f32,
    pub vis_max_trade_qty: f32,
}

pub struct VisibleRow {
    pub row: DomRow,
    pub y: f32,
    pub buy_t: f32,
    pub sell_t: f32,
}

pub struct ColumnRanges {
    pub bid_order: (f32, f32),
    pub sell: (f32, f32),
    pub price: (f32, f32),
    pub buy: (f32, f32),
    pub ask_order: (f32, f32),
}

pub struct PriceLayout {
    pub price_px: f32,
    pub inside_pad_px: f32,
}

pub struct PriceGrid {
    pub best_bid: ExPrice,
    pub best_ask: ExPrice,
    pub tick: PriceStep,
}

impl PriceGrid {
    /// Returns None for index 0 (spread row)
    pub fn index_to_price(&self, idx: i32) -> Option<ExPrice> {
        if idx == 0 {
            return None;
        }
        if idx > 0 {
            let off = (idx - 1) as i64; // 1 => best_bid, 2 => best_bid - 1 tick
            Some(self.best_bid.add_steps(-off, self.tick.into()))
        } else {
            let off = (-1 - idx) as i64; // -1 => best_ask, -2 => best_ask + 1 tick
            Some(self.best_ask.add_steps(off, self.tick.into()))
        }
    }

    pub fn top_y(idx: i32) -> f32 {
        (idx as f32) * ROW_HEIGHT - ROW_HEIGHT * 0.5
    }
}

// ── Internal Data Structures (using exchange::util types) ─────────────

/// Grouped depth for one side of orderbook
#[derive(Debug, Clone)]
pub struct GroupedDepth {
    pub orders: BTreeMap<ExPrice, f32>,
    pub chase: ChaseTracker,
}

impl GroupedDepth {
    pub fn new() -> Self {
        Self {
            orders: BTreeMap::new(),
            chase: ChaseTracker::new(),
        }
    }

    pub fn regroup_from_btree(
        &mut self,
        raw: &BTreeMap<i64, f32>,
        side: Side,
        tick_step: PriceStep,
    ) {
        self.orders.clear();

        for (price_units, qty) in raw.iter() {
            let price = ExPrice::from_units(*price_units);
            let grouped_price =
                price.round_to_side_step(side == Side::Bid, tick_step.into());
            *self.orders.entry(grouped_price).or_insert(0.0) += *qty;
        }
    }

    pub fn best_price(&self, side: Side) -> Option<ExPrice> {
        match side {
            Side::Bid => self.orders.last_key_value().map(|(p, _)| *p),
            Side::Ask => self.orders.first_key_value().map(|(p, _)| *p),
            _ => None,
        }
    }
}

/// Trade store for ladder (grouped by price)
#[derive(Debug, Clone)]
pub struct TradeStore {
    trades: Vec<(u64, ExPrice, f32, bool)>, // (time, price, qty, is_sell)
    grouped_buy: BTreeMap<ExPrice, f32>,
    grouped_sell: BTreeMap<ExPrice, f32>,
}

impl TradeStore {
    pub fn new() -> Self {
        Self {
            trades: Vec::new(),
            grouped_buy: BTreeMap::new(),
            grouped_sell: BTreeMap::new(),
        }
    }

    pub fn insert_trade(
        &mut self,
        time: u64,
        price: ExPrice,
        qty: f32,
        is_sell: bool,
        step: PriceStep,
    ) {
        self.trades.push((time, price, qty, is_sell));

        let grouped_price = price.round_to_step(step.into());
        if is_sell {
            *self.grouped_sell.entry(grouped_price).or_insert(0.0) += qty;
        } else {
            *self.grouped_buy.entry(grouped_price).or_insert(0.0) += qty;
        }
    }

    pub fn trade_qty_at(&self, price: ExPrice) -> (f32, f32) {
        let buy = self.grouped_buy.get(&price).copied().unwrap_or(0.0);
        let sell = self.grouped_sell.get(&price).copied().unwrap_or(0.0);
        (buy, sell)
    }

    pub fn price_range(&self) -> Option<(ExPrice, ExPrice)> {
        if self.trades.is_empty() {
            return None;
        }
        let mut min = self.trades[0].1;
        let mut max = self.trades[0].1;
        for &(_, p, _, _) in &self.trades {
            if p < min {
                min = p;
            }
            if p > max {
                max = p;
            }
        }
        Some((min, max))
    }

    pub fn rebuild_grouped(&mut self, step: PriceStep) {
        self.grouped_buy.clear();
        self.grouped_sell.clear();

        for &(_, price, qty, is_sell) in &self.trades {
            let grouped_price = price.round_to_step(step.into());
            if is_sell {
                *self.grouped_sell.entry(grouped_price).or_insert(0.0) += qty;
            } else {
                *self.grouped_buy.entry(grouped_price).or_insert(0.0) += qty;
            }
        }
    }

    pub fn maybe_cleanup(
        &mut self,
        now_ms: u64,
        retention_ms: u64,
        step: PriceStep,
    ) -> bool {
        let cutoff = now_ms.saturating_sub(retention_ms);
        let before_len = self.trades.len();

        self.trades.retain(|&(time, _, _, _)| time >= cutoff);

        if self.trades.len() != before_len {
            self.rebuild_grouped(step);
            true
        } else {
            false
        }
    }

    pub fn is_empty(&self) -> bool {
        self.trades.is_empty()
    }
}

/// Chase tracker (uses i64 price units internally)
#[derive(Debug, Clone, Copy, Default)]
pub enum ChaseState {
    #[default]
    Idle,
    Chasing {
        start_units: i64,
        end_units: i64,
        consecutive: u32,
    },
    Fading {
        start_units: i64,
        end_units: i64,
        start_consecutive: u32,
        fade_steps: u32,
    },
}

#[derive(Debug, Clone, Default)]
pub struct ChaseTracker {
    pub last_best: Option<i64>,
    pub state: ChaseState,
    pub last_update_ms: Option<u64>,
}

impl ChaseTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(
        &mut self,
        current_best: Option<i64>,
        is_bid: bool,
        now_ms: u64,
        max_interval_ms: u64,
    ) {
        if let Some(prev) = self.last_update_ms
            && max_interval_ms > 0
            && now_ms.saturating_sub(prev) > max_interval_ms
        {
            self.reset();
        }

        self.last_update_ms = Some(now_ms);

        let Some(current) = current_best else {
            self.reset();
            return;
        };

        if let Some(last) = self.last_best {
            let is_continue = if is_bid {
                current > last
            } else {
                current < last
            };
            let is_reverse = if is_bid {
                current < last
            } else {
                current > last
            };
            let is_unchanged = current == last;

            self.state = match (&self.state, is_continue, is_reverse, is_unchanged) {
                (
                    ChaseState::Chasing {
                        start_units,
                        consecutive,
                        ..
                    },
                    true,
                    _,
                    _,
                ) => ChaseState::Chasing {
                    start_units: *start_units,
                    end_units: current,
                    consecutive: consecutive.saturating_add(1),
                },
                (ChaseState::Idle, true, _, _)
                | (ChaseState::Fading { .. }, true, _, _) => ChaseState::Chasing {
                    start_units: last,
                    end_units: current,
                    consecutive: 1,
                },
                (
                    ChaseState::Chasing {
                        start_units,
                        end_units,
                        consecutive,
                    },
                    _,
                    true,
                    _,
                ) if *consecutive > 0 => ChaseState::Fading {
                    start_units: *start_units,
                    end_units: *end_units,
                    start_consecutive: *consecutive,
                    fade_steps: 0,
                },
                (
                    ChaseState::Chasing {
                        start_units,
                        end_units,
                        consecutive,
                    },
                    _,
                    _,
                    true,
                ) if *consecutive > 0 => ChaseState::Fading {
                    start_units: *start_units,
                    end_units: *end_units,
                    start_consecutive: *consecutive,
                    fade_steps: 0,
                },
                (
                    ChaseState::Fading {
                        start_units,
                        end_units,
                        start_consecutive,
                        fade_steps,
                    },
                    _,
                    _,
                    _,
                ) => ChaseState::Fading {
                    start_units: *start_units,
                    end_units: *end_units,
                    start_consecutive: *start_consecutive,
                    fade_steps: fade_steps.saturating_add(1),
                },
                _ => self.state,
            };

            if let ChaseState::Fading {
                start_consecutive,
                fade_steps,
                ..
            } = self.state
            {
                let alpha = Self::calculate_alpha(start_consecutive, fade_steps);
                if alpha < 0.15 {
                    self.state = ChaseState::Idle;
                }
            }
        }

        self.last_best = Some(current);
    }

    pub fn reset(&mut self) {
        self.last_best = None;
        self.state = ChaseState::Idle;
        self.last_update_ms = None;
    }

    pub fn segment(&self) -> Option<(i64, i64, f32)> {
        match self.state {
            ChaseState::Chasing {
                start_units,
                end_units,
                consecutive,
            } => {
                let alpha = Self::consecutive_to_alpha(consecutive);
                Some((start_units, end_units, alpha))
            }
            ChaseState::Fading {
                start_units,
                end_units,
                start_consecutive,
                fade_steps,
            } => {
                let alpha = Self::calculate_alpha(start_consecutive, fade_steps);
                Some((start_units, end_units, alpha))
            }
            ChaseState::Idle => None,
        }
    }

    fn calculate_alpha(start_consecutive: u32, fade_steps: u32) -> f32 {
        let base = Self::consecutive_to_alpha(start_consecutive);
        base / (1.0 + fade_steps as f32)
    }

    fn consecutive_to_alpha(n: u32) -> f32 {
        let nf = n as f32;
        1.0 - 1.0 / (1.0 + nf)
    }
}
