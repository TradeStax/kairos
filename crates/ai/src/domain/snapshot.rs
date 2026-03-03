//! Chart snapshot types for AI tool access.
//!
//! An immutable snapshot of chart data captured at request time so
//! the async streaming function needs no mutable access to pane state.

use data::domain::market::entities::{Candle, Trade};

/// Snapshot of a single study's output for AI tool access.
#[derive(Debug, Clone)]
pub struct StudyOutputSnapshot {
    /// Study instance identifier
    pub study_id: String,
    /// Human-readable study name
    pub study_name: String,
    /// Line series: `(label, Vec<(time_secs, value)>)`
    pub line_values: Vec<(String, Vec<(u64, f32)>)>,
    /// Bar series: `(label, Vec<(time_secs, value)>)`
    pub bar_values: Vec<(String, Vec<(u64, f32)>)>,
    /// Horizontal levels: `(label, price)`
    pub levels: Vec<(String, f64)>,
}

/// Per-price-level trade data within a footprint candle snapshot.
#[derive(Debug, Clone)]
pub struct FootprintLevelSnapshot {
    /// Price level (display units)
    pub price: f64,
    /// Buy-side volume at this level
    pub buy_volume: f32,
    /// Sell-side volume at this level
    pub sell_volume: f32,
}

/// Per-candle footprint data snapshot for AI tool access.
#[derive(Debug, Clone)]
pub struct FootprintCandleSnapshot {
    /// Candle timestamp in seconds since epoch
    pub time_secs: u64,
    /// Open price
    pub open: f64,
    /// High price
    pub high: f64,
    /// Low price
    pub low: f64,
    /// Close price
    pub close: f64,
    /// Point-of-control price, if computed
    pub poc_price: Option<f64>,
    /// Per-level footprint data
    pub levels: Vec<FootprintLevelSnapshot>,
}

/// A single level within a volume profile snapshot.
#[derive(Debug, Clone)]
pub struct ProfileLevelSnapshot {
    /// Price level (display units)
    pub price: f64,
    /// Buy-side volume
    pub buy_volume: f32,
    /// Sell-side volume
    pub sell_volume: f32,
}

/// Volume profile snapshot for AI tool access.
#[derive(Debug, Clone)]
pub struct ProfileSnapshot {
    /// Per-level profile data
    pub levels: Vec<ProfileLevelSnapshot>,
    /// Point-of-control price
    pub poc_price: Option<f64>,
    /// Value area high price
    pub value_area_high: Option<f64>,
    /// Value area low price
    pub value_area_low: Option<f64>,
    /// Total volume across the profile
    pub total_volume: f64,
    /// High volume node prices
    pub hvn_prices: Vec<f64>,
    /// Low volume node prices
    pub lvn_prices: Vec<f64>,
    /// Time range `(start_secs, end_secs)` of the profile
    pub time_range: Option<(u64, u64)>,
}

/// A single anchor point in a drawing snapshot.
#[derive(Debug, Clone)]
pub struct DrawingPointSnapshot {
    /// Price coordinate (display units)
    pub price: f64,
    /// Time coordinate in seconds since epoch
    pub time_secs: u64,
}

/// Snapshot of a chart drawing for AI tool access.
#[derive(Debug, Clone)]
pub struct DrawingSnapshot {
    /// Drawing identifier
    pub id: String,
    /// Drawing tool type name
    pub tool_type: String,
    /// Anchor points
    pub points: Vec<DrawingPointSnapshot>,
    /// Optional text label
    pub label: Option<String>,
    /// Whether the drawing is visible
    pub visible: bool,
    /// Whether the drawing is locked from editing
    pub locked: bool,
}

/// Snapshot of a big trade marker for AI tool access.
///
/// `time` is the marker's raw X coordinate: millisecond timestamp for
/// time-based charts, or reverse candle index for tick-based charts.
#[derive(Debug, Clone)]
pub struct BigTradeSnapshot {
    /// Raw X coordinate (see type docs for meaning)
    pub time: u64,
    /// Trade price (display units)
    pub price: f64,
    /// Trade quantity
    pub quantity: f64,
    /// `true` if the aggressor was a buyer
    pub is_buy: bool,
}

/// Immutable snapshot of chart data captured at request time so the
/// async streaming function needs no mutable access to pane state.
#[derive(Debug, Clone)]
pub struct ChartSnapshot {
    /// Ticker symbol
    pub ticker: String,
    /// Tick size for price formatting
    pub tick_size: f32,
    /// Contract multiplier
    pub contract_size: f32,
    /// Timeframe label
    pub timeframe: String,
    /// Chart type label
    pub chart_type: String,
    /// Whether the chart is receiving live data
    pub is_live: bool,
    /// Candle data
    pub candles: Vec<Candle>,
    /// Raw trade data
    pub trades: Vec<Trade>,
    /// Whether the trade vec was truncated for size
    pub trades_truncated: bool,
    /// Active study names
    pub active_studies: Vec<String>,
    /// Date range as `(start_display, end_display)`
    pub date_range_display: Option<(String, String)>,
    /// Study output snapshots
    pub study_snapshots: Vec<StudyOutputSnapshot>,
    /// Big trade markers
    pub big_trade_markers: Vec<BigTradeSnapshot>,
    /// Timezone label
    pub timezone: String,
    /// Footprint candle data
    pub footprint_candles: Vec<FootprintCandleSnapshot>,
    /// Volume profile snapshots
    pub profile_snapshots: Vec<ProfileSnapshot>,
    /// Drawing snapshots
    pub drawing_snapshots: Vec<DrawingSnapshot>,
    /// Visible price range high
    pub visible_price_high: Option<f64>,
    /// Visible price range low
    pub visible_price_low: Option<f64>,
    /// Visible time range start (seconds since epoch)
    pub visible_time_start: Option<u64>,
    /// Visible time range end (seconds since epoch)
    pub visible_time_end: Option<u64>,
    /// Whether the chart uses tick-based aggregation
    pub is_tick_basis: bool,
}
