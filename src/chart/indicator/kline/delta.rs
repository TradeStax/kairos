//! Delta Indicator - Shows buy volume minus sell volume per candle

use crate::chart::{
    Caches, Message, ViewState,
    indicator::{
        indicator_row,
        kline::KlineIndicatorImpl,
        plot::{
            PlotTooltip,
            bar::{BarClass, BarPlot},
        },
    },
};

use data::{Candle, ChartBasis};
use data::util::format_with_commas;

use std::collections::BTreeMap;
use std::ops::RangeInclusive;

pub struct DeltaIndicator {
    cache: Caches,
    data: BTreeMap<u64, f32>, // timestamp -> delta
}

impl DeltaIndicator {
    pub fn new() -> Self {
        Self {
            cache: Caches::default(),
            data: BTreeMap::new(),
        }
    }
}

impl KlineIndicatorImpl for DeltaIndicator {
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
        let tooltip = |&delta: &f32, _next: Option<&f32>| {
            PlotTooltip::new(format!("Delta: {}", format_with_commas(delta)))
        };

        // Use Overlay variant - the sign of overlay determines color (positive=success, negative=danger)
        let bar_kind = |&delta: &f32| BarClass::Overlay { overlay: delta };

        let value_fn = |&delta: &f32| delta.abs();

        let plot = BarPlot::new(value_fn, bar_kind)
            .bar_width_factor(0.9)
            .with_tooltip(tooltip);

        indicator_row(chart, &self.cache, plot, &self.data, visible_range)
    }

    fn rebuild_from_candles(&mut self, candles: &[Candle], basis: ChartBasis) {
        self.data.clear();
        match basis {
            ChartBasis::Time(_) => {
                for candle in candles {
                    self.data.insert(candle.time.0, candle.volume_delta() as f32);
                }
            }
            ChartBasis::Tick(_) => {
                // For tick-based, store by reverse index (0 = most recent)
                for (index, candle) in candles.iter().rev().enumerate() {
                    self.data.insert(index as u64, candle.volume_delta() as f32);
                }
            }
        }
        self.clear_all_caches();
    }
}
