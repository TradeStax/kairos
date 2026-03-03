//! Big Trades study — institutional-scale execution detection.
//!
//! Reconstructs large executions by aggregating consecutive same-side fills
//! within a configurable time window. Each aggregated block produces a
//! marker at the VWAP-weighted price, sized proportionally to the total
//! contract count.
//!
//! The detection pipeline:
//! 1. Iterate raw trades in chronological order
//! 2. Merge consecutive same-side fills separated by at most
//!    `aggregation_window_ms` milliseconds into a `TradeBlock`
//! 3. On side change or time gap, flush the block through the min/max
//!    contract filter
//! 4. Surviving blocks become [`TradeMarker`]s positioned at the
//!    containing candle's X coordinate
//!
//! **Absorption detection** (optional, enabled by default):
//! Flushed blocks with a tight price range are additionally fed into a
//! confirmation state machine. If subsequent price action reverses by
//! `confirmation_ticks`, the absorption is confirmed and a cross-shaped
//! marker is emitted. If price continues through (`break_ticks`) or
//! times out, the candidate is discarded.
//!
//! Supports incremental computation via [`Study::append_trades`] — only
//! newly arrived trades are processed, and the pending (incomplete) block
//! is carried forward between calls.

use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue, StudyConfig,
    Visibility,
};
use crate::core::{Study, StudyCategory, StudyInput, StudyPlacement};
use crate::error::StudyError;
use crate::output::{
    MarkerData, MarkerRenderConfig, MarkerShape, StudyOutput, TradeMarker, TradeMarkerDebug,
};
use data::{Candle, ChartBasis, SerializableColor, Trade};

const DEFAULT_DAYS_TO_LOAD: i64 = 1;
const DEFAULT_FILTER_MIN: i64 = 50;
const DEFAULT_FILTER_MAX: i64 = 0;
const DEFAULT_AGGREGATION_WINDOW_MS: i64 = 40;

// Absorption defaults
const DEFAULT_ABSORPTION_MAX_RANGE_TICKS: i64 = 4;
const DEFAULT_ABSORPTION_CONFIRMATION_TICKS: i64 = 3;
const DEFAULT_ABSORPTION_TIMEOUT_MS: i64 = 5000;
const DEFAULT_ABSORPTION_BREAK_TICKS: i64 = 6;

// Theme-matched colors (Kairos default palette: success #51CDA0, danger #C0504D)
#[allow(clippy::approx_constant)]
const DEFAULT_BUY_COLOR: SerializableColor = SerializableColor {
    r: 0.318,
    g: 0.804,
    b: 0.627,
    a: 1.0,
};

const DEFAULT_SELL_COLOR: SerializableColor = SerializableColor {
    r: 0.753,
    g: 0.314,
    b: 0.302,
    a: 1.0,
};

const DEFAULT_TEXT_COLOR: SerializableColor = SerializableColor {
    r: 0.88,
    g: 0.88,
    b: 0.88,
    a: 0.9,
};

/// Detects institutional-scale executions by aggregating consecutive
/// same-side fills within a time window and rendering them as sized
/// markers on the chart.
///
/// The full detection pipeline is:
/// 1. Iterate raw trades in chronological order.
/// 2. Merge consecutive same-side fills separated by at most
///    `aggregation_window_ms` into a `TradeBlock`.
/// 3. On side change or time gap, flush the block through the min/max
///    contract filter.
/// 4. Surviving blocks become [`TradeMarker`]s positioned at the
///    containing candle's X coordinate.
///
/// Supports incremental computation via [`Study::append_trades`] --
/// only newly arrived trades are processed, and the pending
/// (incomplete) block is carried forward between calls so that a
/// multi-fill execution spanning two `append_trades` invocations is
/// still aggregated correctly.
pub struct BigTradesStudy {
    /// Persisted user-configurable parameter values.
    config: StudyConfig,
    /// Most recently computed study output (markers or empty).
    output: StudyOutput,
    /// Schema of user-adjustable parameters shown in the settings UI.
    params: Vec<ParameterDef>,
    /// Number of trades already processed (for incremental append).
    processed_trade_count: usize,
    /// In-progress aggregation block awaiting more fills or flush.
    pending_block: Option<TradeBlock>,
    /// Completed markers from prior flushes.
    accumulated_markers: Vec<TradeMarker>,
    /// Absorption candidates awaiting price confirmation.
    pending_absorptions: Vec<PendingAbsorption>,
    /// Cached render config — rebuilt on parameter or data change.
    cached_render_config: MarkerRenderConfig,
    /// Pre-built tick-chart candle boundaries (start, end) pairs.
    cached_candle_boundaries: Option<Vec<(u64, u64)>>,
    /// Candle count when boundaries were last built.
    cached_boundaries_candle_count: usize,
}

