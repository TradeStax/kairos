//! Bollinger Bands Indicator

use crate::chart::{
    Caches, Message, ViewState,
    indicator::{
        indicator_row,
        kline::{KlineIndicatorImpl, OverlayLine},
        plot::{PlotTooltip, line::LinePlot},
    },
};
use data::{Candle, ChartBasis};
use iced::widget::column;
use std::{collections::BTreeMap, ops::RangeInclusive};

/// Band colors
const UPPER_COLOR: iced::Color = iced::Color {
    r: 0.6, g: 0.6, b: 0.8, a: 0.7,
};
const MIDDLE_COLOR: iced::Color = iced::Color {
    r: 0.5, g: 0.5, b: 0.7, a: 0.9,
};
const LOWER_COLOR: iced::Color = iced::Color {
    r: 0.6, g: 0.6, b: 0.8, a: 0.7,
};

pub struct BollingerIndicator {
    upper_band: BTreeMap<u64, f32>,
    middle_band: BTreeMap<u64, f32>,
    lower_band: BTreeMap<u64, f32>,
    cache: Caches,
}

impl BollingerIndicator {
    pub fn new() -> Self {
        Self {
            upper_band: BTreeMap::new(),
            middle_band: BTreeMap::new(),
            lower_band: BTreeMap::new(),
            cache: Caches::default(),
        }
    }

    fn calculate(&mut self, candles: &[Candle], basis: ChartBasis) {
        self.upper_band.clear();
        self.middle_band.clear();
        self.lower_band.clear();

        let period = 20;
        let std_dev_multiplier = 2.0;
        if candles.len() < period { return; }

        for i in period - 1..candles.len() {
            let window = &candles[i - (period - 1)..=i];
            let sum: f32 = window.iter().map(|c| c.close.to_f32()).sum();
            let sma = sum / period as f32;

            let variance: f32 = window.iter().map(|c| {
                let diff = c.close.to_f32() - sma;
                diff * diff
            }).sum::<f32>() / period as f32;

            let std_dev = variance.sqrt();
            let key = match basis {
                ChartBasis::Time(_) => candles[i].time.0,
                ChartBasis::Tick(_) => (candles.len() - 1 - i) as u64,
            };
            self.middle_band.insert(key, sma);
            self.upper_band.insert(key, sma + std_dev_multiplier * std_dev);
            self.lower_band.insert(key, sma - std_dev_multiplier * std_dev);
        }
    }
}

impl KlineIndicatorImpl for BollingerIndicator {
    fn clear_all_caches(&mut self) { self.cache.clear_all(); }
    fn clear_crosshair_caches(&mut self) { self.cache.clear_crosshair(); }

    fn element<'a>(
        &'a self,
        chart: &'a ViewState,
        visible_range: RangeInclusive<u64>,
    ) -> iced::Element<'a, Message> {
        // Bollinger is an overlay indicator; fallback panel view.
        let tooltip = |v: &f32, _n: Option<&f32>| {
            PlotTooltip::new(format!("BB: {:.2}", v))
        };

        let upper_plot = LinePlot::new(|v: &f32| *v)
            .stroke_width(1.0).show_points(false).with_tooltip(tooltip);
        let middle_plot = LinePlot::new(|v: &f32| *v)
            .stroke_width(1.0).show_points(false).with_tooltip(tooltip);
        let lower_plot = LinePlot::new(|v: &f32| *v)
            .stroke_width(1.0).show_points(false).with_tooltip(tooltip);

        column![
            indicator_row(
                chart, &self.cache, upper_plot,
                &self.upper_band, visible_range.clone(),
            ),
            indicator_row(
                chart, &self.cache, middle_plot,
                &self.middle_band, visible_range.clone(),
            ),
            indicator_row(
                chart, &self.cache, lower_plot,
                &self.lower_band, visible_range,
            )
        ]
        .into()
    }

    fn rebuild_from_candles(&mut self, candles: &[Candle], basis: ChartBasis) {
        self.calculate(candles, basis);
        self.clear_all_caches();
    }

    fn overlay_lines(&self) -> Vec<OverlayLine<'_>> {
        vec![
            OverlayLine {
                data: &self.upper_band,
                color: UPPER_COLOR,
                stroke_width: 1.0,
            },
            OverlayLine {
                data: &self.middle_band,
                color: MIDDLE_COLOR,
                stroke_width: 1.0,
            },
            OverlayLine {
                data: &self.lower_band,
                color: LOWER_COLOR,
                stroke_width: 1.0,
            },
        ]
    }
}

impl Default for BollingerIndicator {
    fn default() -> Self { Self::new() }
}
