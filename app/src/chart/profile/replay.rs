use super::ProfileChart;
use data::{Candle, Side, Timestamp, Trade, Volume};
use study::Study as _;

impl ProfileChart {
    /// Rebuild the chart from scratch with the given trades.
    pub fn rebuild_from_trades(&mut self, trades: &[Trade]) {
        self.chart_data.trades.clear();
        self.chart_data.candles.clear();

        self.profile_study.reset();
        for s in &mut self.studies {
            s.reset();
        }

        for trade in trades {
            self.append_trade(trade);
        }

        self.fingerprint = (0, 0, 0, 0); // force recompute
        self.recompute_profile();
        self.studies_dirty = true;
        self.invalidate();
    }

    /// Append a single trade during replay or live streaming.
    pub fn append_trade(&mut self, trade: &Trade) {
        self.chart_data.trades.push(*trade);

        let (buy_vol, sell_vol) = match trade.side {
            Side::Buy | Side::Bid => (Volume(trade.quantity.0), Volume(0.0)),
            Side::Sell | Side::Ask => (Volume(0.0), Volume(trade.quantity.0)),
        };

        match self.basis {
            data::ChartBasis::Time(tf) => {
                let interval = tf.to_milliseconds();
                if interval == 0 {
                    return;
                }
                let bucket_time = (trade.time.to_millis() / interval) * interval;

                if let Some(last) = self.chart_data.candles.last_mut()
                    && last.time.0 == bucket_time
                {
                    last.high = last.high.max(trade.price);
                    last.low = last.low.min(trade.price);
                    last.close = trade.price;
                    last.buy_volume = Volume(last.buy_volume.0 + buy_vol.0);
                    last.sell_volume = Volume(last.sell_volume.0 + sell_vol.0);
                } else {
                    self.chart_data.candles.push(Candle {
                        time: Timestamp::from_millis(bucket_time),
                        open: trade.price,
                        high: trade.price,
                        low: trade.price,
                        close: trade.price,
                        buy_volume: buy_vol,
                        sell_volume: sell_vol,
                    });
                }
            }
            data::ChartBasis::Tick(_) => {} // Profile doesn't use tick basis
        }

        // Recompute profile and invalidate caches so the chart redraws.
        // recompute_profile() has fingerprint-based dedup and won't redo
        // work if nothing meaningful changed.
        self.recompute_profile();
        self.invalidate();
    }
}
