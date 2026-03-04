//! Strategy context construction from cached engine state.
//!
//! The engine maintains a cache of completed candles and partial
//! candles from the aggregator. Before each strategy callback, the
//! cache is refreshed (if stale) and a [`StrategyContext`] is built
//! by borrowing from the cached data and current engine state.

use std::collections::BTreeMap;

use crate::engine::kernel::Engine;
use crate::output::snapshot::TradeSnapshot;
use crate::output::trade_record::TradeRecord;
use crate::strategy::Strategy;
use crate::strategy::context::StrategyContext;
use kairos_data::{FuturesTicker, Trade};

impl Engine {
    /// Refreshes the candle and partial-candle caches from the
    /// aggregator.
    ///
    /// Uses a generation counter to skip redundant rebuilds — if
    /// the aggregator has not produced new candles since the last
    /// rebuild, this is a no-op. Must be called before
    /// [`build_context`](Self::build_context).
    pub(crate) fn rebuild_context_cache(&mut self, primary: FuturesTicker) {
        let current_gen = self.aggregator.generation();
        if current_gen == self.cache_generation && !self.cached_candles.is_empty() {
            return;
        }
        self.cache_generation = current_gen;

        self.cached_candles.clear();
        self.cached_partials.clear();

        // Primary instrument + primary timeframe
        let candles = self.aggregator.candles(primary, self.config.timeframe);
        self.cached_candles
            .insert((primary, self.config.timeframe), candles.to_vec());

        // Additional timeframes for the primary instrument
        for &tf in &self.config.additional_timeframes {
            let c = self.aggregator.candles(primary, tf);
            self.cached_candles.insert((primary, tf), c.to_vec());
        }

        // Additional instruments at the primary timeframe
        for &inst in &self.config.additional_instruments {
            let c = self.aggregator.candles(inst, self.config.timeframe);
            self.cached_candles
                .insert((inst, self.config.timeframe), c.to_vec());
        }

        // Partial (in-progress) candle for the primary pair
        if let Some(p) = self
            .aggregator
            .partial_candle(primary, self.config.timeframe)
        {
            self.cached_partials
                .insert((primary, self.config.timeframe), p.clone());
        }
    }

    /// Builds a [`StrategyContext`] from the engine's cached state.
    ///
    /// The context borrows from cached candles, the portfolio, the
    /// order book, and session state — providing the strategy with
    /// a read-only view of the simulation. Call
    /// [`rebuild_context_cache`](Self::rebuild_context_cache) before
    /// this method.
    pub(crate) fn build_context<'a>(
        &'a self,
        primary: FuturesTicker,
        trade: &'a Trade,
    ) -> StrategyContext<'a> {
        let active_orders: Vec<_> = self.order_book.active_orders().collect();

        let hhmm = self.session_clock.local_hhmm(trade.time);
        let session_state = self.session_clock.session_state;

        StrategyContext {
            trade,
            candles: &self.cached_candles,
            partial_candles: &self.cached_partials,
            depth: &self.latest_depth,
            studies: &self.study_bank,
            positions: self.portfolio.positions(),
            active_orders,
            equity: self.portfolio.total_equity(),
            cash: self.portfolio.cash(),
            buying_power: self.portfolio.buying_power(),
            drawdown_pct: self.portfolio.current_drawdown_pct(),
            realized_pnl: self.portfolio.realized_pnl(),
            timestamp: trade.time,
            local_hhmm: hhmm,
            session_state,
            session_tick_count: self.session_clock.session_trade_count,
            is_warmup: self.is_warmup,
            instruments: &self.instruments,
            primary_instrument: primary,
        }
    }

    /// Builds a [`TradeSnapshot`] for a completed trade record.
    ///
    /// Captures ALL session candles from the cache (full session) and
    /// records the entry/exit candle indices into the full array.
    pub(crate) fn build_trade_snapshot(
        &self,
        record: &TradeRecord,
        trade: &Trade,
        strategy: &dyn Strategy,
    ) -> TradeSnapshot {
        let primary = self.config.ticker;
        let tf = self.config.timeframe;
        let candles = self
            .cached_candles
            .get(&(primary, tf))
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        // Binary search for entry and exit candle indices
        let entry_ts = record.entry_time.0;
        let exit_ts = record.exit_time.0;

        let entry_idx = candles
            .binary_search_by(|c| c.time.0.cmp(&entry_ts))
            .unwrap_or_else(|i| i.saturating_sub(1));
        let exit_idx = candles
            .binary_search_by(|c| c.time.0.cmp(&exit_ts))
            .unwrap_or_else(|i| i.min(candles.len().saturating_sub(1)));

        // Full session candles — no windowing
        let window = candles.to_vec();

        let entry_in_window = if !window.is_empty() {
            Some(entry_idx.min(window.len() - 1))
        } else {
            None
        };
        let exit_in_window = if !window.is_empty() {
            Some(exit_idx.min(window.len() - 1))
        } else {
            None
        };

        // Collect strategy context
        self.rebuild_context_cache_for_snapshot(primary);
        let ctx = self.build_context(primary, trade);
        let context: BTreeMap<String, _> = strategy.trade_context(&ctx).into_iter().collect();

        TradeSnapshot {
            candles: window,
            entry_candle_idx: entry_in_window,
            exit_candle_idx: exit_in_window,
            context,
        }
    }

    /// Ensures context cache is fresh for snapshot building without
    /// requiring `&mut self`.
    ///
    /// Since `build_trade_snapshot` takes `&self`, we can't call
    /// `rebuild_context_cache` (which takes `&mut self`). The caller
    /// is responsible for ensuring the cache is up-to-date before
    /// calling `build_trade_snapshot`.
    fn rebuild_context_cache_for_snapshot(&self, _primary: FuturesTicker) {
        // No-op: caller must ensure cache is rebuilt before calling
        // build_trade_snapshot. This exists as a documentation
        // marker.
    }
}
