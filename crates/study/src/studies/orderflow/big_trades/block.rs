//! Trade block aggregation and flushing.
//!
//! A [`TradeBlock`] accumulates consecutive same-side fills within an
//! aggregation window. When the window breaks (side change, time gap,
//! or candle boundary), the block is flushed into a [`TradeMarker`]
//! if it passes the min/max contract filter.

use crate::output::{TradeMarker, TradeMarkerDebug};
use data::{Candle, ChartBasis};

use super::params::ComputeParams;

/// Format contract count for display.
pub(crate) fn format_contracts(contracts: f64) -> String {
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
pub(super) struct TradeBlock {
    /// `true` for buy-side (aggressor) fills, `false` for sell-side.
    pub is_buy: bool,
    /// Running sum of `price_units * qty` for VWAP computation.
    pub vwap_numerator: f64,
    /// Running sum of quantities across all fills in this block.
    pub total_qty: f64,
    /// Timestamp of the first fill in the block.
    pub first_time: u64,
    /// Timestamp of the most recent fill in the block.
    pub last_time: u64,
    /// Number of individual fills merged into this block.
    pub fill_count: u32,
    /// Lowest price (in i64 units) seen across fills.
    pub min_price_units: i64,
    /// Highest price (in i64 units) seen across fills.
    pub max_price_units: i64,
    /// Containing candle's open time (time-based charts only, 0 otherwise).
    pub candle_open: u64,
    /// Price of the first fill in the block (i64 units), for impact
    /// estimation.
    pub first_price_units: i64,
}

impl TradeBlock {
    /// Start a new block from a single fill.
    pub fn new(is_buy: bool, price_units: i64, qty: f64, time: u64, candle_open: u64) -> Self {
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
            first_price_units: price_units,
        }
    }

    /// Compute the VWAP in i64 price units.
    pub fn vwap_units(&self) -> i64 {
        if self.total_qty > 0.0 {
            (self.vwap_numerator / self.total_qty).round() as i64
        } else {
            0
        }
    }

    /// Midpoint timestamp between the first and last fill.
    pub fn mid_time(&self) -> u64 {
        (self.first_time + self.last_time) / 2
    }
}

/// Build candle boundary lookup table for tick-based charts.
///
/// Returns `Some(vec)` of `(start_time, end_time)` pairs for tick charts,
/// `None` for time-based charts.
pub(super) fn build_candle_boundaries(
    candles: &[Candle],
    basis: &ChartBasis,
) -> Option<Vec<(u64, u64)>> {
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

/// Flush a completed [`TradeBlock`] into a [`TradeMarker`] if it passes
/// the min/max contract filter.
pub(super) fn flush_block(
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
                    (bounds.len().saturating_sub(1) - idx) as u64
                }
            } else {
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
