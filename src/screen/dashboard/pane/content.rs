use crate::chart::{comparison::ComparisonChart, heatmap::HeatmapChart, candlestick::KlineChart};
use crate::screen::dashboard::panel::{ladder::Ladder, timeandsales::TimeAndSales};
use crate::component::layout::reorderable_list as column_drag;

use data::{
    ContentKind, DrawingTool, FootprintStudy, HeatmapIndicator, KlineIndicator, Settings, UiIndicator,
    ViewConfig, VisualConfig,
};
use exchange::FuturesTickerInfo;
use std::time::Instant;

#[derive(Default)]
pub enum Content {
    #[default]
    Starter,
    Heatmap {
        chart: Option<HeatmapChart>,
        indicators: Vec<HeatmapIndicator>,
        layout: ViewConfig,
        studies: Vec<data::domain::chart_ui_types::heatmap::HeatmapStudy>,
    },
    Kline {
        chart: Option<KlineChart>,
        indicators: Vec<KlineIndicator>,
        layout: ViewConfig,
        kind: data::KlineChartKind,
    },
    TimeAndSales(Option<TimeAndSales>),
    Ladder(Option<Ladder>),
    Comparison(Option<ComparisonChart>),
}

impl Content {
    /// Create empty content for a given kind (will be populated when chart data loads)
    pub fn new_for_kind(
        kind: ContentKind,
        ticker_info: FuturesTickerInfo,
        settings: &Settings,
    ) -> Self {
        match kind {
            ContentKind::HeatmapChart => Content::Heatmap {
                chart: None,
                indicators: vec![HeatmapIndicator::Volume],
                layout: ViewConfig {
                    splits: vec![],
                    autoscale: Some(data::Autoscale::CenterLatest),
                },
                studies: vec![],
            },
            ContentKind::CandlestickChart => Content::Kline {
                chart: None,
                indicators: vec![KlineIndicator::Volume],
                layout: ViewConfig {
                    splits: vec![0.8],
                    autoscale: Some(data::Autoscale::FitAll),
                },
                kind: data::KlineChartKind::Candles,
            },
            ContentKind::FootprintChart => Content::Kline {
                chart: None,
                indicators: vec![KlineIndicator::Volume],
                layout: ViewConfig {
                    splits: vec![0.8],
                    autoscale: Some(data::Autoscale::FitAll),
                },
                kind: data::KlineChartKind::Footprint {
                    clusters: data::ClusterKind::default(),
                    scaling: data::ClusterScaling::default(),
                    studies: vec![],
                },
            },
            ContentKind::TimeAndSales => {
                let state_config = settings
                    .visual_config
                    .clone()
                    .and_then(|v| v.time_and_sales());
                // Convert state config to panel config
                let panel_config = state_config.map(|cfg| {
                    data::panel::timeandsales::Config {
                        max_rows: cfg.max_rows,
                        ..Default::default()
                    }
                });
                Content::TimeAndSales(Some(TimeAndSales::new(panel_config, ticker_info.into())))
            }
            ContentKind::Ladder => {
                let state_config = settings.visual_config.clone().and_then(|v| v.ladder());
                // Convert state config to panel config
                let panel_config = state_config.map(|cfg| {
                    data::panel::ladder::Config {
                        levels: cfg.levels,
                        ..Default::default()
                    }
                });
                Content::Ladder(Some(Ladder::new(panel_config, ticker_info.into(), ticker_info.tick_size)))
            }
            ContentKind::ComparisonChart => Content::Comparison(None),
            ContentKind::Starter => Content::Starter,
        }
    }

    pub(crate) fn placeholder(kind: ContentKind) -> Self {
        match kind {
            ContentKind::Starter => Content::Starter,
            ContentKind::CandlestickChart => Content::Kline {
                chart: None,
                indicators: vec![KlineIndicator::Volume],
                kind: data::KlineChartKind::Candles,
                layout: ViewConfig {
                    splits: vec![],
                    autoscale: Some(data::Autoscale::FitAll),
                },
            },
            ContentKind::FootprintChart => Content::Kline {
                chart: None,
                indicators: vec![KlineIndicator::Volume],
                kind: data::KlineChartKind::Footprint {
                    clusters: data::ClusterKind::default(),
                    scaling: data::ClusterScaling::default(),
                    studies: vec![],
                },
                layout: ViewConfig {
                    splits: vec![],
                    autoscale: Some(data::Autoscale::FitAll),
                },
            },
            ContentKind::HeatmapChart => Content::Heatmap {
                chart: None,
                indicators: vec![HeatmapIndicator::Volume],
                studies: vec![],
                layout: ViewConfig {
                    splits: vec![],
                    autoscale: Some(data::Autoscale::CenterLatest),
                },
            },
            ContentKind::ComparisonChart => Content::Comparison(None),
            ContentKind::TimeAndSales => Content::TimeAndSales(None),
            ContentKind::Ladder => Content::Ladder(None),
        }
    }

