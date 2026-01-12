//! Exponential Moving Average (EMA) Indicator

use crate::chart::{Caches, Message, ViewState, indicator::{indicator_row, kline::KlineIndicatorImpl, plot::{PlotTooltip, line::LinePlot}}};
use data::Candle;
use std::{collections::BTreeMap, ops::RangeInclusive};

pub struct EmaIndicator {
    period: usize,
    data: BTreeMap<u64, f32>,
    cache: Caches,
}

impl EmaIndicator {
    pub fn new(period: usize) -> Self {
        Self { period, data: BTreeMap::new(), cache: Caches::default() }
    }

    fn calculate(&mut self, candles: &[Candle]) {
        self.data.clear();
        if candles.is_empty() { return; }

        let multiplier = 2.0 / (self.period as f32 + 1.0);
        let mut ema = candles[0].close.to_f32();

        for candle in candles {
            let close = candle.close.to_f32();
            ema = close * multiplier + ema * (1.0 - multiplier);
            self.data.insert(candle.time.0, ema);
        }
    }

    fn get_color(&self) -> iced::Color {
        match self.period {
            9 => iced::Color::from_rgb(0.3, 1.0, 0.3),
            21 => iced::Color::from_rgb(1.0, 0.3, 0.3),
            _ => iced::Color::from_rgb(0.5, 0.5, 0.5),
        }
    }
}

impl KlineIndicatorImpl for EmaIndicator {
    fn clear_all_caches(&mut self) { self.cache.clear_all(); }
    fn clear_crosshair_caches(&mut self) { self.cache.clear_crosshair(); }

    fn element<'a>(&'a self, chart: &'a ViewState, visible_range: RangeInclusive<u64>) -> iced::Element<'a, Message> {
        let period = self.period;
        let tooltip = move |v: &f32, _n: Option<&f32>| PlotTooltip::new(format!("EMA({}): {:.2}", period, v));
        let plot = LinePlot::new(|v: &f32| *v).stroke_width(1.5).show_points(false).with_tooltip(tooltip);
        indicator_row(chart, &self.cache, plot, &self.data, visible_range)
    }

    fn rebuild_from_candles(&mut self, candles: &[Candle]) {
        self.calculate(candles);
        self.clear_all_caches();
    }
}
