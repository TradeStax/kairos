//! Exponential Moving Average (EMA) Indicator

use crate::chart::{
    Caches, Message, ViewState,
    indicator::{
        indicator_row,
        kline::{KlineIndicatorImpl, OverlayLine},
        plot::{PlotTooltip, line::LinePlot},
    },
};
use data::{Candle, ChartBasis};
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

    fn calculate(&mut self, candles: &[Candle], basis: ChartBasis) {
        self.data.clear();
        if candles.is_empty() { return; }

        let multiplier = 2.0 / (self.period as f32 + 1.0);
        let mut ema = candles[0].close.to_f32();

        for (i, candle) in candles.iter().enumerate() {
            let close = candle.close.to_f32();
            ema = close * multiplier + ema * (1.0 - multiplier);

            let key = match basis {
                ChartBasis::Time(_) => candle.time.0,
                ChartBasis::Tick(_) => (candles.len() - 1 - i) as u64,
            };
            self.data.insert(key, ema);
        }
    }

    fn get_color(&self) -> iced::Color {
        match self.period {
            9 => iced::Color::from_rgb(0.3, 1.0, 0.3),   // Green
            21 => iced::Color::from_rgb(1.0, 0.3, 0.3),   // Red
            _ => iced::Color::from_rgb(0.5, 0.5, 0.5),    // Gray
        }
    }
}

impl KlineIndicatorImpl for EmaIndicator {
    fn clear_all_caches(&mut self) { self.cache.clear_all(); }
    fn clear_crosshair_caches(&mut self) { self.cache.clear_crosshair(); }

    fn element<'a>(
        &'a self,
        chart: &'a ViewState,
        visible_range: RangeInclusive<u64>,
    ) -> iced::Element<'a, Message> {
        // EMA is an overlay indicator; fallback panel view.
        let period = self.period;
        let tooltip = move |v: &f32, _n: Option<&f32>| {
            PlotTooltip::new(format!("EMA({}): {:.2}", period, v))
        };
        let plot = LinePlot::new(|v: &f32| *v)
            .stroke_width(1.5)
            .show_points(false)
            .with_tooltip(tooltip);
        indicator_row(chart, &self.cache, plot, &self.data, visible_range)
    }

    fn rebuild_from_candles(&mut self, candles: &[Candle], basis: ChartBasis) {
        self.calculate(candles, basis);
        self.clear_all_caches();
    }

    fn overlay_lines(&self) -> Vec<OverlayLine<'_>> {
        vec![OverlayLine {
            data: &self.data,
            color: self.get_color(),
            stroke_width: 1.5,
        }]
    }
}
