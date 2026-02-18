//! MACD Indicator

use crate::chart::{Caches, Message, ViewState, indicator::{indicator_row, kline::KlineIndicatorImpl, plot::{PlotTooltip, line::LinePlot}}};
use data::{Candle, ChartBasis};
use iced::widget::column;
use std::{collections::BTreeMap, ops::RangeInclusive};

pub struct MacdIndicator {
    macd_line: BTreeMap<u64, f32>,
    signal_line: BTreeMap<u64, f32>,
    cache: Caches,
}

impl MacdIndicator {
    pub fn new() -> Self {
        Self {
            macd_line: BTreeMap::new(),
            signal_line: BTreeMap::new(),
            cache: Caches::default(),
        }
    }

    fn calculate(&mut self, candles: &[Candle], basis: ChartBasis) {
        self.macd_line.clear();
        self.signal_line.clear();
        if candles.len() < 26 { return; }

        let multiplier_12 = 2.0 / 13.0;
        let multiplier_26 = 2.0 / 27.0;
        let mut ema12 = candles[0].close.to_f32();
        let mut ema26 = candles[0].close.to_f32();
        let mut macd_values = Vec::new();

        for (i, candle) in candles.iter().enumerate() {
            let close = candle.close.to_f32();
            ema12 = close * multiplier_12 + ema12 * (1.0 - multiplier_12);
            ema26 = close * multiplier_26 + ema26 * (1.0 - multiplier_26);
            let macd = ema12 - ema26;

            let key = match basis {
                ChartBasis::Time(_) => candle.time.0,
                ChartBasis::Tick(_) => (candles.len() - 1 - i) as u64,
            };
            macd_values.push((key, macd));
            self.macd_line.insert(key, macd);
        }

        if macd_values.len() < 9 { return; }
        let multiplier_9 = 2.0 / 10.0;
        let mut signal = macd_values[0].1;

        for (key, macd) in &macd_values {
            signal = macd * multiplier_9 + signal * (1.0 - multiplier_9);
            self.signal_line.insert(*key, signal);
        }
    }
}

impl KlineIndicatorImpl for MacdIndicator {
    fn clear_all_caches(&mut self) { self.cache.clear_all(); }
    fn clear_crosshair_caches(&mut self) { self.cache.clear_crosshair(); }

    fn element<'a>(&'a self, chart: &'a ViewState, visible_range: RangeInclusive<u64>) -> iced::Element<'a, Message> {
        let tooltip_macd = |v: &f32, _n: Option<&f32>| PlotTooltip::new(format!("MACD: {:.2}", v));
        let tooltip_signal = |v: &f32, _n: Option<&f32>| PlotTooltip::new(format!("Signal: {:.2}", v));

        let macd_plot = LinePlot::new(|v: &f32| *v).stroke_width(1.5).show_points(false).with_tooltip(tooltip_macd);
        let signal_plot = LinePlot::new(|v: &f32| *v).stroke_width(1.5).show_points(false).with_tooltip(tooltip_signal);

        column![
            indicator_row(chart, &self.cache, macd_plot, &self.macd_line, visible_range.clone()),
            indicator_row(chart, &self.cache, signal_plot, &self.signal_line, visible_range)
        ].into()
    }

    fn rebuild_from_candles(&mut self, candles: &[Candle], basis: ChartBasis) {
        self.calculate(candles, basis);
        self.clear_all_caches();
    }
}

impl Default for MacdIndicator {
    fn default() -> Self { Self::new() }
}
