use super::ProfileChart;
use super::apply_profile_config_to_study;
use super::compute_initial_price_scale;
use crate::screen::dashboard::pane::config::ProfileConfig;
use data::FuturesTickerInfo;
use data::util::count_decimals;
use data::{Autoscale, ChartBasis, ChartData, Timeframe, ViewConfig};
use data::{Price, PriceStep};
use iced::widget::canvas::Cache;
use std::time::Instant;
use study::studies::orderflow::VbpStudy;

impl ProfileChart {
    /// Create a new ProfileChart from loaded chart data.
    pub fn from_chart_data(
        chart_data: ChartData,
        ticker_info: FuturesTickerInfo,
        layout: ViewConfig,
        config: ProfileConfig,
    ) -> Self {
        let step = PriceStep::from_f32(ticker_info.tick_size);
        let basis = ChartBasis::Time(Timeframe::M5);

        // Compute initial cell_height from price range
        let (_, _, cell_height) =
            compute_initial_price_scale(&chart_data.candles, ticker_info.tick_size);

        let base_price_y = chart_data
            .candles
            .iter()
            .map(|c| c.high)
            .max()
            .map(|p| Price::from_units(p.units()))
            .unwrap_or(Price::from_f32(0.0));

        let default_cell_width = 4.0;
        let latest_x = chart_data.candles.last().map(|c| c.time.0).unwrap_or(0);

        let mut chart = crate::chart::ViewState::new(
            basis,
            step,
            count_decimals(ticker_info.tick_size),
            ticker_info,
            ViewConfig {
                splits: layout.splits,
                autoscale: Some(Autoscale::FitAll),
                side_splits: layout.side_splits,
            },
            default_cell_width,
            cell_height,
        );
        chart.base_price_y = base_price_y;
        chart.latest_x = latest_x;

        let x_translation =
            0.5 * (chart.bounds.width / chart.scaling) - (8.0 * chart.cell_width / chart.scaling);
        chart.translation.x = x_translation;
        chart.translation.y = -chart.bounds.height / 2.0;

        let mut profile_study = VbpStudy::new();
        apply_profile_config_to_study(&mut profile_study, &config, &ticker_info);

        let mut profile = ProfileChart {
            chart,
            chart_data,
            basis,
            ticker_info,
            last_tick: Instant::now(),
            drawings: crate::chart::drawing::DrawingManager::new(),
            profile_study,
            fingerprint: (0, 0, 0, 0),
            display_config: config,
            studies: Vec::new(),
            studies_dirty: false,
            last_visible_range: None,
            panel_cache: Cache::default(),
            panel_labels_cache: Cache::default(),
            panel_crosshair_cache: Cache::default(),
        };
        profile.recompute_profile();
        profile
    }
}
