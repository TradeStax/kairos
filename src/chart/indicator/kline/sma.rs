//! Simple Moving Average (SMA) Indicator

use crate::chart::{
    Caches, Message, ViewState,
    indicator::{
        indicator_row,
        kline::KlineIndicatorImpl,
        plot::{PlotTooltip, line::LinePlot},
    },
};

use data::Candle;
use std::{collections::BTreeMap, ops::RangeInclusive};

/// SMA Indicator with configurable period
pub struct SmaIndicator {
    period: usize,
    data: BTreeMap<u64, f32>,
    cache: Caches,
}

impl SmaIndicator {
    pub fn new(period: usize) -> Self {
        Self {
            period,
            data: BTreeMap::new(),
            cache: Caches::default(),
        }
    }

    /// Calculate SMA from candles
    fn calculate(&mut self, candles: &[Candle]) {
        self.data.clear();

        if candles.len() < self.period {
            return;
        }

        // Calculate SMA using sliding window
        for i in self.period - 1..candles.len() {
            let window = &candles[i - (self.period - 1)..=i];
            let sum: f32 = window.iter().map(|c| c.close.to_f32()).sum();
            let sma = sum / self.period as f32;
            self.data.insert(candles[i].time.0, sma);
        }
    }

    fn get_color(&self) -> iced::Color {
        match self.period {
            20 => iced::Color::from_rgb(0.2, 0.8, 1.0),   // Blue
            50 => iced::Color::from_rgb(1.0, 0.6, 0.2),   // Orange
            200 => iced::Color::from_rgb(0.8, 0.2, 0.8),  // Purple
            _ => iced::Color::from_rgb(0.5, 0.5, 0.5),    // Gray
        }
    }

    fn indicator_elem<'a>(
        &'a self,
        main_chart: &'a ViewState,
        visible_range: RangeInclusive<u64>,
    ) -> iced::Element<'a, Message> {
        let period = self.period;
        let tooltip = move |value: &f32, _next: Option<&f32>| {
            PlotTooltip::new(format!("SMA({}): {:.2}", period, value))
        };

        let value_fn = |v: &f32| *v;

        let plot = LinePlot::new(value_fn)
            .stroke_width(1.5)
            .show_points(false)
            .padding(0.05)
            .with_tooltip(tooltip);

        indicator_row(main_chart, &self.cache, plot, &self.data, visible_range)
    }
}

impl KlineIndicatorImpl for SmaIndicator {
    fn clear_all_caches(&mut self) {
        self.cache.clear_all();
    }

    fn clear_crosshair_caches(&mut self) {
        self.cache.clear_crosshair();
    }

    fn element<'a>(
        &'a self,
        chart: &'a ViewState,
        visible_range: RangeInclusive<u64>,
    ) -> iced::Element<'a, Message> {
        self.indicator_elem(chart, visible_range)
    }

    fn rebuild_from_candles(&mut self, candles: &[Candle]) {
        self.calculate(candles);
        self.clear_all_caches();
    }
}
