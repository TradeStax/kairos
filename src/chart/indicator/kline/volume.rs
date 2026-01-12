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

use data::Candle;
use data::util::format_with_commas;

use std::collections::BTreeMap;
use std::ops::RangeInclusive;

pub struct VolumeIndicator {
    cache: Caches,
    data: BTreeMap<u64, f32>, // timestamp -> volume
}

impl VolumeIndicator {
    pub fn new() -> Self {
        Self {
            cache: Caches::default(),
            data: BTreeMap::new(),
        }
    }

    fn indicator_elem<'a>(
        &'a self,
        main_chart: &'a ViewState,
        visible_range: RangeInclusive<u64>,
    ) -> iced::Element<'a, Message> {
        let tooltip = |&volume: &f32, _next: Option<&f32>| {
            PlotTooltip::new(format!("Volume: {}", format_with_commas(volume)))
        };

        let bar_kind = |_: &f32| BarClass::Single;

        let value_fn = |&volume: &f32| volume;

        let plot = BarPlot::new(value_fn, bar_kind)
            .bar_width_factor(0.9)
            .with_tooltip(tooltip);

        indicator_row(main_chart, &self.cache, plot, &self.data, visible_range)
    }
}

impl KlineIndicatorImpl for VolumeIndicator {
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
        self.data.clear();
        for candle in candles {
            self.data.insert(candle.time.0, candle.volume());
        }
        self.clear_all_caches();
    }
}
