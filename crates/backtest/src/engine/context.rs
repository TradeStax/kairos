use crate::engine::kernel::Engine;
use crate::strategy::context::StrategyContext;
use kairos_data::{FuturesTicker, Trade};

impl Engine {
    /// Populate cached candles and partial candles from the
    /// aggregator. Must be called before `build_context`.
    pub(crate) fn rebuild_context_cache(&mut self, primary: FuturesTicker) {
        let current_gen = self.aggregator.generation();
        if current_gen == self.cache_generation && !self.cached_candles.is_empty() {
            return;
        }
        self.cache_generation = current_gen;

        self.cached_candles.clear();
        self.cached_partials.clear();

        // Primary timeframe
        let candles = self.aggregator.candles(primary, self.config.timeframe);
        self.cached_candles
            .insert((primary, self.config.timeframe), candles.to_vec());

        // Additional timeframes
        for tf in self.config.additional_timeframes.clone() {
            let c = self.aggregator.candles(primary, tf);
            self.cached_candles.insert((primary, tf), c.to_vec());
        }

        // Additional instruments
        for inst in self.config.additional_instruments.clone() {
            let c = self.aggregator.candles(inst, self.config.timeframe);
            self.cached_candles
                .insert((inst, self.config.timeframe), c.to_vec());
        }

        // Partial candles
        if let Some(p) = self
            .aggregator
            .partial_candle(primary, self.config.timeframe)
        {
            self.cached_partials
                .insert((primary, self.config.timeframe), p.clone());
        }
    }

    /// Build a `StrategyContext` from cached state.
    /// `rebuild_context_cache` must be called before this method.
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
}
