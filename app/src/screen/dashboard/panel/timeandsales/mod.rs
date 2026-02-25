mod render;

use data::Trade;
pub use data::config::panel::timeandsales::Config;
use data::config::panel::timeandsales::{HistAgg, StackedBar, TradeEntry};
use exchange::TickerInfo;

use iced::widget::canvas;
use std::collections::VecDeque;
use std::time::Instant;

pub(super) const TRADE_ROW_HEIGHT: f32 =
    crate::style::tokens::layout::PANEL_ROW_HEIGHT_SM;
pub(super) const METRICS_HEIGHT_COMPACT: f32 =
    crate::style::tokens::spacing::MD;
pub(super) const METRICS_HEIGHT_FULL: f32 = 18.0;

impl super::Panel for TimeAndSales {
    fn scroll(&mut self, delta: f32) {
        self.scroll_offset -= delta;

        let stacked_bar_h = self.stacked_bar_height();
        let total_content_height =
            (self.recent_trades.len() as f32 * TRADE_ROW_HEIGHT) + stacked_bar_h;
        let max_scroll_offset =
            (total_content_height - TRADE_ROW_HEIGHT).max(0.0);

        self.scroll_offset = self.scroll_offset.clamp(0.0, max_scroll_offset);

        if self.scroll_offset > stacked_bar_h + TRADE_ROW_HEIGHT {
            self.is_paused = true;
        } else if self.is_paused {
            self.is_paused = false;

            for trade in self.paused_trades_buffer.iter() {
                self.hist_agg.add(&trade.display);
            }

            self.recent_trades
                .extend(self.paused_trades_buffer.drain(..));

            self.prune_by_time(None);
        }

        self.invalidate(Some(Instant::now()));
    }

    fn reset_scroll(&mut self) {
        self.scroll_offset = 0.0;
        self.is_paused = false;

        for trade in self.paused_trades_buffer.iter() {
            self.hist_agg.add(&trade.display);
        }

        self.recent_trades
            .extend(self.paused_trades_buffer.drain(..));

        self.prune_by_time(None);

        self.invalidate(Some(Instant::now()));
    }

    fn invalidate(&mut self, now: Option<Instant>) -> Option<super::Action> {
        self.invalidate(now)
    }

    fn is_empty(&self) -> bool {
        self.recent_trades.is_empty() && self.paused_trades_buffer.is_empty()
    }
}

pub struct TimeAndSales {
    pub(super) recent_trades: VecDeque<TradeEntry>,
    pub(super) paused_trades_buffer: VecDeque<TradeEntry>,
    pub(super) hist_agg: HistAgg,
    pub(super) is_paused: bool,
    pub(super) max_filtered_qty: f32,
    pub(super) ticker_info: TickerInfo,
    pub config: Config,
    pub(super) cache: canvas::Cache,
    pub(super) last_tick: Instant,
    pub(super) scroll_offset: f32,
}

impl TimeAndSales {
    pub fn new(config: Option<Config>, ticker_info: TickerInfo) -> Self {
        Self {
            recent_trades: VecDeque::new(),
            paused_trades_buffer: VecDeque::new(),
            hist_agg: HistAgg::default(),
            is_paused: false,
            config: config.unwrap_or_default(),
            max_filtered_qty: 0.0,
            ticker_info,
            cache: canvas::Cache::default(),
            last_tick: Instant::now(),
            scroll_offset: 0.0,
        }
    }

    /// Update from replay data - converts domain Trades to TradeEntries
    pub fn update_from_replay(&mut self, trades: &[Trade]) {
        let size_filter = self.config.trade_size_filter;

        let target_trades = if self.is_paused {
            &mut self.paused_trades_buffer
        } else {
            &mut self.recent_trades
        };

        for trade in trades {
            let trade_entry = convert_trade_to_entry(trade);

            if trade_entry.quantity >= size_filter {
                self.max_filtered_qty =
                    self.max_filtered_qty.max(trade_entry.quantity);
            }

            target_trades.push_back(trade_entry.clone());

            if !self.is_paused {
                self.hist_agg.add(&trade_entry.display);
            }
        }

        if !self.is_paused {
            self.prune_by_time(None);
        }
        self.prune_paused_by_time(None);
    }

