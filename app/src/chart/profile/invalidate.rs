use super::ProfileChart;
use super::apply_profile_config_to_study;
use data::Price;
use data::{Autoscale, Price as DomainPrice};
use std::time::Instant;
use study::Study as _;
use study::core::StudyInput;
use study::output::StudyOutput;

impl ProfileChart {
    /// Rebuild the volume profile via the internal VbpStudy.
    pub(super) fn recompute_profile(&mut self) {
        let fp = (
            self.chart_data.trades.len(),
            self.chart_data
                .trades
                .first()
                .map(|t| t.time.0)
                .unwrap_or(0),
            self.chart_data.trades.last().map(|t| t.time.0).unwrap_or(0),
            self.chart_data.candles.len(),
        );
        if fp == self.fingerprint && !matches!(self.profile_study.output(), StudyOutput::Empty) {
            return;
        }
        self.fingerprint = fp;

        // Reapply config in case display_config changed
        apply_profile_config_to_study(
            &mut self.profile_study,
            &self.display_config,
            &self.ticker_info,
        );

        // Always pass all data — split mode handles segmentation
        let trades: Option<&[data::Trade]> = if !self.chart_data.trades.is_empty() {
            Some(&self.chart_data.trades)
        } else {
            None
        };
        let input = StudyInput {
            candles: &self.chart_data.candles,
            trades,
            basis: self.basis,
            tick_size: DomainPrice::from_f32(self.ticker_info.tick_size),
            visible_range: None,
        };
        if let Err(e) = self.profile_study.compute(&input) {
            log::warn!("Profile study compute error: {e}");
        }
    }

    pub fn invalidate(&mut self) {
        // Snapshot the price extremes from ALL profiles before
        // we mutably borrow `self.chart` for autoscaling.
        // In split mode there are multiple profiles covering
        // different price ranges — we need the union of all.
        let price_extremes = self.profiles_and_config().and_then(|(profiles, _)| {
            let mut highest = f32::MIN;
            let mut lowest = f32::MAX;
            for p in profiles {
                if let Some(last) = p.levels.last() {
                    highest = highest.max(last.price as f32);
                }
                if let Some(first) = p.levels.first() {
                    lowest = lowest.min(first.price as f32);
                }
            }
            if highest > lowest {
                Some((highest, lowest))
            } else {
                None
            }
        });

        let chart = &mut self.chart;

        // Fit-all autoscaling: fit price range to visible area
        if let Some(Autoscale::FitAll) = chart.layout.autoscale
            && let Some((highest, lowest)) = price_extremes
        {
            let padding = (highest - lowest) * 0.05;
            let price_span = (highest - lowest) + (2.0 * padding);

            if price_span > 0.0 && chart.bounds.height > f32::EPSILON {
                let padded_highest = highest + padding;
                let chart_height = chart.bounds.height;
                let tick_size = chart.tick_size.to_f32_lossy();

                if tick_size > 0.0 {
                    chart.cell_height = (chart_height * tick_size) / price_span;
                    chart.base_price_y = Price::from_f32(padded_highest);
                    chart.translation.y = -chart_height / 2.0;
                }
            }
        }

        chart.cache.clear_all();
        self.panel_cache.clear();
        self.panel_labels_cache.clear();
        self.panel_crosshair_cache.clear();

        // Check if visible range changed (triggers study recompute)
        if chart.bounds.width > 0.0 {
            let region = chart.visible_region(chart.bounds.size());
            let (_, _) = chart.interval_range(&region);
            let price_range = chart.price_range(&region);
            let new_range = Some((price_range.1.units() as u64, price_range.0.units() as u64));
            if new_range != self.last_visible_range {
                self.last_visible_range = new_range;
                self.studies_dirty = true;
            }
        }

        if self.studies_dirty {
            self.recompute_studies();
            self.studies_dirty = false;
        }

        self.last_tick = Instant::now();
    }
}