impl BigTradesStudy {
    /// Create a new big trades study with default parameters.
    ///
    /// Registers all user-configurable parameters (filter thresholds,
    /// aggregation window, marker shape/size/color, text and debug
    /// toggles), seeds the [`StudyConfig`] with their defaults, and
    /// pre-builds the cached [`MarkerRenderConfig`].
    pub fn new() -> Self {
        let params = vec![
            // ── Data Settings ────────────────────────────────
            ParameterDef {
                key: "days_to_load".into(),
                label: "Days to Load".into(),
                description: "Number of days of trade data to analyze".into(),
                kind: ParameterKind::Integer { min: 1, max: 30 },
                default: ParameterValue::Integer(DEFAULT_DAYS_TO_LOAD),
                tab: ParameterTab::Parameters,
                section: None,
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "filter_min".into(),
                label: "Filter Min".into(),
                description: "Minimum contracts to display (0 = none)".into(),
                kind: ParameterKind::Integer { min: 0, max: 2000 },
                default: ParameterValue::Integer(DEFAULT_FILTER_MIN),
                tab: ParameterTab::Parameters,
                section: None,
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "filter_max".into(),
                label: "Filter Max".into(),
                description: "Maximum contracts to display (0 = none)".into(),
                kind: ParameterKind::Integer { min: 0, max: 2000 },
                default: ParameterValue::Integer(DEFAULT_FILTER_MAX),
                tab: ParameterTab::Parameters,
                section: None,
                order: 2,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "aggregation_window_ms".into(),
                label: "Aggregation Window".into(),
                description: "Max ms gap between fills to merge".into(),
                kind: ParameterKind::Integer { min: 10, max: 500 },
                default: ParameterValue::Integer(DEFAULT_AGGREGATION_WINDOW_MS),
                tab: ParameterTab::Parameters,
                section: None,
                order: 3,
                format: DisplayFormat::Integer { suffix: " ms" },
                visible_when: Visibility::Always,
            },
            // ── Style / General ──────────────────────────────
            ParameterDef {
                key: "marker_shape".into(),
                label: "Marker Shape".into(),
                description: "Shape used for markers".into(),
                kind: ParameterKind::Choice {
                    options: &["Circle", "Square", "Text Only"],
                },
                default: ParameterValue::Choice("Circle".to_string()),
                tab: ParameterTab::Style,
                section: None,
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "hollow".into(),
                label: "Hollow Fill".into(),
                description: "Draw markers as outlines only".into(),
                kind: ParameterKind::Boolean,
                default: ParameterValue::Boolean(false),
                tab: ParameterTab::Style,
                section: None,
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "show_text".into(),
                label: "Show Text".into(),
                description: "Show contract count text on markers".into(),
                kind: ParameterKind::Boolean,
                default: ParameterValue::Boolean(true),
                tab: ParameterTab::Display,
                section: None,
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            // ── Style / Size ─────────────────────────────────
            ParameterDef {
                key: "min_size".into(),
                label: "Min Size".into(),
                description: "Minimum marker radius in pixels".into(),
                kind: ParameterKind::Float {
                    min: 2.0,
                    max: 60.0,
                    step: 1.0,
                },
                default: ParameterValue::Float(8.0),
                tab: ParameterTab::Style,
                section: None,
                order: 2,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "max_size".into(),
                label: "Max Size".into(),
                description: "Maximum marker radius in pixels".into(),
                kind: ParameterKind::Float {
                    min: 10.0,
                    max: 100.0,
                    step: 1.0,
                },
                default: ParameterValue::Float(36.0),
                tab: ParameterTab::Style,
                section: None,
                order: 3,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            // ── Style / Color ────────────────────────────────
            ParameterDef {
                key: "min_opacity".into(),
                label: "Min Opacity".into(),
                description: "Opacity for smallest markers".into(),
                kind: ParameterKind::Float {
                    min: 0.0,
                    max: 1.0,
                    step: 0.05,
                },
                default: ParameterValue::Float(0.10),
                tab: ParameterTab::Style,
                section: None,
                order: 4,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "max_opacity".into(),
                label: "Max Opacity".into(),
                description: "Opacity for largest markers".into(),
                kind: ParameterKind::Float {
                    min: 0.0,
                    max: 1.0,
                    step: 0.05,
                },
                default: ParameterValue::Float(0.60),
                tab: ParameterTab::Style,
                section: None,
                order: 5,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "buy_color".into(),
                label: "Buy Color".into(),
                description: "Color for buy (aggressor) markers".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_BUY_COLOR),
                tab: ParameterTab::Style,
                section: None,
                order: 6,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "sell_color".into(),
                label: "Sell Color".into(),
                description: "Color for sell (aggressor) markers".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_SELL_COLOR),
                tab: ParameterTab::Style,
                section: None,
                order: 7,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            // ── Style / Text ─────────────────────────────────
            ParameterDef {
                key: "text_size".into(),
                label: "Text Size".into(),
                description: "Font size for marker labels".into(),
                kind: ParameterKind::Float {
                    min: 6.0,
                    max: 20.0,
                    step: 0.5,
                },
                default: ParameterValue::Float(10.0),
                tab: ParameterTab::Style,
                section: None,
                order: 8,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "text_color".into(),
                label: "Text Color".into(),
                description: "Color for marker label text".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_TEXT_COLOR),
                tab: ParameterTab::Style,
                section: None,
                order: 9,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            // ── Debug ────────────────────────────────────────
            ParameterDef {
                key: "show_debug".into(),
                label: "Show Debug".into(),
                description: "Show debug annotations on markers".into(),
                kind: ParameterKind::Boolean,
                default: ParameterValue::Boolean(false),
                tab: ParameterTab::Display,
                section: None,
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            // ── Absorption ────────────────────────────────────
            ParameterDef {
                key: "show_absorption".into(),
                label: "Show Absorption".into(),
                description: "Detect passive absorption at tight price levels".into(),
                kind: ParameterKind::Boolean,
                default: ParameterValue::Boolean(true),
                tab: ParameterTab::Absorption,
                section: None,
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "absorption_max_range_ticks".into(),
                label: "Max Price Range".into(),
                description: "Max ticks of price movement in block".into(),
                kind: ParameterKind::Integer { min: 1, max: 20 },
                default: ParameterValue::Integer(DEFAULT_ABSORPTION_MAX_RANGE_TICKS),
                tab: ParameterTab::Absorption,
                section: None,
                order: 1,
                format: DisplayFormat::Integer { suffix: " ticks" },
                visible_when: Visibility::WhenTrue("show_absorption"),
            },
            ParameterDef {
                key: "absorption_confirmation_ticks".into(),
                label: "Confirmation Ticks".into(),
                description: "Ticks of reversal needed to confirm".into(),
                kind: ParameterKind::Integer { min: 1, max: 20 },
                default: ParameterValue::Integer(DEFAULT_ABSORPTION_CONFIRMATION_TICKS),
                tab: ParameterTab::Absorption,
                section: None,
                order: 2,
                format: DisplayFormat::Integer { suffix: " ticks" },
                visible_when: Visibility::WhenTrue("show_absorption"),
            },
            ParameterDef {
                key: "absorption_timeout_ms".into(),
                label: "Confirmation Timeout".into(),
                description: "Max ms to wait for confirmation".into(),
                kind: ParameterKind::Integer {
                    min: 500,
                    max: 30000,
                },
                default: ParameterValue::Integer(DEFAULT_ABSORPTION_TIMEOUT_MS),
                tab: ParameterTab::Absorption,
                section: None,
                order: 3,
                format: DisplayFormat::Integer { suffix: " ms" },
                visible_when: Visibility::WhenTrue("show_absorption"),
            },
            ParameterDef {
                key: "absorption_break_ticks".into(),
                label: "Break Ticks".into(),
                description: "Ticks of continuation to invalidate".into(),
                kind: ParameterKind::Integer { min: 1, max: 30 },
                default: ParameterValue::Integer(DEFAULT_ABSORPTION_BREAK_TICKS),
                tab: ParameterTab::Absorption,
                section: None,
                order: 4,
                format: DisplayFormat::Integer { suffix: " ticks" },
                visible_when: Visibility::WhenTrue("show_absorption"),
            },
            ParameterDef {
                key: "absorption_buy_color".into(),
                label: "Buy Absorption Color".into(),
                description: "Color when sell aggression absorbed (bullish)".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_BUY_COLOR),
                tab: ParameterTab::Absorption,
                section: None,
                order: 5,
                format: DisplayFormat::Auto,
                visible_when: Visibility::WhenTrue("show_absorption"),
            },
            ParameterDef {
                key: "absorption_sell_color".into(),
                label: "Sell Absorption Color".into(),
                description: "Color when buy aggression absorbed (bearish)".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_SELL_COLOR),
                tab: ParameterTab::Absorption,
                section: None,
                order: 6,
                format: DisplayFormat::Auto,
                visible_when: Visibility::WhenTrue("show_absorption"),
            },
        ];

        let mut config = StudyConfig::new("big_trades");
        for p in &params {
            config.set(p.key.clone(), p.default.clone());
        }

        let mut study = Self {
            config,
            output: StudyOutput::Empty,
            params,
            processed_trade_count: 0,
            pending_block: None,
            accumulated_markers: Vec::new(),
            pending_absorptions: Vec::new(),
            cached_render_config: MarkerRenderConfig::default(),
            cached_candle_boundaries: None,
            cached_boundaries_candle_count: 0,
        };
        study.cached_render_config = study.build_marker_render_config();
        study
    }

    /// Read current parameters from config.
    fn read_params(&self, tick_size_units: i64) -> ComputeParams {
        ComputeParams {
            filter_min: self.config.get_int("filter_min", DEFAULT_FILTER_MIN) as f64,
            filter_max: self.config.get_int("filter_max", DEFAULT_FILTER_MAX) as f64,
            window_ms: self
                .config
                .get_int("aggregation_window_ms", DEFAULT_AGGREGATION_WINDOW_MS)
                as u64,
            buy_color: self.config.get_color("buy_color", DEFAULT_BUY_COLOR),
            sell_color: self.config.get_color("sell_color", DEFAULT_SELL_COLOR),
            show_text: self.config.get_bool("show_text", true),
            show_debug: self.config.get_bool("show_debug", false),
            show_absorption: self.config.get_bool("show_absorption", true),
            absorption_max_range_ticks: self.config.get_int(
                "absorption_max_range_ticks",
                DEFAULT_ABSORPTION_MAX_RANGE_TICKS,
            ),
            absorption_confirmation_ticks: self.config.get_int(
                "absorption_confirmation_ticks",
                DEFAULT_ABSORPTION_CONFIRMATION_TICKS,
            ),
            absorption_timeout_ms: self
                .config
                .get_int("absorption_timeout_ms", DEFAULT_ABSORPTION_TIMEOUT_MS)
                as u64,
            absorption_break_ticks: self
                .config
                .get_int("absorption_break_ticks", DEFAULT_ABSORPTION_BREAK_TICKS),
            absorption_buy_color: self
                .config
                .get_color("absorption_buy_color", DEFAULT_BUY_COLOR),
            absorption_sell_color: self
                .config
                .get_color("absorption_sell_color", DEFAULT_SELL_COLOR),
            tick_size_units: tick_size_units.max(1),
        }
    }

    /// Core processing loop: aggregates trades into blocks, flushes
    /// big-trade markers, and runs the absorption state machine.
    fn process_trades(
        trades: &[Trade],
        pending: &mut Option<TradeBlock>,
        markers: &mut Vec<TradeMarker>,
        pending_absorptions: &mut Vec<PendingAbsorption>,
        params: &ComputeParams,
        candles: &[Candle],
        basis: &ChartBasis,
        candle_boundaries: Option<&[(u64, u64)]>,
    ) {
        let is_time_based = matches!(basis, ChartBasis::Time(_));

        for trade in trades {
            let qty = trade.quantity.value();
            if qty <= 0.0 {
                continue;
            }

            let price_units = trade.price.units();
            let time = trade.time.0;
            let is_buy = trade.side.is_buy();

            // Run absorption confirmation on every trade
            if params.show_absorption {
                update_pendings(
                    pending_absorptions,
                    markers,
                    price_units,
                    time,
                    params,
                    candles,
                    basis,
                    candle_boundaries,
                );
            }

            let candle_open = if is_time_based {
                find_candle_open(time, candles)
            } else {
                0
            };

            if let Some(block) = pending {
                let same_candle = !is_time_based || candle_open == block.candle_open;

                if block.is_buy == is_buy
                    && time.saturating_sub(block.last_time) <= params.window_ms
                    && same_candle
                {
                    // Merge into current block
                    block.vwap_numerator += price_units as f64 * qty;
                    block.total_qty += qty;
                    block.last_time = time;
                    block.fill_count += 1;
                    block.min_price_units = block.min_price_units.min(price_units);
                    block.max_price_units = block.max_price_units.max(price_units);
                } else {
                    // Flush current block
                    if let Some(marker) =
                        flush_block(block, params, candles, basis, candle_boundaries)
                    {
                        markers.push(marker);
                    }
                    // Check if flushed block qualifies as absorption
                    if params.show_absorption
                        && let Some(pa) = try_create_pending(block, params)
                    {
                        pending_absorptions.push(pa);
                    }
                    *pending = Some(TradeBlock::new(is_buy, price_units, qty, time, candle_open));
                }
            } else {
                *pending = Some(TradeBlock::new(is_buy, price_units, qty, time, candle_open));
            }
        }
    }

