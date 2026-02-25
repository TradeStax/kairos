use kairos_data::{Candle, Price, Side, Timestamp, Trade};

/// Full context passed to strategy callbacks on every event.
#[derive(Debug)]
pub struct StrategyInput<'a> {
    /// The trade that triggered this callback.
    pub trade: &'a Trade,
    /// All completed (closed) candles in chronological order.
    /// Does not include the candle currently forming.
    pub candles: &'a [Candle],
    /// The candle currently being built (not yet closed).
    pub candle_in_progress: Option<PartialCandleView>,
    /// Instrument tick size.
    pub tick_size: Price,
    /// Contract multiplier (e.g. 50 for ES).
    pub contract_size: f32,
    /// Current RTH session state.
    pub session_state: SessionState,
    /// Local hour (0-23), computed from UTC + timezone_offset_hours.
    pub local_hour: u32,
    /// Local minute (0-59).
    pub local_minute: u32,
    /// Convenience: local_hour * 100 + local_minute (e.g. 930, 1600).
    pub local_hhmm: u32,
    /// Number of trades processed in the current RTH session (resets to 0 at SessionOpen).
    pub session_trade_count: u32,
    /// Current unrealized PnL in USD for the open position, if any.
    pub unrealized_pnl_usd: Option<f64>,
    /// View of the currently open position, if any.
    pub open_position: Option<OpenPositionView>,
}

/// A snapshot view of the candle currently being formed.
#[derive(Debug, Clone, Copy)]
pub struct PartialCandleView {
    pub open: Price,
    pub high: Price,
    pub low: Price,
    pub close: Price,
    pub buy_volume: f64,
    pub sell_volume: f64,
    /// Bucket start time (ms since epoch).
    pub bucket_start_ms: u64,
}

/// A snapshot view of the currently open position.
#[derive(Debug, Clone, Copy)]
pub struct OpenPositionView {
    pub side: Side,
    pub entry_price: Price,
    pub entry_time: Timestamp,
    pub quantity: f64,
    pub stop_loss: Option<Price>,
    pub take_profit: Option<Price>,
    /// Current MAE (Max Adverse Excursion) price.
    pub mae: Price,
    /// Current MFE (Max Favorable Excursion) price.
    pub mfe: Price,
}

/// Current state of the RTH session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    PreMarket,
    Open,
    Closed,
}