    pub fn last_tick(&self) -> Option<Instant> {
        match self {
            Content::Heatmap { chart, .. } => Some(chart.as_ref()?.last_update()),
            Content::Kline { chart, .. } => Some(chart.as_ref()?.last_update()),
            Content::TimeAndSales(panel) => Some(panel.as_ref()?.last_update()),
            Content::Ladder(panel) => Some(panel.as_ref()?.last_update()),
            Content::Comparison(chart) => Some(chart.as_ref()?.last_update()),
            Content::Starter => None,
        }
    }

    pub fn chart_kind(&self) -> Option<data::KlineChartKind> {
        match self {
            Content::Kline { chart, .. } => Some(chart.as_ref()?.kind().clone()),
            _ => None,
        }
    }

    pub fn toggle_indicator(&mut self, indicator: UiIndicator) {
        match (&mut *self, indicator) {
            (
                Content::Heatmap {
                    chart, indicators, ..
                },
                UiIndicator::Heatmap(ind),
            ) => {
                let Some(chart) = chart else {
                    return;
                };

                if indicators.contains(&ind) {
                    indicators.retain(|i| i != &ind);
                } else {
                    indicators.push(ind);
                }
                chart.toggle_indicator(ind);
            }
            (
                Content::Kline {
                    chart, indicators, ..
                },
                UiIndicator::Kline(ind),
            ) => {
                let Some(chart) = chart else {
                    return;
                };

                if indicators.contains(&ind) {
                    indicators.retain(|i| i != &ind);
                } else {
                    indicators.push(ind);
                }
                chart.toggle_indicator(ind);
            }
            (other, ind) => {
                log::warn!(
                    "indicator toggle on {ind:?} ignored for \
                     {other} pane"
                );
            }
        }
    }

    pub fn reorder_indicators(&mut self, event: &column_drag::DragEvent) {
        match self {
            Content::Heatmap { indicators, .. } => column_drag::reorder_vec(indicators, event),
            Content::Kline { indicators, .. } => column_drag::reorder_vec(indicators, event),
            Content::TimeAndSales(_)
            | Content::Ladder(_)
            | Content::Starter
            | Content::Comparison(_) => {
                log::warn!("indicator reorder ignored for {} pane", self);
            }
        }
    }

    pub fn change_visual_config(&mut self, config: VisualConfig) {
        match (self, config) {
            (Content::Heatmap { chart: Some(c), .. }, VisualConfig::Heatmap(cfg)) => {
                // Convert data::HeatmapConfig to chart::heatmap::VisualConfig
                let visual = crate::chart::heatmap::VisualConfig {
                    order_size_filter: cfg.order_size_filter,
                    trade_size_filter: cfg.trade_size_filter,
                    trade_size_scale: cfg.trade_size_scale,
                    coalescing: None, // CoalesceKind is not exposed, use None
                    trade_rendering_mode: crate::chart::heatmap::TradeRenderingMode::Auto,
                    max_trade_markers: 10_000,
                };
                c.set_visual_config(visual);
            }
            (Content::Kline { .. }, VisualConfig::Kline(_cfg)) => {
                // KlineChart doesn't expose set_visual_config
                // Future: implement if needed
            }
            (Content::TimeAndSales(Some(panel)), VisualConfig::TimeAndSales(cfg)) => {
                // Convert state config to panel config
                let stacked_bar = cfg.stacked_bar.map(|(is_compact, ratio)| {
                    if is_compact {
                        data::panel::timeandsales::StackedBar::Compact(ratio)
                    } else {
                        data::panel::timeandsales::StackedBar::Full(ratio)
                    }
                });

                panel.config = data::panel::timeandsales::Config {
                    max_rows: cfg.max_rows,
                    show_delta: cfg.show_delta,
                    stacked_bar,
                    trade_size_filter: cfg.trade_size_filter,
                    trade_retention: std::time::Duration::from_secs(cfg.trade_retention_secs),
                };
            }
            (Content::Ladder(Some(panel)), VisualConfig::Ladder(cfg)) => {
                // Convert state config to panel config
                panel.config = data::panel::ladder::Config {
                    levels: cfg.levels,
                    group_by_ticks: panel.config.group_by_ticks, // Preserve existing value
                    show_chase: panel.config.show_chase,         // Preserve existing value
                    show_chase_tracker: cfg.show_chase_tracker,
                    show_spread: cfg.show_spread,
                    trade_retention: std::time::Duration::from_secs(cfg.trade_retention_secs),
                };
            }
            (Content::Comparison(_), VisualConfig::Comparison(_cfg)) => {
                // ComparisonChart doesn't expose set_config for runtime changes
                // Config is set during construction
            }
            _ => {}
        }
    }

    pub fn heatmap_studies(&self) -> Option<Vec<data::domain::chart_ui_types::heatmap::HeatmapStudy>> {
        match &self {
            Content::Heatmap { studies, .. } => Some(studies.clone()),
            _ => None,
        }
    }