    /// Build output from accumulated markers + optional pending marker.
    fn rebuild_output(
        accumulated: &[TradeMarker],
        pending_marker: Option<&TradeMarker>,
        render_config: &MarkerRenderConfig,
    ) -> StudyOutput {
        let total = accumulated.len() + pending_marker.is_some() as usize;
        if total == 0 {
            return StudyOutput::Empty;
        }
        let mut markers = Vec::with_capacity(total);
        markers.extend_from_slice(accumulated);
        if let Some(pm) = pending_marker {
            markers.push(pm.clone());
        }
        StudyOutput::Markers(MarkerData {
            markers,
            render_config: *render_config,
        })
    }
}

impl Default for BigTradesStudy {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of user-configurable parameters for a single compute pass.
///
/// Extracted from [`StudyConfig`] at the start of each compute/append
/// to avoid repeated hash lookups during the hot loop.
struct ComputeParams {
    /// Minimum contract count for a block to produce a marker.
    /// Blocks with fewer contracts are silently discarded.
    filter_min: f64,
    /// Maximum contract count (0 = no upper limit).
    /// Blocks exceeding this are discarded (useful for filtering
    /// out spread/roll trades that inflate the count).
    filter_max: f64,
    /// Maximum millisecond gap between consecutive fills before the
    /// aggregation window breaks and the current block is flushed.
    window_ms: u64,
    /// Fill color for buy-side (aggressor) markers.
    buy_color: SerializableColor,
    /// Fill color for sell-side (aggressor) markers.
    sell_color: SerializableColor,
    /// Whether to render the contract-count text label on each marker.
    show_text: bool,
    /// Whether to attach [`TradeMarkerDebug`] metadata to each marker
    /// for diagnostic overlay rendering.
    show_debug: bool,
    // ── Absorption parameters ────────────────────────────
    /// Whether absorption detection is enabled.
    show_absorption: bool,
    /// Max ticks of price range for an absorption candidate.
    absorption_max_range_ticks: i64,
    /// Ticks of reversal needed to confirm absorption.
    absorption_confirmation_ticks: i64,
    /// Max ms to wait for confirmation before discarding.
    absorption_timeout_ms: u64,
    /// Ticks of continuation that invalidates a pending absorption.
    absorption_break_ticks: i64,
    /// Color for confirmed buy absorption (sell aggression absorbed).
    absorption_buy_color: SerializableColor,
    /// Color for confirmed sell absorption (buy aggression absorbed).
    absorption_sell_color: SerializableColor,
    /// Tick size in price units for tick-based distance calculations.
    tick_size_units: i64,
}

/// Format contract count for display.
fn format_contracts(contracts: f64) -> String {
    if contracts >= 1000.0 {
        format!("{:.1}K", contracts / 1000.0)
    } else {
        format!("{}", contracts as u64)
    }
}

/// Accumulator for aggregating consecutive same-side fills into a single
/// logical execution block.
///
/// Tracks running VWAP components, fill count, price range, and the time
/// span of the aggregated fills. A block is started by the first fill and
/// extended by each subsequent fill that matches the same side and arrives
/// within the `aggregation_window_ms` threshold. It is flushed into a
/// [`TradeMarker`] when the next trade breaks the aggregation window,
/// changes side, or crosses a candle boundary.
struct TradeBlock {
    /// `true` for buy-side (aggressor) fills, `false` for sell-side.
    is_buy: bool,
    /// Running sum of `price_units * qty` for VWAP computation.
    vwap_numerator: f64,
    /// Running sum of quantities across all fills in this block.
    total_qty: f64,
    /// Timestamp of the first fill in the block.
    first_time: u64,
    /// Timestamp of the most recent fill in the block.
    last_time: u64,
    /// Number of individual fills merged into this block.
    fill_count: u32,
    /// Lowest price (in i64 units) seen across fills.
    min_price_units: i64,
    /// Highest price (in i64 units) seen across fills.
    max_price_units: i64,
    /// Containing candle's open time (time-based charts only, 0 otherwise).
    candle_open: u64,
}

impl TradeBlock {
    /// Start a new block from a single fill.
    ///
    /// Initializes the VWAP numerator to `price_units * qty`, sets the
    /// fill count to 1, and records the fill's timestamp as both the
    /// first and last time.
    fn new(is_buy: bool, price_units: i64, qty: f64, time: u64, candle_open: u64) -> Self {
        Self {
            is_buy,
            vwap_numerator: price_units as f64 * qty,
            total_qty: qty,
            first_time: time,
            last_time: time,
            fill_count: 1,
            min_price_units: price_units,
            max_price_units: price_units,
            candle_open,
        }
    }

    /// Compute the VWAP in i64 price units.
    ///
    /// Returns `vwap_numerator / total_qty` rounded to the nearest
    /// integer, or `0` when `total_qty` is zero (degenerate block).
    fn vwap_units(&self) -> i64 {
        if self.total_qty > 0.0 {
            (self.vwap_numerator / self.total_qty).round() as i64
        } else {
            0
        }
    }

    /// Midpoint timestamp between the first and last fill.
    ///
    /// Used to locate the block within the candle grid -- the mid-time
    /// is binary-searched against candle boundaries to determine which
    /// candle's X coordinate the resulting marker should snap to.
    fn mid_time(&self) -> u64 {
        (self.first_time + self.last_time) / 2
    }
}

/// A candidate absorption awaiting confirmation via subsequent trades.
struct PendingAbsorption {
    /// VWAP price in i64 units.
    vwap_units: i64,
    /// Total contracts in the absorption block.
    total_qty: f64,
    /// `true` if the aggressor was buying (absorbed by passive sellers).
    aggressor_is_buy: bool,
    /// Timestamp when the block ended (last fill time).
    block_end_time: u64,
    /// First fill time.
    first_time: u64,
    /// Last fill time.
    last_time: u64,
    /// Fill count.
    fill_count: u32,
    /// Min price in units.
    min_price_units: i64,
    /// Max price in units.
    max_price_units: i64,
    /// VWAP numerator for debug.
    vwap_numerator: f64,
}

impl PendingAbsorption {
    fn mid_time(&self) -> u64 {
        (self.first_time + self.last_time) / 2
    }
}

/// Build candle boundary lookup table for tick-based charts.
///
/// Returns `Some(vec)` of `(start_time, end_time)` pairs for tick charts,
/// where `end_time` of candle `i` equals the `start_time` of candle
/// `i + 1`, and the final candle's `end_time` is `u64::MAX` (open-ended).
/// This allows binary-searching a trade's timestamp to find which tick
/// candle it belongs to.
///
/// Returns `None` for time-based charts where candle open times are
/// monotonically increasing and a simple binary search on `candle.time`
/// suffices.
fn build_candle_boundaries(candles: &[Candle], basis: &ChartBasis) -> Option<Vec<(u64, u64)>> {
    match basis {
        ChartBasis::Tick(_) => {
            let len = candles.len();
            Some(
                candles
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let end = if i + 1 < len {
                            candles[i + 1].time.0
                        } else {
                            u64::MAX
                        };
                        (c.time.0, end)
                    })
                    .collect(),
            )
        }
        _ => None,
    }
}

/// Binary-search for the candle containing `time` and return its open.
fn find_candle_open(time: u64, candles: &[Candle]) -> u64 {
    if candles.is_empty() {
        return 0;
    }
    let idx = candles
        .binary_search_by_key(&time, |c| c.time.0)
        .unwrap_or_else(|i| i.saturating_sub(1));
    let idx = idx.min(candles.len().saturating_sub(1));
    candles[idx].time.0
}

