use super::super::{Action, Content, State};
use crate::screen::dashboard::pane::config::ContentKind;
use data::{ChartBasis, ChartConfig, DataSchema, DateRange, LoadingStatus, Timeframe};

impl State {
    /// Rebuild the current chart with a specific number of days
    pub(in crate::screen::dashboard::pane) fn rebuild_chart_with_days(
        &mut self,
        days: i64,
    ) -> Option<Action> {
        let ticker_info = self.ticker_info?;
        let kind = self.content.kind();

        match kind {
            ContentKind::CandlestickChart => {}
            #[cfg(feature = "heatmap")]
            ContentKind::HeatmapChart => {}
            _ => return None,
        };

        let date_range = DateRange::last_n_days(days.max(1));

        self.content = Content::new_for_kind(kind, ticker_info, &self.settings);
        self.chart_data = None;

        let days_total = date_range.num_days() as usize;
        self.loading_status = LoadingStatus::LoadingFromCache {
            schema: DataSchema::Trades,
            days_total,
            days_loaded: 0,
            items_loaded: 0,
        };

        let basis = self
            .settings
            .selected_basis
            .unwrap_or(ChartBasis::Time(Timeframe::M5));

        Some(Action::LoadChart {
            config: ChartConfig {
                ticker: ticker_info.ticker,
                basis,
                date_range,
                chart_type: kind.to_chart_type(),
            },
            ticker_info,
        })
    }

    /// Rebuild the current chart by re-requesting data load
    pub(in crate::screen::dashboard::pane) fn rebuild_current_chart(
        &mut self,
    ) -> Option<Action> {
        let ticker_info = self.ticker_info?;
        let kind = self.content.kind();

        match kind {
            ContentKind::CandlestickChart => {}
            #[cfg(feature = "heatmap")]
            ContentKind::HeatmapChart => {}
            _ => return None,
        };

        let date_range = self
            .loaded_date_range
            .unwrap_or_else(|| DateRange::last_n_days(1));

        // Reset content to show loading screen
        self.content = Content::new_for_kind(kind, ticker_info, &self.settings);
        self.chart_data = None;

        let days_total = date_range.num_days() as usize;
        self.loading_status = LoadingStatus::LoadingFromCache {
            schema: DataSchema::Trades,
            days_total,
            days_loaded: 0,
            items_loaded: 0,
        };

        let basis = self
            .settings
            .selected_basis
            .unwrap_or(ChartBasis::Time(Timeframe::M5));

        Some(Action::LoadChart {
            config: ChartConfig {
                ticker: ticker_info.ticker,
                basis,
                date_range,
                chart_type: kind.to_chart_type(),
            },
            ticker_info,
        })
    }

    /// Center the chart view on the last price, showing ~50 bars
    pub(in crate::screen::dashboard::pane) fn center_last_price(&mut self) {
        use crate::chart::Chart;
        use crate::chart::candlestick::domain_to_exchange_price;

        // Get last candle close price from pane's chart_data
        let last_close = self
            .chart_data
            .as_ref()
            .and_then(|d| d.candles.last())
            .map(|c| domain_to_exchange_price(c.close));

        match &mut self.content {
            Content::Candlestick { chart, .. } if chart.is_some() => {
                let c = (**chart).as_mut().unwrap();
                let chart = c.mut_state();
                let x_translation = 0.5 * (chart.bounds.width / chart.scaling)
                    - (8.0 * chart.cell_width / chart.scaling);
                chart.translation.x = x_translation;

                if let Some(price) = last_close {
                    let y = chart.price_to_y(price);
                    chart.translation.y = -y;
                }

                chart.cache.clear_all();
            }
            #[cfg(feature = "heatmap")]
            Content::Heatmap { chart: Some(c), .. } => {
                use crate::chart::scale::linear::PriceInfoLabel;

                let chart = c.mut_state();
                let x_translation = 0.5 * (chart.bounds.width / chart.scaling)
                    - (8.0 * chart.cell_width / chart.scaling);
                chart.translation.x = x_translation;

                // For heatmap use last_price from ViewState, else
                // fall back to last candle close
                let price = chart
                    .last_price
                    .map(|lp| match lp {
                        PriceInfoLabel::Up(p)
                        | PriceInfoLabel::Down(p)
                        | PriceInfoLabel::Neutral(p) => p,
                    })
                    .or(last_close);

                if let Some(price) = price {
                    let y = chart.price_to_y(price);
                    chart.translation.y = -y;
                }

                chart.cache.clear_all();
            }
            _ => {}
        }
    }
}