    pub fn footprint_studies(&self) -> Option<Vec<FootprintStudy>> {
        match &self {
            Content::Kline { kind, .. } => {
                if let data::KlineChartKind::Footprint { studies, .. } = kind {
                    Some(studies.clone())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn update_heatmap_studies(
        &mut self,
        studies: Vec<data::domain::chart_ui_types::heatmap::HeatmapStudy>,
    ) {
        if let Content::Heatmap {
            chart,
            studies: previous,
            ..
        } = self
        {
            if let Some(c) = chart {
                // Convert data studies to chart studies
                c.studies = studies
                    .iter()
                    .map(|s| match s {
                        data::domain::chart_ui_types::heatmap::HeatmapStudy::VolumeProfile(kind) => {
                            crate::chart::heatmap::HeatmapStudy::VolumeProfile(*kind)
                        }
                    })
                    .collect();
            }
            *previous = studies;
        }
    }

    pub fn update_footprint_studies(&mut self, studies: Vec<FootprintStudy>) {
        if let Content::Kline { chart, kind, .. } = self {
            if let Some(c) = chart {
                c.set_studies(studies.clone());
            }
            if let data::KlineChartKind::Footprint {
                studies: k_studies, ..
            } = kind
            {
                *k_studies = studies;
            }
        }
    }

    pub fn kind(&self) -> ContentKind {
        match self {
            Content::Heatmap { .. } => ContentKind::HeatmapChart,
            Content::Kline { kind, .. } => match kind {
                data::KlineChartKind::Footprint { .. } => ContentKind::FootprintChart,
                data::KlineChartKind::Candles => ContentKind::CandlestickChart,
            },
            Content::TimeAndSales(_) => ContentKind::TimeAndSales,
            Content::Ladder(_) => ContentKind::Ladder,
            Content::Comparison(_) => ContentKind::ComparisonChart,
            Content::Starter => ContentKind::Starter,
        }
    }

    pub(crate) fn initialized(&self) -> bool {
        match self {
            Content::Heatmap { chart, .. } => chart.is_some(),
            Content::Kline { chart, .. } => chart.is_some(),
            Content::TimeAndSales(panel) => panel.is_some(),
            Content::Ladder(panel) => panel.is_some(),
            Content::Comparison(chart) => chart.is_some(),
            Content::Starter => true,
        }
    }

    /// Clear chart/panel objects while keeping the content kind and settings.
    /// Used when a feed disconnects to unload data without losing the pane layout.
    pub fn clear_chart(&mut self) {
        match self {
            Content::Heatmap { chart, .. } => *chart = None,
            Content::Kline { chart, .. } => *chart = None,
            Content::TimeAndSales(panel) => *panel = None,
            Content::Ladder(panel) => *panel = None,
            Content::Comparison(chart) => *chart = None,
            Content::Starter => {}
        }
    }

    /// Set the active drawing tool on the chart
    pub fn set_drawing_tool(&mut self, tool: DrawingTool) {
        use crate::chart::Chart;
        match self {
            Content::Kline { chart: Some(c), .. } => {
                c.drawings.set_tool(tool);
                c.mut_state().cache.clear_crosshair();
            }
            Content::Heatmap { chart: Some(c), .. } => {
                c.drawings.set_tool(tool);
                c.mut_state().cache.clear_crosshair();
            }
            _ => {}
        }
    }

    /// Toggle snap mode for drawing tools
    pub fn toggle_drawing_snap(&mut self) {
        match self {
            Content::Kline { chart: Some(c), .. } => {
                c.drawings.toggle_snap();
            }
            Content::Heatmap { chart: Some(c), .. } => {
                c.drawings.toggle_snap();
            }
            _ => {}
        }
    }

    /// Get the current drawing tool (if chart is active)
    pub fn drawing_tool(&self) -> Option<DrawingTool> {
        match self {
            Content::Kline { chart: Some(c), .. } => Some(c.drawings.active_tool()),
            Content::Heatmap { chart: Some(c), .. } => Some(c.drawings.active_tool()),
            _ => None,
        }
    }

    /// Serialize drawings for persistence
    pub fn serialize_drawings(&self) -> Vec<data::SerializableDrawing> {
        match self {
            Content::Kline { chart: Some(c), .. } => c.drawings.to_serializable(),
            Content::Heatmap { chart: Some(c), .. } => c.drawings.to_serializable(),
            _ => vec![],
        }
    }
}

impl std::fmt::Display for Content {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind())
    }
}

impl PartialEq for Content {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Content::Starter, Content::Starter)
                | (Content::Heatmap { .. }, Content::Heatmap { .. })
                | (Content::Kline { .. }, Content::Kline { .. })
                | (Content::TimeAndSales(_), Content::TimeAndSales(_))
                | (Content::Ladder(_), Content::Ladder(_))
                | (Content::Comparison(_), Content::Comparison(_))
        )
    }
}