/// Flush a completed [`TradeBlock`] into a [`TradeMarker`] if it passes
/// the min/max contract filter.
///
/// The filter logic:
/// - If `filter_min > 0` and the block's `total_qty` is below it, the
///   block is silently discarded (not large enough).
/// - If `filter_max > 0` and the block's `total_qty` exceeds it, the
///   block is also discarded (likely a spread/roll artifact).
///
/// For surviving blocks, the X coordinate is mapped as follows:
/// - **Time-based charts**: binary-search `mid_time()` against candle
///   open times and snap to the matching candle's open timestamp.
/// - **Tick-based charts**: binary-search `mid_time()` against the
///   pre-built `candle_boundaries` and convert the matched index to a
///   reverse index (newest candle = 0) for the renderer.
fn flush_block(
    block: &TradeBlock,
    params: &ComputeParams,
    candles: &[Candle],
    basis: &ChartBasis,
    candle_boundaries: Option<&[(u64, u64)]>,
) -> Option<TradeMarker> {
    if params.filter_min > 0.0 && block.total_qty < params.filter_min {
        return None;
    }
    if params.filter_max > 0.0 && block.total_qty > params.filter_max {
        return None;
    }

    let color = if block.is_buy {
        params.buy_color
    } else {
        params.sell_color
    };
    let label = if params.show_text {
        Some(format_contracts(block.total_qty))
    } else {
        None
    };

    // Map timestamp to appropriate X coordinate
    let time = match basis {
        ChartBasis::Time(_) => {
            // Snap to the containing candle's open time so the marker
            // is centered on the correct candle regardless of timeframe.
            let mid = block.mid_time();
            if candles.is_empty() {
                mid
            } else {
                let idx = candles
                    .binary_search_by_key(&mid, |c| c.time.0)
                    .unwrap_or_else(|i| i.saturating_sub(1));
                let idx = idx.min(candles.len().saturating_sub(1));
                candles[idx].time.0
            }
        }
        ChartBasis::Tick(_) => {
            if let Some(bounds) = candle_boundaries {
                if bounds.is_empty() {
                    0
                } else {
                    let mid = block.mid_time();
                    let idx = bounds
                        .binary_search_by(|(start, _)| start.cmp(&mid))
                        .unwrap_or_else(|i| i.saturating_sub(1));
                    let idx = idx.min(bounds.len().saturating_sub(1));
                    // Reverse index (newest = 0)
                    (bounds.len().saturating_sub(1) - idx) as u64
                }
            } else {
                // Fallback: binary search candles directly
                let mid = block.mid_time();
                let idx = candles
                    .binary_search_by_key(&mid, |c| c.time.0)
                    .unwrap_or_else(|i| i.saturating_sub(1));
                let idx = idx.min(candles.len().saturating_sub(1));
                (candles.len().saturating_sub(1) - idx) as u64
            }
        }
    };

    let debug = if params.show_debug {
        Some(TradeMarkerDebug {
            fill_count: block.fill_count,
            first_fill_time: block.first_time,
            last_fill_time: block.last_time,
            price_min_units: block.min_price_units,
            price_max_units: block.max_price_units,
            vwap_numerator: block.vwap_numerator,
            vwap_denominator: block.total_qty,
        })
    } else {
        None
    };

    Some(TradeMarker {
        time,
        price: block.vwap_units(),
        contracts: block.total_qty,
        is_buy: block.is_buy,
        color,
        label,
        debug,
        shape_override: None,
    })
}

/// Try to create a [`PendingAbsorption`] from a flushed
/// [`TradeBlock`]. Returns `None` if the block doesn't qualify
/// (wrong size or too wide a price range).
fn try_create_pending(block: &TradeBlock, params: &ComputeParams) -> Option<PendingAbsorption> {
    // Size filter (reuse same filter_min/max as big trades)
    if params.filter_min > 0.0 && block.total_qty < params.filter_min {
        return None;
    }
    if params.filter_max > 0.0 && block.total_qty > params.filter_max {
        return None;
    }

    // Tight price range filter — the key differentiator
    if params.tick_size_units > 0 {
        let range = block.max_price_units - block.min_price_units;
        let range_ticks = range / params.tick_size_units;
        if range_ticks > params.absorption_max_range_ticks {
            return None;
        }
    }

    Some(PendingAbsorption {
        vwap_units: block.vwap_units(),
        total_qty: block.total_qty,
        aggressor_is_buy: block.is_buy,
        block_end_time: block.last_time,
        first_time: block.first_time,
        last_time: block.last_time,
        fill_count: block.fill_count,
        min_price_units: block.min_price_units,
        max_price_units: block.max_price_units,
        vwap_numerator: block.vwap_numerator,
    })
}

/// Run the confirmation state machine on all pending absorptions
/// given the current trade's price and time. Confirmed absorptions
/// emit cross-shaped markers; broken/timed-out ones are discarded.
fn update_pendings(
    pendings: &mut Vec<PendingAbsorption>,
    markers: &mut Vec<TradeMarker>,
    trade_price_units: i64,
    trade_time: u64,
    params: &ComputeParams,
    candles: &[Candle],
    basis: &ChartBasis,
    candle_boundaries: Option<&[(u64, u64)]>,
) {
    let mut i = 0;
    while i < pendings.len() {
        let p = &pendings[i];
        let elapsed = trade_time.saturating_sub(p.block_end_time);

        // Timeout check
        if elapsed > params.absorption_timeout_ms {
            pendings.swap_remove(i);
            continue;
        }

        debug_assert!(
            params.tick_size_units > 0,
            "tick_size_units must be positive (clamped in read_params)"
        );
        let tick_units = params.tick_size_units;
        let distance_from_vwap = trade_price_units - p.vwap_units;

        if p.aggressor_is_buy {
            // Buy aggression absorbed by passive sellers →
            // confirm if price drops (bearish reversal)
            let reversal_ticks = -distance_from_vwap / tick_units;
            let continuation_ticks = distance_from_vwap / tick_units;

            if reversal_ticks >= params.absorption_confirmation_ticks {
                let pending = pendings.swap_remove(i);
                emit_absorption_marker(
                    &pending,
                    markers,
                    params,
                    candles,
                    basis,
                    candle_boundaries,
                );
                continue;
            }
            if continuation_ticks >= params.absorption_break_ticks {
                pendings.swap_remove(i);
                continue;
            }
        } else {
            // Sell aggression absorbed by passive buyers →
            // confirm if price rises (bullish reversal)
            let reversal_ticks = distance_from_vwap / tick_units;
            let continuation_ticks = -distance_from_vwap / tick_units;

            if reversal_ticks >= params.absorption_confirmation_ticks {
                let pending = pendings.swap_remove(i);
                emit_absorption_marker(
                    &pending,
                    markers,
                    params,
                    candles,
                    basis,
                    candle_boundaries,
                );
                continue;
            }
            if continuation_ticks >= params.absorption_break_ticks {
                pendings.swap_remove(i);
                continue;
            }
        }

        i += 1;
    }
}

/// Convert a confirmed [`PendingAbsorption`] into a [`TradeMarker`]
/// with `shape_override: Some(MarkerShape::Cross)`. Color is inverted
/// (passive side won): sell aggression → buy color, buy aggression →
/// sell color.
fn emit_absorption_marker(
    pending: &PendingAbsorption,
    markers: &mut Vec<TradeMarker>,
    params: &ComputeParams,
    candles: &[Candle],
    basis: &ChartBasis,
    candle_boundaries: Option<&[(u64, u64)]>,
) {
    // Color based on what was absorbed (inverted from aggressor):
    //   sell aggression absorbed → bullish → absorption_buy_color
    //   buy aggression absorbed → bearish → absorption_sell_color
    let color = if pending.aggressor_is_buy {
        params.absorption_sell_color
    } else {
        params.absorption_buy_color
    };

    let label = if params.show_text {
        Some(format_contracts(pending.total_qty))
    } else {
        None
    };

    let mid = pending.mid_time();
    let time = map_time_to_x(mid, candles, basis, candle_boundaries);

    let debug = if params.show_debug {
        Some(TradeMarkerDebug {
            fill_count: pending.fill_count,
            first_fill_time: pending.first_time,
            last_fill_time: pending.last_time,
            price_min_units: pending.min_price_units,
            price_max_units: pending.max_price_units,
            vwap_numerator: pending.vwap_numerator,
            vwap_denominator: pending.total_qty,
        })
    } else {
        None
    };

    markers.push(TradeMarker {
        time,
        price: pending.vwap_units,
        contracts: pending.total_qty,
        is_buy: !pending.aggressor_is_buy,
        color,
        label,
        debug,
        shape_override: Some(MarkerShape::Cross),
    });
}

/// Map a trade block's mid-time to the appropriate X coordinate.
fn map_time_to_x(
    mid_time: u64,
    candles: &[Candle],
    basis: &ChartBasis,
    candle_boundaries: Option<&[(u64, u64)]>,
) -> u64 {
    match basis {
        ChartBasis::Time(_) => {
            if candles.is_empty() {
                mid_time
            } else {
                let idx = candles
                    .binary_search_by_key(&mid_time, |c| c.time.0)
                    .unwrap_or_else(|i| i.saturating_sub(1));
                let idx = idx.min(candles.len().saturating_sub(1));
                candles[idx].time.0
            }
        }
        ChartBasis::Tick(_) => {
            if let Some(bounds) = candle_boundaries {
                if bounds.is_empty() {
                    0
                } else {
                    let idx = bounds
                        .binary_search_by(|(start, _)| start.cmp(&mid_time))
                        .unwrap_or_else(|i| i.saturating_sub(1));
                    let idx = idx.min(bounds.len().saturating_sub(1));
                    (bounds.len().saturating_sub(1) - idx) as u64
                }
            } else {
                let idx = candles
                    .binary_search_by_key(&mid_time, |c| c.time.0)
                    .unwrap_or_else(|i| i.saturating_sub(1));
                let idx = idx.min(candles.len().saturating_sub(1));
                (candles.len().saturating_sub(1) - idx) as u64
            }
        }
    }
}

