//! Relative Strength Index (RSI) Indicator

use crate::chart::{Caches, Message, ViewState, indicator::{indicator_row, kline::KlineIndicatorImpl, plot::{PlotTooltip, line::LinePlot}}};
use data::{Candle, ChartBasis};
use std::{collections::BTreeMap, ops::RangeInclusive};

pub struct RsiIndicator {
    period: usize,
    data: BTreeMap<u64, f32>,
    cache: Caches,
}

impl RsiIndicator {
    pub fn new(period: usize) -> Self {
        Self { period, data: BTreeMap::new(), cache: Caches::default() }
    }

    fn calculate(&mut self, candles: &[Candle], basis: ChartBasis) {
        self.data.clear();
        if candles.len() < self.period + 1 { return; }

        let mut gains = Vec::new();
        let mut losses = Vec::new();

        for i in 1..candles.len() {
            let change = candles[i].close.to_f32() - candles[i - 1].close.to_f32();
            if change >= 0.0 {
                gains.push(change);
                losses.push(0.0);
            } else {
                gains.push(0.0);
                losses.push(-change);
            }
        }

        let mut avg_gain: f32 = gains[..self.period].iter().sum::<f32>() / self.period as f32;
        let mut avg_loss: f32 = losses[..self.period].iter().sum::<f32>() / self.period as f32;

        // Emit initial RSI from the simple average
        {
            let rs = if avg_loss == 0.0 { 100.0 } else { avg_gain / avg_loss };
            let rsi = 100.0 - (100.0 / (1.0 + rs));
            let candle_idx = self.period;
            let key = match basis {
                ChartBasis::Time(_) => candles[candle_idx].time.0,
                ChartBasis::Tick(_) => (candles.len() - 1 - candle_idx) as u64,
            };
            self.data.insert(key, rsi);
        }

        // Smoothed RSI for subsequent candles
        for i in self.period..gains.len() {
            avg_gain = (avg_gain * (self.period - 1) as f32 + gains[i]) / self.period as f32;
            avg_loss = (avg_loss * (self.period - 1) as f32 + losses[i]) / self.period as f32;

            let rs = if avg_loss == 0.0 { 100.0 } else { avg_gain / avg_loss };
            let rsi = 100.0 - (100.0 / (1.0 + rs));

            let candle_idx = i + 1;
            let key = match basis {
                ChartBasis::Time(_) => candles[candle_idx].time.0,
                ChartBasis::Tick(_) => (candles.len() - 1 - candle_idx) as u64,
            };
            self.data.insert(key, rsi);
        }
    }
}

impl KlineIndicatorImpl for RsiIndicator {
    fn clear_all_caches(&mut self) { self.cache.clear_all(); }
    fn clear_crosshair_caches(&mut self) { self.cache.clear_crosshair(); }

    fn element<'a>(&'a self, chart: &'a ViewState, visible_range: RangeInclusive<u64>) -> iced::Element<'a, Message> {
        let period = self.period;
        let tooltip = move |v: &f32, _n: Option<&f32>| PlotTooltip::new(format!("RSI({}): {:.1}", period, v));
        let plot = LinePlot::new(|v: &f32| *v).stroke_width(1.5).show_points(false).with_tooltip(tooltip);
        indicator_row(chart, &self.cache, plot, &self.data, visible_range)
    }

    fn rebuild_from_candles(&mut self, candles: &[Candle], basis: ChartBasis) {
        self.calculate(candles, basis);
        self.clear_all_caches();
    }
}
