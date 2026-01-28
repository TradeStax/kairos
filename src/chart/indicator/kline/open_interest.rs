use crate::chart::{
    Caches, Message, ViewState,
    indicator::{
        indicator_row,
        kline::KlineIndicatorImpl,
        plot::{PlotTooltip, line::LinePlot},
    },
};

use data::{Candle, ChartBasis, Timeframe};
use data::util::format_with_commas;

use iced::widget::{center, row, text};
use std::{collections::BTreeMap, ops::RangeInclusive};

pub struct OpenInterestIndicator {
    cache: Caches,
    pub data: BTreeMap<u64, f32>, // timestamp -> open interest value
}

impl OpenInterestIndicator {
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
        match main_chart.basis {
            ChartBasis::Time(timeframe) => {
                // Check timeframe support (Databento provides OI for most timeframes)
                if !Self::is_supported_timeframe(timeframe) {
                    return center(text(format!(
                        "Open Interest is not available on {:?} timeframe",
                        timeframe
                    )))
                    .into();
                }

                let (earliest, latest) = visible_range.clone().into_inner();
                if latest < earliest {
                    return row![].into();
                }
            }
            ChartBasis::Tick(_) => {
                return center(text("Open Interest is not available for tick charts.")).into();
            }
        }

        let tooltip = |value: &f32, next: Option<&f32>| {
            let value_text = format!("Open Interest: {}", format_with_commas(*value));
            let change_text = if let Some(next_value) = next {
                let delta = next_value - *value;
                let sign = if delta >= 0.0 { "+" } else { "" };
                format!("Change: {}{}", sign, format_with_commas(delta))
            } else {
                "Change: N/A".to_string()
            };
            PlotTooltip::new(format!("{value_text}\n{change_text}"))
        };

        let value_fn = |v: &f32| *v;

        let plot = LinePlot::new(value_fn)
            .stroke_width(1.0)
            .show_points(true)
            .point_radius_factor(0.2)
            .padding(0.08)
            .with_tooltip(tooltip);

        indicator_row(main_chart, &self.cache, plot, &self.data, visible_range)
    }

    pub fn is_supported_timeframe(timeframe: Timeframe) -> bool {
        // Databento provides OI for standard timeframes
        matches!(
            timeframe,
            Timeframe::M5
                | Timeframe::M15
                | Timeframe::M30
                | Timeframe::H1
                | Timeframe::H4
                | Timeframe::D1
        )
    }

    /// Set OI data (loaded from repository)
    pub fn set_data(&mut self, oi_data: Vec<(u64, f32)>) {
        self.data.clear();
        self.data.extend(oi_data);
        self.clear_all_caches();
    }
}

impl KlineIndicatorImpl for OpenInterestIndicator {
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

    fn rebuild_from_candles(&mut self, _candles: &[Candle], _basis: ChartBasis) {
        // OI is separate data, not derived from candles
        // Data is set via set_data() method when loaded from repository
        self.clear_all_caches();
    }
}