impl BigTradesStudy {
    /// Build marker render config from current parameters.
    /// Used by the renderer to control marker appearance.
    ///
    /// `scale_min` is set to `filter_min` (the threshold at which
    /// trades appear) and `scale_max` is derived from accumulated
    /// markers so that sizing scales linearly with contract count.
    pub fn build_marker_render_config(&self) -> MarkerRenderConfig {
        let shape_str = self.config.get_choice("marker_shape", "Circle");
        let shape = match shape_str {
            "Square" => MarkerShape::Square,
            "Text Only" => MarkerShape::TextOnly,
            _ => MarkerShape::Circle,
        };

        let filter_min = self.config.get_int("filter_min", DEFAULT_FILTER_MIN) as f64;
        let filter_max = self.config.get_int("filter_max", DEFAULT_FILTER_MAX) as f64;

        // Derive scale range from filter params + observed data.
        // scale_min = filter_min (smallest trade shown → min_size)
        // scale_max = filter_max if set, otherwise observed max
        let scale_min = filter_min.max(1.0);
        let observed_max = self
            .accumulated_markers
            .iter()
            .map(|m| m.contracts)
            .fold(0.0f64, f64::max);
        let scale_max = if filter_max > 0.0 {
            filter_max
        } else if observed_max > scale_min {
            observed_max
        } else {
            // Fallback: 10x filter_min gives reasonable default range
            scale_min * 10.0
        };

        MarkerRenderConfig {
            shape,
            hollow: self.config.get_bool("hollow", false),
            scale_min,
            scale_max,
            min_size: self.config.get_float("min_size", 8.0) as f32,
            max_size: self.config.get_float("max_size", 36.0) as f32,
            min_opacity: self.config.get_float("min_opacity", 0.10) as f32,
            max_opacity: self.config.get_float("max_opacity", 0.60) as f32,
            show_text: self.config.get_bool("show_text", true),
            text_size: self.config.get_float("text_size", 10.0) as f32,
            text_color: self.config.get_color("text_color", DEFAULT_TEXT_COLOR),
        }
    }
}

impl Study for BigTradesStudy {
    fn id(&self) -> &str {
        "big_trades"
    }

    fn name(&self) -> &str {
        "Big Trades"
    }

    fn category(&self) -> StudyCategory {
        StudyCategory::OrderFlow
    }

    fn placement(&self) -> StudyPlacement {
        StudyPlacement::Overlay
    }