    pub fn last_update(&self) -> Instant {
        self.last_tick
    }

    pub fn invalidate(
        &mut self,
        now: Option<Instant>,
    ) -> Option<super::Action> {
        if !self.is_paused {
            self.prune_by_time(None);
        }
        self.prune_paused_by_time(None);

        self.cache.clear();
        if let Some(now) = now {
            self.last_tick = now;
        }
        None
    }

    pub(super) fn stacked_bar_height(&self) -> f32 {
        match &self.config.stacked_bar {
            Some(StackedBar::Compact(_)) => METRICS_HEIGHT_COMPACT,
            Some(StackedBar::Full(_)) => METRICS_HEIGHT_FULL,
            None => 0.0,
        }
    }

    pub(super) fn pause_overlay_height(&self) -> f32 {
        self.stacked_bar_height().max(METRICS_HEIGHT_COMPACT)
            + TRADE_ROW_HEIGHT
    }

    fn prune_by_time(&mut self, now_epoch_ms: Option<u64>) {
        if self.recent_trades.is_empty() {
            return;
        }

        let now_ms = now_epoch_ms.unwrap_or_else(|| {
            let ts = chrono::Utc::now().timestamp_millis();
            if ts < 0 { 0 } else { ts as u64 }
        });

        let trade_retention_ms =
            self.config.trade_retention.as_millis() as u64;
        let prune_slack_ms = trade_retention_ms / 10;

        let low_cutoff = now_ms.saturating_sub(trade_retention_ms);
        let high_cutoff = now_ms
            .saturating_sub(trade_retention_ms.saturating_add(prune_slack_ms));

        if let Some(oldest) = self.recent_trades.front() {
            if oldest.ts_ms >= high_cutoff {
                return;
            }
        } else {
            return;
        }

        let size_filter = self.config.trade_size_filter;

        let mut popped_any = false;
        while let Some(front) = self.recent_trades.front() {
            if front.ts_ms >= low_cutoff {
                break;
            }
            let old = self.recent_trades.pop_front().unwrap();
            self.hist_agg.remove(&old.display);
            popped_any = true;
        }

        if popped_any {
            self.max_filtered_qty = self
                .recent_trades
                .iter()
                .filter(|t| t.quantity >= size_filter)
                .map(|e| e.quantity)
                .fold(0.0, f32::max);

            let stacked_bar_h = self.stacked_bar_height();
            let total_content_height =
                (self.recent_trades.len() as f32 * TRADE_ROW_HEIGHT)
                    + stacked_bar_h;
            let max_scroll_offset =
                (total_content_height - TRADE_ROW_HEIGHT).max(0.0);
            self.scroll_offset =
                self.scroll_offset.clamp(0.0, max_scroll_offset);
        }
    }

    fn prune_paused_by_time(&mut self, now_epoch_ms: Option<u64>) {
        if self.paused_trades_buffer.is_empty() {
            return;
        }

        let trade_retention_ms =
            self.config.trade_retention.as_millis() as u64;
        let prune_slack_ms = trade_retention_ms / 10;

        let now_ms = now_epoch_ms.unwrap_or_else(|| {
            let ts = chrono::Utc::now().timestamp_millis();
            if ts < 0 { 0 } else { ts as u64 }
        });

        let low_cutoff = now_ms.saturating_sub(trade_retention_ms);
        let high_cutoff = now_ms
            .saturating_sub(trade_retention_ms.saturating_add(prune_slack_ms));

        if let Some(oldest) = self.paused_trades_buffer.front() {
            if oldest.ts_ms >= high_cutoff {
                return;
            }
        } else {
            return;
        }

        while let Some(front) = self.paused_trades_buffer.front() {
            if front.ts_ms >= low_cutoff {
                break;
            }
            self.paused_trades_buffer.pop_front();
        }
    }
}

/// Helper function to convert domain Trade to display TradeEntry
fn convert_trade_to_entry(trade: &Trade) -> TradeEntry {
    TradeEntry::new(
        trade.time,
        trade.price,
        trade.quantity.0 as f32,
        trade.side.is_sell(),
    )
}
