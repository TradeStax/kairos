use kairos_data::{Candle, Price, Side, Timestamp, Trade, Volume};

/// Partial candle being built.
#[derive(Debug, Clone)]
pub struct PartialCandle {
    pub bucket_start: u64,
    pub open: Price,
    pub high: Price,
    pub low: Price,
    pub close: Price,
    pub buy_volume: f64,
    pub sell_volume: f64,
}

/// Aggregates trade ticks into candles at a fixed timeframe.
pub struct CandleAggregator {
    timeframe_ms: u64,
    partial: Option<PartialCandle>,
}

impl CandleAggregator {
    pub fn new(timeframe_ms: u64) -> Self {
        Self {
            timeframe_ms,
            partial: None,
        }
    }

    /// Feed a trade. Returns a closed candle if the bucket
    /// boundary was crossed.
    pub fn update(&mut self, trade: &Trade) -> Option<Candle> {
        let bucket = (trade.time.0 / self.timeframe_ms) * self.timeframe_ms;

        match &mut self.partial {
            None => {
                self.start_new_bar(trade, bucket);
                None
            }
            Some(bar) if bar.bucket_start == bucket => {
                Self::update_bar(bar, trade);
                None
            }
            Some(_) => {
                let closed = self.close_bar();
                self.start_new_bar(trade, bucket);
                closed
            }
        }
    }

    /// Flush the current partial candle (e.g. at end of data).
    pub fn flush(&mut self) -> Option<Candle> {
        self.close_bar()
    }

    /// Get a view of the partial candle in progress.
    pub fn partial(&self) -> Option<&PartialCandle> {
        self.partial.as_ref()
    }

    fn start_new_bar(&mut self, trade: &Trade, bucket: u64) {
        let (buy_vol, sell_vol) = match trade.side {
            Side::Buy | Side::Bid => (trade.quantity.0, 0.0),
            Side::Sell | Side::Ask => (0.0, trade.quantity.0),
        };
        self.partial = Some(PartialCandle {
            bucket_start: bucket,
            open: trade.price,
            high: trade.price,
            low: trade.price,
            close: trade.price,
            buy_volume: buy_vol,
            sell_volume: sell_vol,
        });
    }

    fn update_bar(bar: &mut PartialCandle, trade: &Trade) {
        if trade.price > bar.high {
            bar.high = trade.price;
        }
        if trade.price < bar.low {
            bar.low = trade.price;
        }
        bar.close = trade.price;
        match trade.side {
            Side::Buy | Side::Bid => {
                bar.buy_volume += trade.quantity.0;
            }
            Side::Sell | Side::Ask => {
                bar.sell_volume += trade.quantity.0;
            }
        }
    }

    fn close_bar(&mut self) -> Option<Candle> {
        self.partial.take().and_then(|bar| {
            Candle::new(
                Timestamp(bar.bucket_start),
                bar.open,
                bar.high,
                bar.low,
                bar.close,
                Volume(bar.buy_volume),
                Volume(bar.sell_volume),
            )
            .ok()
        })
    }
}