    fn tab_labels(&self) -> Option<&[(&'static str, ParameterTab)]> {
        static LABELS: &[(&str, ParameterTab)] = &[
            ("Data", ParameterTab::Parameters),
            ("Style", ParameterTab::Style),
            ("Display", ParameterTab::Display),
            ("Absorption", ParameterTab::Absorption),
        ];
        Some(LABELS)
    }

    fn parameters(&self) -> &[ParameterDef] {
        &self.params
    }

    fn config(&self) -> &StudyConfig {
        &self.config
    }

    fn config_mut(&mut self) -> &mut StudyConfig {
        &mut self.config
    }

    fn set_parameter(&mut self, key: &str, value: ParameterValue) -> Result<(), StudyError> {
        let params = self.parameters();
        let def =
            params
                .iter()
                .find(|p| p.key == key)
                .ok_or_else(|| StudyError::InvalidParameter {
                    key: key.to_string(),
                    reason: "unknown parameter".to_string(),
                })?;
        def.validate(&value)
            .map_err(|reason| StudyError::InvalidParameter {
                key: key.to_string(),
                reason,
            })?;
        self.config_mut().set(key, value);
        // Rebuild cached render config when any parameter changes
        self.cached_render_config = self.build_marker_render_config();
        Ok(())
    }

    fn compute(&mut self, input: &StudyInput) -> Result<(), StudyError> {
        let trades = match input.trades {
            Some(t) if !t.is_empty() => t,
            _ => {
                self.output = StudyOutput::Empty;
                self.processed_trade_count = 0;
                self.pending_block = None;
                self.accumulated_markers.clear();
                self.pending_absorptions.clear();
                self.cached_candle_boundaries = None;
                self.cached_boundaries_candle_count = 0;
                return Ok(());
            }
        };

        let params = self.read_params(input.tick_size.units());
        let candle_boundaries = build_candle_boundaries(input.candles, &input.basis);

        let mut markers: Vec<TradeMarker> = Vec::with_capacity((trades.len() / 100).max(64));
        let mut pending: Option<TradeBlock> = None;
        let mut pending_absorptions: Vec<PendingAbsorption> = Vec::new();

        BigTradesStudy::process_trades(
            trades,
            &mut pending,
            &mut markers,
            &mut pending_absorptions,
            &params,
            input.candles,
            &input.basis,
            candle_boundaries.as_deref(),
        );

        // Flush final pending block into a separate marker
        // for output (keep pending_block unflushed for incremental).
        // Note: do NOT add the pending block as an absorption candidate
        // here — it's still in-flight and will be flushed properly on
        // the next side change or append.
        let pending_marker = pending.as_ref().and_then(|block| {
            flush_block(
                block,
                &params,
                input.candles,
                &input.basis,
                candle_boundaries.as_deref(),
            )
        });

        self.processed_trade_count = trades.len();
        self.pending_block = pending;
        self.accumulated_markers = markers;
        self.pending_absorptions = pending_absorptions;
        self.cached_candle_boundaries = candle_boundaries;
        self.cached_boundaries_candle_count = input.candles.len();
        self.cached_render_config = self.build_marker_render_config();
        self.output = Self::rebuild_output(
            &self.accumulated_markers,
            pending_marker.as_ref(),
            &self.cached_render_config,
        );
        Ok(())
    }

    /// Incremental trade processing. The `_new_trades` parameter is
    /// intentionally unused — this study slices from
    /// `input.trades[processed_count..]` instead, because trade
    /// aggregation blocks must align with candle boundaries and the
    /// pending block state from prior calls.
    fn append_trades(
        &mut self,
        _new_trades: &[Trade],
        input: &StudyInput,
    ) -> Result<(), StudyError> {
        let trades = match input.trades {
            Some(t) if !t.is_empty() => t,
            _ => return Ok(()),
        };

        // If no prior state, do full compute
        if self.processed_trade_count == 0 {
            return self.compute(input);
        }

        // Process only new trades
        if self.processed_trade_count >= trades.len() {
            return Ok(());
        }
        let new_slice = &trades[self.processed_trade_count..];

        let params = self.read_params(input.tick_size.units());

        // Reuse cached candle boundaries if candle count unchanged
        if input.candles.len() != self.cached_boundaries_candle_count {
            self.cached_candle_boundaries = build_candle_boundaries(input.candles, &input.basis);
            self.cached_boundaries_candle_count = input.candles.len();
        }

        let markers_before = self.accumulated_markers.len();

        BigTradesStudy::process_trades(
            new_slice,
            &mut self.pending_block,
            &mut self.accumulated_markers,
            &mut self.pending_absorptions,
            &params,
            input.candles,
            &input.basis,
            self.cached_candle_boundaries.as_deref(),
        );

        self.processed_trade_count = trades.len();

        let markers_changed = self.accumulated_markers.len() != markers_before;

        // Compute pending marker without cloning accumulated
        let pending_marker = self.pending_block.as_ref().and_then(|block| {
            flush_block(
                block,
                &params,
                input.candles,
                &input.basis,
                self.cached_candle_boundaries.as_deref(),
            )
        });

        // Only rebuild output if something changed
        if markers_changed || pending_marker.is_some() {
            if markers_changed {
                self.cached_render_config = self.build_marker_render_config();
            }
            self.output = Self::rebuild_output(
                &self.accumulated_markers,
                pending_marker.as_ref(),
                &self.cached_render_config,
            );
        }
        Ok(())
    }

    fn output(&self) -> &StudyOutput {
        &self.output
    }

    fn reset(&mut self) {
        self.output = StudyOutput::Empty;
        self.processed_trade_count = 0;
        self.pending_block = None;
        self.accumulated_markers.clear();
        self.pending_absorptions.clear();
        self.cached_candle_boundaries = None;
        self.cached_boundaries_candle_count = 0;
    }

    fn clone_study(&self) -> Box<dyn Study> {
        Box::new(Self {
            config: self.config.clone(),
            output: self.output.clone(),
            params: self.params.clone(),
            processed_trade_count: 0,
            pending_block: None,
            accumulated_markers: Vec::new(),
            pending_absorptions: Vec::new(),
            cached_render_config: self.cached_render_config,
            cached_candle_boundaries: None,
            cached_boundaries_candle_count: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::{Candle, ChartBasis, Price, Quantity, Side, Timeframe, Timestamp, Trade, Volume};

    fn make_trade(time_ms: u64, price: f32, qty: f64, side: Side) -> Trade {
        Trade {
            time: Timestamp::from_millis(time_ms),
            price: Price::from_f32(price),
            quantity: Quantity(qty),
            side,
        }
    }

    fn make_candle(time_ms: u64, price: f32) -> Candle {
        Candle::new(
            Timestamp::from_millis(time_ms),
            Price::from_f32(price),
            Price::from_f32(price),
            Price::from_f32(price),
            Price::from_f32(price),
            Volume(0.0),
            Volume(0.0),
        )
        .expect("test: valid candle")
    }

    fn study_input<'a>(candles: &'a [Candle], trades: &'a [Trade]) -> StudyInput<'a> {
        StudyInput {
            candles,
            trades: Some(trades),
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        }
    }

    /// Helper: convert marker price (i64 units) back to f64 for assertions
    fn marker_price_f64(marker: &TradeMarker) -> f64 {
        Price::from_units(marker.price).to_f64()
    }

    #[test]
    fn test_empty_trades() {
        let mut study = BigTradesStudy::new();
        let candles = vec![];
        let trades: Vec<Trade> = vec![];
        study.compute(&study_input(&candles, &trades)).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_single_large_fill() {
        let mut study = BigTradesStudy::new();
        let candles = vec![make_candle(1000, 100.0)];
        let trades = vec![make_trade(1000, 100.0, 100.0, Side::Buy)];
        study.compute(&study_input(&candles, &trades)).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Markers(_)),
            "Expected Markers"
        );
        let StudyOutput::Markers(md) = output else {
            unreachable!()
        };
        let m = &md.markers;
        assert_eq!(m.len(), 1);
        assert!(m[0].is_buy);
        assert!((m[0].contracts - 100.0).abs() < f64::EPSILON);
        assert!(
            (marker_price_f64(&m[0]) - 100.0).abs() < 0.01,
            "price: {} expected ~100.0",
            marker_price_f64(&m[0])
        );
        assert_eq!(m[0].label.as_deref(), Some("100"));
    }

    #[test]
    fn test_single_small_fill_below_threshold() {
        let mut study = BigTradesStudy::new();
        let candles = vec![make_candle(1000, 100.0)];
        let trades = vec![make_trade(1000, 100.0, 10.0, Side::Buy)];
        study.compute(&study_input(&candles, &trades)).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_three_same_side_fills_merge_with_correct_vwap() {
        let mut study = BigTradesStudy::new();
        let candles = vec![make_candle(1000, 100.0)];
        // 20 @ 100.0, 30 @ 101.0, 10 @ 102.0 => total 60
        // VWAP = (20*100 + 30*101 + 10*102) / 60 = 6050/60 = 100.8333...
        let trades = vec![
            make_trade(1000, 100.0, 20.0, Side::Buy),
            make_trade(1020, 101.0, 30.0, Side::Buy),
            make_trade(1040, 102.0, 10.0, Side::Buy),
        ];

        study
            .set_parameter("filter_min", ParameterValue::Integer(50))
            .unwrap();
        study.compute(&study_input(&candles, &trades)).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Markers(_)),
            "Expected Markers"
        );
        let StudyOutput::Markers(md) = output else {
            unreachable!()
        };
        let m = &md.markers;
        assert_eq!(m.len(), 1);
        assert!(m[0].is_buy);
        assert!(
            (m[0].contracts - 60.0).abs() < f64::EPSILON,
            "contracts: {}",
            m[0].contracts
        );
        let expected_vwap = 6050.0 / 60.0;
        assert!(
            (marker_price_f64(&m[0]) - expected_vwap).abs() < 0.01,
            "vwap: {} expected: {}",
            marker_price_f64(&m[0]),
            expected_vwap
        );
    }

    #[test]
    fn test_vwap_precision() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("filter_min", ParameterValue::Integer(1))
            .unwrap();
        let candles = vec![make_candle(1000, 100.0)];
        let trades = vec![
            make_trade(1000, 5432.75, 7.0, Side::Buy),
            make_trade(1010, 5433.25, 13.0, Side::Buy),
        ];
        study.compute(&study_input(&candles, &trades)).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Markers(_)),
            "Expected Markers"
        );
        let StudyOutput::Markers(md) = output else {
            unreachable!()
        };
        let m = &md.markers;
        assert_eq!(m.len(), 1);
        // VWAP = (7*5432.75 + 13*5433.25) / 20 = 5433.075
        let expected = (7.0 * 5432.75 + 13.0 * 5433.25) / 20.0;
        // With i64 units we have 10^-8 precision, so the
        // round-trip should be very close
        assert!(
            (marker_price_f64(&m[0]) - expected).abs() < 1e-6,
            "vwap: {:.10} expected: {:.10}",
            marker_price_f64(&m[0]),
            expected
        );
    }

    #[test]
    fn test_gap_exceeding_window_creates_two_markers() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("filter_min", ParameterValue::Integer(50))
            .unwrap();
        study
            .set_parameter("aggregation_window_ms", ParameterValue::Integer(100))
            .unwrap();

        let candles = vec![make_candle(1000, 100.0)];
        // Two groups separated by 200ms gap (> 100ms window)
        let trades = vec![
            make_trade(1000, 100.0, 60.0, Side::Buy),
            make_trade(1200, 101.0, 60.0, Side::Buy),
        ];
        study.compute(&study_input(&candles, &trades)).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Markers(_)),
            "Expected Markers"
        );
        let StudyOutput::Markers(md) = output else {
            unreachable!()
        };
        let m = &md.markers;
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn test_side_change_creates_separate_markers() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("filter_min", ParameterValue::Integer(50))
            .unwrap();

        let candles = vec![make_candle(1000, 100.0)];
        let trades = vec![
            make_trade(1000, 100.0, 60.0, Side::Buy),
            make_trade(1050, 100.0, 60.0, Side::Sell),
        ];
        study.compute(&study_input(&candles, &trades)).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Markers(_)),
            "Expected Markers"
        );
        let StudyOutput::Markers(md) = output else {
            unreachable!()
        };
        let m = &md.markers;
        assert_eq!(m.len(), 2);
        assert!(m[0].is_buy);
        assert!(!m[1].is_buy);
    }

    #[test]
    fn test_continuous_burst_merges_with_previous_fill_window() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("filter_min", ParameterValue::Integer(1))
            .unwrap();
        study
            .set_parameter("aggregation_window_ms", ParameterValue::Integer(150))
            .unwrap();

        let candles = vec![make_candle(1000, 100.0)];
        let trades: Vec<Trade> = (0..10)
            .map(|i| make_trade(1000 + i * 100, 100.0, 10.0, Side::Buy))
            .collect();
        study.compute(&study_input(&candles, &trades)).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Markers(_)),
            "Expected Markers"
        );
        let StudyOutput::Markers(md) = output else {
            unreachable!()
        };
        let m = &md.markers;
        assert_eq!(m.len(), 1, "Expected 1 merged marker, got {}", m.len());
        assert!(
            (m[0].contracts - 100.0).abs() < f64::EPSILON,
            "contracts: {}",
            m[0].contracts
        );
    }

    #[test]
    fn test_zero_quantity_trades_skipped() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("filter_min", ParameterValue::Integer(50))
            .unwrap();
        study
            .set_parameter("aggregation_window_ms", ParameterValue::Integer(150))
            .unwrap();

        let candles = vec![make_candle(1000, 100.0)];
        let trades = vec![
            make_trade(1000, 100.0, 60.0, Side::Buy),
            make_trade(1050, 100.0, 0.0, Side::Sell), // zero qty
            make_trade(1100, 100.0, 10.0, Side::Buy),
        ];
        study.compute(&study_input(&candles, &trades)).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Markers(_)),
            "Expected Markers"
        );
        let StudyOutput::Markers(md) = output else {
            unreachable!()
        };
        let m = &md.markers;
        assert_eq!(m.len(), 1);
        assert!(
            (m[0].contracts - 70.0).abs() < f64::EPSILON,
            "contracts: {}",
            m[0].contracts
        );
    }

    #[test]
    fn test_label_formatting() {
        assert_eq!(format_contracts(50.0), "50");
        assert_eq!(format_contracts(999.0), "999");
        assert_eq!(format_contracts(1000.0), "1.0K");
        assert_eq!(format_contracts(1200.0), "1.2K");
        assert_eq!(format_contracts(15000.0), "15.0K");
    }

    #[test]
    fn test_parameter_update_affects_output() {
        let mut study = BigTradesStudy::new();
        let candles = vec![make_candle(1000, 100.0)];
        let trades = vec![make_trade(1000, 100.0, 30.0, Side::Buy)];

        // Default filter_min=50, so 30 contracts won't show
        study.compute(&study_input(&candles, &trades)).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));

        // Lower threshold to 20
        study
            .set_parameter("filter_min", ParameterValue::Integer(20))
            .unwrap();
        study.compute(&study_input(&candles, &trades)).unwrap();
        assert!(matches!(study.output(), StudyOutput::Markers(_)));
    }

    #[test]
    fn test_clone_study_produces_independent_copy() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("filter_min", ParameterValue::Integer(10))
            .unwrap();

        let cloned = study.clone_study();
        assert_eq!(cloned.id(), "big_trades");
        assert_eq!(cloned.config().get_int("filter_min", 50), 10);

        // Mutating original doesn't affect clone
        study
            .set_parameter("filter_min", ParameterValue::Integer(99))
            .unwrap();
        assert_eq!(cloned.config().get_int("filter_min", 50), 10);
    }

    #[test]
    fn test_debug_annotations_populated() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("filter_min", ParameterValue::Integer(1))
            .unwrap();
        study
            .set_parameter("show_debug", ParameterValue::Boolean(true))
            .unwrap();

        let candles = vec![make_candle(1000, 100.0)];
        let trades = vec![
            make_trade(1000, 100.0, 20.0, Side::Buy),
            make_trade(1030, 101.0, 30.0, Side::Buy),
        ];
        study.compute(&study_input(&candles, &trades)).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Markers(_)),
            "Expected Markers"
        );
        let StudyOutput::Markers(md) = output else {
            unreachable!()
        };
        let m = &md.markers;
        assert_eq!(m.len(), 1);
        let debug = m[0].debug.as_ref().expect("debug should be set");
        assert_eq!(debug.fill_count, 2);
        assert_eq!(debug.first_fill_time, 1000);
        assert_eq!(debug.last_fill_time, 1030);
    }

    #[test]
    fn test_incremental_append() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("filter_min", ParameterValue::Integer(50))
            .unwrap();

        let candles = vec![make_candle(1000, 100.0)];

        // First batch: 30 contracts (below threshold)
        let trades1 = vec![make_trade(1000, 100.0, 30.0, Side::Buy)];
        study.compute(&study_input(&candles, &trades1)).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));

        // Append more trades to reach threshold
        let mut trades2 = trades1.clone();
        trades2.push(make_trade(1030, 100.0, 30.0, Side::Buy));

        let input = study_input(&candles, &trades2);
        study.append_trades(&trades2[1..], &input).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Markers(_)),
            "Expected Markers"
        );
        let StudyOutput::Markers(md) = output else {
            unreachable!()
        };
        let m = &md.markers;
        assert_eq!(m.len(), 1);
        assert!(
            (m[0].contracts - 60.0).abs() < f64::EPSILON,
            "contracts: {}",
            m[0].contracts
        );
    }

    #[test]
    fn test_time_based_marker_snaps_to_candle_open() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("filter_min", ParameterValue::Integer(1))
            .unwrap();

        // M5 candles: open at 0, 300_000, 600_000
        let candles = vec![
            make_candle(0, 100.0),
            make_candle(300_000, 101.0),
            make_candle(600_000, 102.0),
        ];
        // Trades at 150_100ms and 150_120ms — inside the first M5 candle
        let trades = vec![
            make_trade(150_100, 100.0, 30.0, Side::Buy),
            make_trade(150_120, 100.0, 30.0, Side::Buy),
        ];

        let input = StudyInput {
            candles: &candles,
            trades: Some(&trades),
            basis: ChartBasis::Time(Timeframe::M5),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        };
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Markers(_)),
            "Expected Markers"
        );
        let StudyOutput::Markers(md) = output else {
            unreachable!()
        };
        let m = &md.markers;
        assert_eq!(m.len(), 1);
        // Marker time should snap to candle open (0),
        // not the raw mid_time (150_150)
        assert_eq!(
            m[0].time, 0,
            "marker time {} should snap to candle open 0",
            m[0].time
        );
    }

    #[test]
    fn test_time_based_marker_snaps_to_correct_candle() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("filter_min", ParameterValue::Integer(1))
            .unwrap();

        // M5 candles
        let candles = vec![
            make_candle(0, 100.0),
            make_candle(300_000, 101.0),
            make_candle(600_000, 102.0),
        ];
        // Trades in the second candle (300_000..600_000)
        let trades = vec![make_trade(450_000, 101.0, 50.0, Side::Sell)];

        let input = StudyInput {
            candles: &candles,
            trades: Some(&trades),
            basis: ChartBasis::Time(Timeframe::M5),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        };
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Markers(_)),
            "Expected Markers"
        );
        let StudyOutput::Markers(md) = output else {
            unreachable!()
        };
        let m = &md.markers;
        assert_eq!(m.len(), 1);
        assert_eq!(
            m[0].time, 300_000,
            "marker time {} should snap to candle open 300000",
            m[0].time
        );
    }

    #[test]
    fn test_tick_based_marker_uses_candle_index() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("filter_min", ParameterValue::Integer(1))
            .unwrap();

        // 3 tick candles
        let candles = vec![
            make_candle(1000, 100.0),
            make_candle(2000, 101.0),
            make_candle(3000, 102.0),
        ];
        // Trade in the middle candle
        let trades = vec![make_trade(2500, 101.0, 50.0, Side::Buy)];

        let input = StudyInput {
            candles: &candles,
            trades: Some(&trades),
            basis: ChartBasis::Tick(100),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        };
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Markers(_)),
            "Expected Markers"
        );
        let StudyOutput::Markers(md) = output else {
            unreachable!()
        };
        let m = &md.markers;
        assert_eq!(m.len(), 1);
        // Candle index 1 (middle), reverse index = 2 - 1 = 1
        assert_eq!(
            m[0].time, 1,
            "marker time {} should be reverse candle index 1",
            m[0].time
        );
    }

    #[test]
    fn test_time_based_candle_boundary_splits_block() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("filter_min", ParameterValue::Integer(1))
            .unwrap();
        // Use a long aggregation window so only the candle boundary
        // causes the split, not the time gap.
        study
            .set_parameter("aggregation_window_ms", ParameterValue::Integer(200))
            .unwrap();

        // Two M5 candles
        let candles = vec![make_candle(0, 100.0), make_candle(300_000, 101.0)];
        // Two same-side trades 50ms apart but straddling the candle
        // boundary (299_980 in candle 0, 300_030 in candle 1).
        let trades = vec![
            make_trade(299_980, 100.0, 30.0, Side::Buy),
            make_trade(300_030, 100.0, 30.0, Side::Buy),
        ];

        let input = StudyInput {
            candles: &candles,
            trades: Some(&trades),
            basis: ChartBasis::Time(Timeframe::M5),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        };
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Markers(_)),
            "Expected Markers"
        );
        let StudyOutput::Markers(md) = output else {
            unreachable!()
        };
        let m = &md.markers;
        assert_eq!(
            m.len(),
            2,
            "trades crossing a candle boundary should produce \
             separate markers, got {}",
            m.len()
        );
        assert_eq!(m[0].time, 0);
        assert_eq!(m[1].time, 300_000);
    }

    #[test]
    fn test_tick_based_no_candle_boundary_restriction() {
        // Tick charts should NOT split on candle boundaries since the
        // x-mapping already handles index assignment independently.
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("filter_min", ParameterValue::Integer(1))
            .unwrap();
        study
            .set_parameter("aggregation_window_ms", ParameterValue::Integer(400))
            .unwrap();

        let candles = vec![make_candle(1000, 100.0), make_candle(2000, 101.0)];
        // Two same-side trades straddling tick candle boundary
        // (200ms apart, well within the 400ms window)
        let trades = vec![
            make_trade(1500, 100.0, 30.0, Side::Buy),
            make_trade(1700, 100.0, 30.0, Side::Buy),
        ];

        let input = StudyInput {
            candles: &candles,
            trades: Some(&trades),
            basis: ChartBasis::Tick(100),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        };
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Markers(_)),
            "Expected Markers"
        );
        let StudyOutput::Markers(md) = output else {
            unreachable!()
        };
        let m = &md.markers;
        // Should merge into a single marker on tick charts
        assert_eq!(
            m.len(),
            1,
            "tick charts should not split on candle boundaries, \
             got {} markers",
            m.len()
        );
        assert!(
            (m[0].contracts - 60.0).abs() < f64::EPSILON,
            "contracts: {}",
            m[0].contracts
        );
    }

    #[test]
    fn test_filter_max_excludes_large_trades() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("filter_min", ParameterValue::Integer(1))
            .unwrap();
        study
            .set_parameter("filter_max", ParameterValue::Integer(50))
            .unwrap();

        let candles = vec![make_candle(1000, 100.0)];
        // One trade below max (30), one above max (60)
        let trades = vec![
            make_trade(1000, 100.0, 30.0, Side::Buy),
            make_trade(2000, 100.0, 60.0, Side::Sell),
        ];
        study.compute(&study_input(&candles, &trades)).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Markers(_)),
            "Expected Markers"
        );
        let StudyOutput::Markers(md) = output else {
            unreachable!()
        };
        let m = &md.markers;
        assert_eq!(m.len(), 1, "filter_max should exclude 60-lot trade");
        assert!(
            (m[0].contracts - 30.0).abs() < f64::EPSILON,
            "contracts: {}",
            m[0].contracts
        );
    }

    #[test]
    fn test_filter_max_zero_means_no_upper_limit() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("filter_min", ParameterValue::Integer(1))
            .unwrap();
        // filter_max=0 means no upper limit (default)

        let candles = vec![make_candle(1000, 100.0)];
        let trades = vec![make_trade(1000, 100.0, 10000.0, Side::Buy)];
        study.compute(&study_input(&candles, &trades)).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Markers(_)),
            "Expected Markers with no upper filter"
        );
    }

    #[test]
    fn test_marker_render_config() {
        let study = BigTradesStudy::new();
        let config = study.build_marker_render_config();
        assert_eq!(config.shape, MarkerShape::Circle);
        assert!(!config.hollow);
        assert!(config.show_text);
        // Default filter_min=50 → scale_min=50
        assert!((config.scale_min - 50.0).abs() < f64::EPSILON);
        // No markers yet → fallback 10x filter_min
        assert!((config.scale_max - 500.0).abs() < f64::EPSILON);
        assert!((config.min_size - 8.0).abs() < f32::EPSILON);
        assert!((config.max_size - 36.0).abs() < f32::EPSILON);
        assert!((config.min_opacity - 0.10).abs() < f32::EPSILON);
        assert!((config.max_opacity - 0.60).abs() < f32::EPSILON);
    }

    // ── Absorption tests ─────────────────────────────────────────

    /// Helper to create a study with absorption-friendly defaults.
    fn absorption_study() -> BigTradesStudy {
        let mut study = BigTradesStudy::new();
        // Low filter so our test blocks qualify
        study
            .set_parameter("filter_min", ParameterValue::Integer(10))
            .unwrap();
        study
            .set_parameter("show_absorption", ParameterValue::Boolean(true))
            .unwrap();
        // 4 tick max range, 3 tick confirm, 6 tick break, 5s timeout
        study
            .set_parameter("absorption_max_range_ticks", ParameterValue::Integer(4))
            .unwrap();
        study
            .set_parameter("absorption_confirmation_ticks", ParameterValue::Integer(3))
            .unwrap();
        study
            .set_parameter("absorption_break_ticks", ParameterValue::Integer(6))
            .unwrap();
        study
            .set_parameter("absorption_timeout_ms", ParameterValue::Integer(5000))
            .unwrap();
        study
    }

    fn absorption_input<'a>(candles: &'a [Candle], trades: &'a [Trade]) -> StudyInput<'a> {
        StudyInput {
            candles,
            trades: Some(trades),
            basis: ChartBasis::Time(Timeframe::M1),
            // ES tick size = 0.25
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        }
    }

    fn cross_markers(study: &BigTradesStudy) -> Vec<&TradeMarker> {
        match study.output() {
            StudyOutput::Markers(md) => md
                .markers
                .iter()
                .filter(|m| m.shape_override == Some(MarkerShape::Cross))
                .collect(),
            _ => vec![],
        }
    }

    #[test]
    fn test_absorption_confirmed_cross_marker() {
        let mut study = absorption_study();
        let candles = vec![make_candle(1000, 5000.0)];
        // 1. Big sell block at 5000.00 (tight range — single price)
        // 2. Price reverses UP by 3 ticks (0.75) = confirmed
        let mut trades = vec![];
        // 50 sell fills at 5000.00
        for i in 0..50 {
            trades.push(make_trade(1000 + i * 5, 5000.0, 1.0, Side::Sell));
        }
        // Then a buy at 5000.00 to flush the sell block
        trades.push(make_trade(1300, 5000.0, 1.0, Side::Buy));
        // Price rises 3 ticks (0.75) — confirms absorption
        trades.push(make_trade(1400, 5000.75, 1.0, Side::Buy));

        study.compute(&absorption_input(&candles, &trades)).unwrap();

        let crosses = cross_markers(&study);
        assert_eq!(
            crosses.len(),
            1,
            "expected 1 cross marker, got {}",
            crosses.len()
        );
        assert_eq!(crosses[0].shape_override, Some(MarkerShape::Cross));
        // Sell aggression absorbed → bullish → marker is_buy=true
        assert!(crosses[0].is_buy);
    }

    #[test]
    fn test_absorption_wide_range_rejected() {
        let mut study = absorption_study();
        let candles = vec![make_candle(1000, 5000.0)];
        // Sell block spanning 5 ticks (1.25) — exceeds 4-tick max
        let mut trades = vec![];
        for i in 0..25 {
            trades.push(make_trade(1000 + i * 5, 5000.0, 1.0, Side::Sell));
        }
        for i in 0..25 {
            trades.push(make_trade(
                1130 + i * 5,
                5001.25, // 5 ticks above base
                1.0,
                Side::Sell,
            ));
        }
        // Flush + reversal attempt
        trades.push(make_trade(1400, 5001.25, 1.0, Side::Buy));
        trades.push(make_trade(1500, 5002.00, 1.0, Side::Buy));

        study.compute(&absorption_input(&candles, &trades)).unwrap();

        let crosses = cross_markers(&study);
        assert!(
            crosses.is_empty(),
            "wide range should not produce absorption, got {}",
            crosses.len()
        );
    }

    #[test]
    fn test_absorption_break_discards() {
        let mut study = absorption_study();
        let candles = vec![make_candle(1000, 5000.0)];
        // Sell block at 5000.00, then price continues DOWN 6 ticks
        let mut trades = vec![];
        for i in 0..50 {
            trades.push(make_trade(1000 + i * 5, 5000.0, 1.0, Side::Sell));
        }
        // Flush block
        trades.push(make_trade(1300, 5000.0, 1.0, Side::Buy));
        // Price drops 6 ticks (1.50) — breaks absorption
        trades.push(make_trade(1400, 4998.50, 1.0, Side::Buy));

        study.compute(&absorption_input(&candles, &trades)).unwrap();

        let crosses = cross_markers(&study);
        assert!(
            crosses.is_empty(),
            "break should discard absorption, got {}",
            crosses.len()
        );
    }

    #[test]
    fn test_absorption_timeout_discards() {
        let mut study = absorption_study();
        let candles = vec![make_candle(1000, 5000.0)];
        // Sell block, then no meaningful price action for 5001ms
        let mut trades = vec![];
        for i in 0..50 {
            trades.push(make_trade(1000 + i * 5, 5000.0, 1.0, Side::Sell));
        }
        // Flush block
        trades.push(make_trade(1300, 5000.0, 1.0, Side::Buy));
        // Trade after timeout (5001ms later) — reversal is too late
        trades.push(make_trade(6302, 5000.75, 1.0, Side::Buy));

        study.compute(&absorption_input(&candles, &trades)).unwrap();

        let crosses = cross_markers(&study);
        assert!(
            crosses.is_empty(),
            "timeout should discard absorption, got {}",
            crosses.len()
        );
    }

    #[test]
    fn test_absorption_disabled() {
        let mut study = absorption_study();
        study
            .set_parameter("show_absorption", ParameterValue::Boolean(false))
            .unwrap();
        let candles = vec![make_candle(1000, 5000.0)];
        // Same trades that would normally produce absorption
        let mut trades = vec![];
        for i in 0..50 {
            trades.push(make_trade(1000 + i * 5, 5000.0, 1.0, Side::Sell));
        }
        trades.push(make_trade(1300, 5000.0, 1.0, Side::Buy));
        trades.push(make_trade(1400, 5000.75, 1.0, Side::Buy));

        study.compute(&absorption_input(&candles, &trades)).unwrap();

        let crosses = cross_markers(&study);
        assert!(
            crosses.is_empty(),
            "absorption disabled should produce no crosses"
        );
    }

    #[test]
    fn test_absorption_incremental_append() {
        let mut study = absorption_study();
        let candles = vec![make_candle(1000, 5000.0)];

        // Phase 1: sell block
        let mut trades1 = vec![];
        for i in 0..50 {
            trades1.push(make_trade(1000 + i * 5, 5000.0, 1.0, Side::Sell));
        }
        study
            .compute(&absorption_input(&candles, &trades1))
            .unwrap();
        assert!(cross_markers(&study).is_empty());

        // Phase 2: flush + reversal via append
        let mut trades2 = trades1.clone();
        trades2.push(make_trade(1300, 5000.0, 1.0, Side::Buy));
        trades2.push(make_trade(1400, 5000.75, 1.0, Side::Buy));

        let input = absorption_input(&candles, &trades2);
        study.append_trades(&trades2[50..], &input).unwrap();

        let crosses = cross_markers(&study);
        assert_eq!(crosses.len(), 1, "incremental should produce absorption");
    }

    #[test]
    fn test_absorption_color_semantics() {
        let mut study = absorption_study();
        // Set distinct colors
        let buy_abs_color = SerializableColor::new(0.0, 1.0, 0.0, 1.0);
        let sell_abs_color = SerializableColor::new(1.0, 0.0, 0.0, 1.0);
        study
            .set_parameter("absorption_buy_color", ParameterValue::Color(buy_abs_color))
            .unwrap();
        study
            .set_parameter(
                "absorption_sell_color",
                ParameterValue::Color(sell_abs_color),
            )
            .unwrap();

        let candles = vec![make_candle(1000, 5000.0)];

        // Sell aggression absorbed → bullish → buy_abs_color
        let mut trades = vec![];
        for i in 0..50 {
            trades.push(make_trade(1000 + i * 5, 5000.0, 1.0, Side::Sell));
        }
        trades.push(make_trade(1300, 5000.0, 1.0, Side::Buy));
        trades.push(make_trade(1400, 5000.75, 1.0, Side::Buy));

        study.compute(&absorption_input(&candles, &trades)).unwrap();
        let crosses = cross_markers(&study);
        assert_eq!(crosses.len(), 1);
        assert_eq!(
            crosses[0].color, buy_abs_color,
            "sell aggression absorbed should use buy color"
        );

        // Buy aggression absorbed → bearish → sell_abs_color
        let mut trades2 = vec![];
        for i in 0..50 {
            trades2.push(make_trade(2000 + i * 5, 5000.0, 1.0, Side::Buy));
        }
        trades2.push(make_trade(2300, 5000.0, 1.0, Side::Sell));
        // Price drops 3 ticks = confirms buy absorption
        trades2.push(make_trade(2400, 4999.25, 1.0, Side::Sell));

        study
            .compute(&absorption_input(&candles, &trades2))
            .unwrap();
        let crosses = cross_markers(&study);
        assert_eq!(crosses.len(), 1);
        assert_eq!(
            crosses[0].color, sell_abs_color,
            "buy aggression absorbed should use sell color"
        );
    }
}
