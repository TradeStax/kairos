//! Kline (candlestick) indicators
//!
//! This module provides technical indicators that can be overlaid
//! on candlestick charts, such as volume, moving averages, RSI, etc.

use crate::chart::{Message, ViewState};

use data::{Candle, KlineIndicator};

pub mod bollinger;
pub mod delta;
pub mod ema;
pub mod macd;
pub mod open_interest;
pub mod rsi;
pub mod sma;
pub mod volume;

pub trait KlineIndicatorImpl {
    /// Clear all caches for a full redraw
    fn clear_all_caches(&mut self);

    /// Clear caches related to crosshair only
    /// e.g. tooltips and scale labels for a partial redraw
    fn clear_crosshair_caches(&mut self);

    fn element<'a>(
        &'a self,
        chart: &'a ViewState,
        visible_range: std::ops::RangeInclusive<u64>,
    ) -> iced::Element<'a, Message>;

    /// Rebuild indicator from candle data
    fn rebuild_from_candles(&mut self, candles: &[Candle]);

    /// Handle tick size changes (recalculate if needed)
    fn on_ticksize_change(&mut self, candles: &[Candle]) {
        self.rebuild_from_candles(candles);
    }

    /// Handle basis changes (recalculate if needed)
    fn on_basis_change(&mut self, candles: &[Candle]) {
        self.rebuild_from_candles(candles);
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
