use super::{Action, Content, Effect, State};
use crate::chart::{candlestick::KlineChart, comparison::ComparisonChart, heatmap::HeatmapChart};
use crate::component::display::toast::Toast;
use data::{
    ChartBasis, ChartConfig, ChartData, ContentKind, DataSchema, DateRange, LoadingStatus,
    Timeframe, VisualConfig,
};
use exchange::FuturesTickerInfo;
use std::time::Instant;

impl State {
    /// Set chart data (called by dashboard after loading)
    pub fn set_chart_data(&mut self, chart_data: ChartData) {
        self.chart_data = Some(chart_data.clone());
        self.loading_status = LoadingStatus::Ready;

        let ticker_info = match self.ticker_info {
            Some(ti) => ti,
            None => return,
        };

        let basis = self
            .settings
            .selected_basis
            .unwrap_or(ChartBasis::Time(Timeframe::M15));

        match &mut self.content {
            Content::Kline {
                chart,
                indicators,
                kind,
                layout,
            } => {
                let mut new_chart = KlineChart::from_chart_data(
                    chart_data,
                    basis,
                    ticker_info,
                    layout.clone(),
                    indicators,
                    kind.clone(),
                );
                if !self.settings.drawings.is_empty() {
                    new_chart
                        .drawings
                        .load_drawings(self.settings.drawings.clone());
                }
                *chart = Some(new_chart);
            }
            Content::Heatmap {
                chart,
                indicators,
                layout,
                studies,
            } => {
                let chart_studies: Vec<crate::chart::heatmap::HeatmapStudy> = studies
                    .iter()
                    .map(|s| match s {
                        data::domain::chart_ui_types::heatmap::HeatmapStudy::VolumeProfile(
                            kind,
                        ) => crate::chart::heatmap::HeatmapStudy::VolumeProfile(*kind),
                    })
                    .collect();
                log::info!(
                    "Constructing HeatmapChart from chart_data: \
                     {} trades, {} candles, {} depth snapshots",
                    chart_data.trades.len(),
                    chart_data.candles.len(),
                    chart_data
                        .depth_snapshots
                        .as_ref()
                        .map(|d| d.len())
                        .unwrap_or(0)
                );

                let mut new_chart = HeatmapChart::from_chart_data(
                    chart_data,
                    basis,
                    ticker_info,
                    layout.clone(),
                    indicators,
                    chart_studies,
                );
                if !self.settings.drawings.is_empty() {
                    new_chart
                        .drawings
                        .load_drawings(self.settings.drawings.clone());
                }
                *chart = Some(new_chart);

                log::info!("HeatmapChart construction COMPLETE");
            }
            Content::Comparison(chart_opt) => match chart_opt {
                Some(chart) => {
                    if let Err(e) = chart.add_ticker(&ticker_info, chart_data) {
                        log::warn!("Failed to add ticker to comparison: {}", e);
                        self.notifications.push(Toast::warn(format!(
                            "Failed to add {}: {}",
                            ticker_info.ticker.as_str(),
                            e
                        )));
                    } else {
                        log::info!(
                            "Added ticker {} to comparison chart",
                            ticker_info.ticker.as_str()
                        );
                        self.loading_status = LoadingStatus::Ready;
                    }
                }
                None => {
                    let config = self.settings.visual_config.as_ref().and_then(|vc| {
                        if let VisualConfig::Comparison(cfg) = vc {
                            Some(cfg.clone())
                        } else {
                            None
                        }
                    });

                    let new_chart = ComparisonChart::from_multi_chart_data(
                        vec![(ticker_info, chart_data)],
                        basis,
                        config,
                    );
                    *chart_opt = Some(new_chart);
                    self.loading_status = LoadingStatus::Ready;
                    log::info!(
                        "Created comparison chart with ticker {}",
                        ticker_info.ticker.as_str()
                    );
                }
            },
            Content::TimeAndSales(_panel) => {}
            Content::Ladder(_panel) => {}
            Content::Starter => {}
        }
    }

    /// Set content and request chart loading with specified date range
    pub fn set_content_with_range(
        &mut self,
        ticker_info: FuturesTickerInfo,
        kind: ContentKind,
        date_range: DateRange,
    ) -> Effect {
        log::info!(
            "PANE: set_content_with_range called with {:?} \
             ContentKind::{:?}, range {} to {}",
            ticker_info.ticker,
            kind,
            date_range.start,
            date_range.end
        );

        let basis = self
            .settings
            .selected_basis
            .unwrap_or(ChartBasis::Time(Timeframe::M15));

        self.ticker_info = Some(ticker_info);
        self.content = Content::new_for_kind(kind, ticker_info, &self.settings);

        let days_total = date_range.num_days() as usize;
        self.loading_status = LoadingStatus::LoadingFromCache {
            schema: DataSchema::Trades,
            days_total,
            days_loaded: 0,
            items_loaded: 0,
        };

        let config = ChartConfig {
            ticker: ticker_info.ticker,
            basis,
            date_range,
            chart_type: kind.to_chart_type(),
        };

        Effect::LoadChart {
            config,
            ticker_info,
        }
    }

    /// Set content and request chart loading (legacy - uses default 1 day)
    pub fn set_content(&mut self, ticker_info: FuturesTickerInfo, kind: ContentKind) -> Effect {
        self.set_content_with_range(ticker_info, kind, DateRange::last_n_days(1))
    }

    pub fn invalidate(&mut self, now: Instant) -> Option<Action> {
        match &mut self.content {
            Content::Heatmap { chart, .. } => chart
                .as_mut()
                .and_then(|c| c.invalidate(Some(now)).map(Action::Chart)),
            Content::Kline { chart, .. } => {
                if let Some(c) = chart.as_mut() {
                    c.invalidate()
                }
                None
            }
            Content::TimeAndSales(panel) => panel
                .as_mut()
                .and_then(|p| p.invalidate(Some(now)).map(Action::Panel)),
            Content::Ladder(panel) => panel
                .as_mut()
                .and_then(|p| p.invalidate(Some(now)).map(Action::Panel)),
            Content::Starter => None,
            Content::Comparison(_) => None,
        }
    }

    pub fn update_interval(&self) -> Option<u64> {
        match &self.content {
            Content::Kline { .. } | Content::Comparison(_) => Some(1000),
            Content::Heatmap { chart, .. } => {
                if let Some(chart) = chart {
                    chart.basis_interval()
                } else {
                    None
                }
            }
            Content::Ladder(_) | Content::TimeAndSales(_) => Some(100),
            Content::Starter => None,
        }
    }

    pub fn last_tick(&self) -> Option<Instant> {
        self.content.last_tick()
    }

    pub fn tick(&mut self, now: Instant) -> Option<Action> {
        let invalidate_interval: Option<u64> = self.update_interval();
        let last_tick: Option<Instant> = self.last_tick();

        if !self.content.initialized() {
            return None;
        }

        match (invalidate_interval, last_tick) {
            (Some(interval_ms), Some(previous_tick_time)) => {
                if interval_ms > 0 {
                    let interval_duration = std::time::Duration::from_millis(interval_ms);
                    if now.duration_since(previous_tick_time) >= interval_duration {
                        return self.invalidate(now);
                    }
                }
            }
            (Some(interval_ms), None) => {
                if interval_ms > 0 {
                    return self.invalidate(now);
                }
            }
            (None, _) => {}
        }

        None
    }
}
