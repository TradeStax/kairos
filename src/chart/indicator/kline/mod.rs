//! Kline (candlestick) indicators
//!
//! This module provides technical indicators for candlestick charts.
//!
//! Indicators come in two flavours:
//!
//! * **Overlay** indicators (SMA, EMA, Bollinger Bands) produce values in
//!   price-space and are drawn directly on the main candlestick canvas.
//! * **Panel** indicators (Volume, Delta, RSI, MACD, OI) have their own
//!   Y-axis scale and are rendered in separate sub-panels below the chart.

use crate::chart::{Message, ViewState};

use data::{Candle, ChartBasis, KlineIndicator};
use std::collections::BTreeMap;

pub mod bollinger;
pub mod delta;
pub mod ema;
pub mod macd;
pub mod open_interest;
pub mod rsi;
pub mod sma;
pub mod volume;

/// A single line to be drawn on the main chart canvas as an overlay.
pub struct OverlayLine<'a> {
    pub data: &'a BTreeMap<u64, f32>,
    pub color: iced::Color,
    pub stroke_width: f32,
}

pub trait KlineIndicatorImpl {
    /// Clear all caches for a full redraw
    fn clear_all_caches(&mut self);

    /// Clear caches related to crosshair only
    /// e.g. tooltips and scale labels for a partial redraw
    fn clear_crosshair_caches(&mut self);

    /// Return the indicator as a separate panel element.
    ///
    /// Only called for **panel** indicators (not overlays).
    fn element<'a>(
        &'a self,
        chart: &'a ViewState,
        visible_range: std::ops::RangeInclusive<u64>,
    ) -> iced::Element<'a, Message>;

    /// Rebuild indicator from candle data with chart basis
    ///
    /// For time-based charts, data is stored keyed by timestamp.
    /// For tick-based charts, data is stored keyed by reverse index (0 = most recent).
    fn rebuild_from_candles(&mut self, candles: &[Candle], basis: ChartBasis);

    /// Handle tick size changes (recalculate if needed)
    fn on_ticksize_change(&mut self, candles: &[Candle], basis: ChartBasis) {
        self.rebuild_from_candles(candles, basis);
    }

    /// Handle basis changes (recalculate if needed)
    fn on_basis_change(&mut self, candles: &[Candle], basis: ChartBasis) {
        self.rebuild_from_candles(candles, basis);
    }

    /// Return overlay line data to draw on the main chart canvas.
    ///
    /// Returns an empty vec by default (panel indicators).
    /// Overlay indicators (SMA, EMA, Bollinger) override this to provide
    /// their line data and styling.
    fn overlay_lines(&self) -> Vec<OverlayLine<'_>> {
        vec![]
    }
}

pub fn make_empty(which: KlineIndicator) -> Box<dyn KlineIndicatorImpl> {
    match which {
        KlineIndicator::Volume => Box::new(volume::VolumeIndicator::new()),
        KlineIndicator::Delta => Box::new(delta::DeltaIndicator::new()),
        KlineIndicator::OpenInterest => Box::new(open_interest::OpenInterestIndicator::new()),
        KlineIndicator::Sma20 => Box::new(sma::SmaIndicator::new(20)),
        KlineIndicator::Sma50 => Box::new(sma::SmaIndicator::new(50)),
        KlineIndicator::Sma200 => Box::new(sma::SmaIndicator::new(200)),
        KlineIndicator::Ema9 => Box::new(ema::EmaIndicator::new(9)),
        KlineIndicator::Ema21 => Box::new(ema::EmaIndicator::new(21)),
        KlineIndicator::Rsi14 => Box::new(rsi::RsiIndicator::new(14)),
        KlineIndicator::Macd => Box::new(macd::MacdIndicator::new()),
        KlineIndicator::BollingerBands => Box::new(bollinger::BollingerIndicator::new()),
    }
}
