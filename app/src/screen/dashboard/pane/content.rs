use crate::chart::{
    candlestick::KlineChart, comparison::ComparisonChart, heatmap::HeatmapChart,
    profile::ProfileChart,
};
use crate::chart::Chart;
use crate::chart::drawing::ChartDrawingAccess;
use crate::components::layout::reorderable_list as column_drag;
use crate::screen::dashboard::panel::{ladder::Ladder, timeandsales::TimeAndSales};

use data::{ContentKind, DrawingTool, HeatmapIndicator, Settings, ViewConfig, VisualConfig};
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
        studies: Vec<data::domain::chart::heatmap::HeatmapStudy>,
    },
    Kline {
        chart: Option<KlineChart>,
        layout: ViewConfig,
        study_ids: Vec<String>,
    },
    TimeAndSales(Option<TimeAndSales>),
    Ladder(Option<Ladder>),
    Comparison(Option<ComparisonChart>),
    Profile {
        chart: Option<ProfileChart>,
        layout: ViewConfig,
        study_ids: Vec<String>,
    },
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
                layout: ViewConfig {
                    splits: vec![],
                    autoscale: Some(data::Autoscale::FitAll),
                },
                study_ids: vec![],
            },
            ContentKind::TimeAndSales => {
                let state_config = settings
                    .visual_config
                    .clone()
                    .and_then(|v| v.time_and_sales());
                // Convert state config to panel config
                let panel_config =
                    state_config.map(|cfg| data::config::panel::timeandsales::Config {
                        max_rows: cfg.max_rows,
                        ..Default::default()
                    });
                Content::TimeAndSales(Some(TimeAndSales::new(panel_config, ticker_info.into())))
            }
            ContentKind::Ladder => {
                let state_config = settings.visual_config.clone().and_then(|v| v.ladder());
                // Convert state config to panel config
                let panel_config = state_config.map(|cfg| data::config::panel::ladder::Config {
                    levels: cfg.levels,
                    ..Default::default()
                });
                Content::Ladder(Some(Ladder::new(
                    panel_config,
                    ticker_info.into(),
                    ticker_info.tick_size,
                )))
            }
            ContentKind::ComparisonChart => Content::Comparison(None),
            ContentKind::ProfileChart => Content::Profile {
                chart: None,
                layout: ViewConfig {
                    splits: vec![],
                    autoscale: Some(data::Autoscale::FitAll),
                },
                study_ids: vec![],
            },
            ContentKind::Starter => Content::Starter,
        }
    }

    pub(crate) fn placeholder(kind: ContentKind) -> Self {
        match kind {
            ContentKind::Starter => Content::Starter,
            ContentKind::CandlestickChart => Content::Kline {
                chart: None,
                layout: ViewConfig {
                    splits: vec![],
                    autoscale: Some(data::Autoscale::FitAll),
                },
                study_ids: vec![],
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
            ContentKind::ProfileChart => Content::Profile {
                chart: None,
                layout: ViewConfig {
                    splits: vec![],
                    autoscale: Some(data::Autoscale::FitAll),
                },
                study_ids: vec![],
            },
        }
    }

    pub fn last_tick(&self) -> Option<Instant> {
        match self {
            Content::Heatmap { chart, .. } => Some(chart.as_ref()?.last_update()),
            Content::Kline { chart, .. } => Some(chart.as_ref()?.last_update()),
            Content::TimeAndSales(panel) => Some(panel.as_ref()?.last_update()),
            Content::Ladder(panel) => Some(panel.as_ref()?.last_update()),
            Content::Comparison(chart) => Some(chart.as_ref()?.last_update()),
            Content::Profile { chart, .. } => Some(chart.as_ref()?.last_update()),
            Content::Starter => None,
        }
    }

    pub fn toggle_heatmap_indicator(&mut self, indicator: HeatmapIndicator) {
        if let Content::Heatmap {
            chart, indicators, ..
        } = self
        {
            let Some(chart) = chart else {
                return;
            };

            if indicators.contains(&indicator) {
                indicators.retain(|i| i != &indicator);
            } else {
                indicators.push(indicator);
            }
            chart.toggle_indicator(indicator);
        }
    }

    pub fn toggle_study(&mut self, study_id: &str) {
        macro_rules! toggle {
            ($chart:expr, $study_ids:expr) => {
                if let Some(pos) = $study_ids.iter().position(|id| id == study_id) {
                    $study_ids.remove(pos);
                    if let Some(c) = $chart {
                        c.remove_study(study_id);
                    }
                } else {
                    let registry = crate::app::services::create_unified_registry();
                    if let Some(study) = registry.create(study_id) {
                        $study_ids.push(study_id.to_string());
                        if let Some(c) = $chart {
                            c.add_study(study);
                        }
                    }
                }
            };
        }
        match self {
            Content::Kline {
                chart, study_ids, ..
            } => toggle!(chart, study_ids),
            Content::Profile {
                chart, study_ids, ..
            } => toggle!(chart, study_ids),
            _ => {}
        }
    }

    pub fn update_study_parameter(
        &mut self,
        study_id: &str,
        key: &str,
        value: study::ParameterValue,
    ) {
        match self {
            Content::Kline { chart: Some(c), .. } => {
                c.update_study_parameter(study_id, key, value);
            }
            Content::Profile { chart: Some(c), .. } => {
                c.update_study_parameter(study_id, key, value);
            }
            _ => {}
        }
    }

    pub fn reorder_indicators(&mut self, event: &column_drag::DragEvent) {
        if let Content::Heatmap { indicators, .. } = self {
            column_drag::reorder_vec(indicators, event);
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
                    trade_rendering_mode: crate::chart::heatmap::TradeRenderingMode::Auto,
                    max_trade_markers: 10_000,
                };
                c.set_visual_config(visual);
            }
            (Content::Kline { chart: Some(c), .. }, VisualConfig::Kline(cfg)) => {
                c.set_candle_style(cfg.candle_style);
                c.set_show_debug_info(cfg.show_debug_info);
            }
            (Content::TimeAndSales(Some(panel)), VisualConfig::TimeAndSales(cfg)) => {
                // Convert state config to panel config
                let stacked_bar = cfg.stacked_bar.map(|(is_compact, ratio)| {
                    if is_compact {
                        data::config::panel::timeandsales::StackedBar::Compact(ratio)
                    } else {
                        data::config::panel::timeandsales::StackedBar::Full(ratio)
                    }
                });

                panel.config = data::config::panel::timeandsales::Config {
                    max_rows: cfg.max_rows,
                    show_delta: cfg.show_delta,
                    stacked_bar,
                    trade_size_filter: cfg.trade_size_filter,
                    trade_retention: std::time::Duration::from_secs(cfg.trade_retention_secs),
                };
            }
            (Content::Ladder(Some(panel)), VisualConfig::Ladder(cfg)) => {
                // Convert state config to panel config
                panel.config = data::config::panel::ladder::Config {
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
            (Content::Profile { chart: Some(c), .. }, VisualConfig::Profile(cfg)) => {
                c.set_display_config(cfg.into());
            }
            _ => {}
        }
    }

    pub fn heatmap_studies(&self) -> Option<Vec<data::domain::chart::heatmap::HeatmapStudy>> {
        match &self {
            Content::Heatmap { studies, .. } => Some(studies.clone()),
            _ => None,
        }
    }

    pub fn update_heatmap_studies(
        &mut self,
        studies: Vec<data::domain::chart::heatmap::HeatmapStudy>,
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
                        data::domain::chart::heatmap::HeatmapStudy::VolumeProfile(kind) => {
                            crate::chart::heatmap::HeatmapStudy::VolumeProfile(*kind)
                        }
                    })
                    .collect();
            }
            *previous = studies;
        }
    }

    pub fn has_indicators(&self) -> bool {
        matches!(
            self,
            Content::Kline { .. } | Content::Heatmap { .. } | Content::Profile { .. }
        )
    }

    pub fn kind(&self) -> ContentKind {
        match self {
            Content::Heatmap { .. } => ContentKind::HeatmapChart,
            Content::Kline { .. } => ContentKind::CandlestickChart,
            Content::TimeAndSales(_) => ContentKind::TimeAndSales,
            Content::Ladder(_) => ContentKind::Ladder,
            Content::Comparison(_) => ContentKind::ComparisonChart,
            Content::Profile { .. } => ContentKind::ProfileChart,
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
            Content::Profile { chart, .. } => chart.is_some(),
            Content::Starter => true,
        }
    }

    /// Append a single trade to the active chart (used by replay).
    pub fn append_trade(&mut self, trade: &data::Trade) {
        match self {
            Content::Kline { chart: Some(c), .. } => c.append_trade(trade),
            Content::Heatmap { chart: Some(c), .. } => c.append_trade(trade),
            Content::Profile { chart: Some(c), .. } => c.append_trade(trade),
            _ => {}
        }
    }

    /// Rebuild the chart from scratch with the given trades (used by replay seek).
    pub fn rebuild_from_trades(&mut self, trades: &[data::Trade]) {
        match self {
            Content::Kline { chart: Some(c), .. } => c.rebuild_from_trades(trades),
            Content::Heatmap { chart: Some(c), .. } => c.rebuild_from_trades(trades),
            Content::Profile { chart: Some(c), .. } => c.rebuild_from_trades(trades),
            _ => {}
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
            Content::Profile { chart, .. } => *chart = None,
            Content::Starter => {}
        }
    }

    /// Get a reference to the chart's drawing system (if content is a
    /// drawable chart type with a loaded chart).
    pub fn drawing_chart(&self) -> Option<&dyn ChartDrawingAccess> {
        match self {
            Content::Kline { chart: Some(c), .. } => Some(c),
            Content::Heatmap { chart: Some(c), .. } => Some(c),
            Content::Profile { chart: Some(c), .. } => Some(c),
            _ => None,
        }
    }

    /// Get a mutable reference to the chart's drawing system.
    pub fn drawing_chart_mut(&mut self) -> Option<&mut dyn ChartDrawingAccess> {
        match self {
            Content::Kline { chart: Some(c), .. } => Some(c),
            Content::Heatmap { chart: Some(c), .. } => Some(c),
            Content::Profile { chart: Some(c), .. } => Some(c),
            _ => None,
        }
    }

    /// Set the active drawing tool on the chart
    pub fn set_drawing_tool(&mut self, tool: DrawingTool) {
        if let Some(chart) = self.drawing_chart_mut() {
            chart.drawings_mut().set_tool(tool);
            chart.invalidate_crosshair_cache();
        }
    }

    /// Toggle snap mode for drawing tools
    pub fn toggle_drawing_snap(&mut self) {
        if let Some(chart) = self.drawing_chart_mut() {
            chart.drawings_mut().toggle_snap();
        }
    }

    /// Get the current drawing tool (if chart is active)
    pub fn drawing_tool(&self) -> Option<DrawingTool> {
        self.drawing_chart().map(|c| c.drawings().active_tool())
    }

    /// Scroll the chart to show the latest data.
    pub fn scroll_to_latest(&mut self) {
        match self {
            Content::Kline { chart: Some(c), .. } => {
                c.mut_state().layout.autoscale = Some(data::Autoscale::CenterLatest);
            }
            Content::Heatmap { chart: Some(c), .. } => {
                c.mut_state().layout.autoscale = Some(data::Autoscale::CenterLatest);
            }
            Content::Profile { chart: Some(c), .. } => {
                c.mut_state().layout.autoscale = Some(data::Autoscale::CenterLatest);
            }
            _ => {}
        }
    }

    /// Apply a zoom step to the X-axis (positive = zoom in, negative = zoom out).
    pub fn zoom_step(&mut self, factor: f32) {
        const ZOOM_BASE: f32 = 1.5;
        match self {
            Content::Kline { chart: Some(c), .. } => {
                let state = c.mut_state();
                state.cell_width = (state.cell_width * ZOOM_BASE.powf(factor)).clamp(2.0, 200.0);
            }
            Content::Heatmap { chart: Some(c), .. } => {
                let state = c.mut_state();
                state.cell_width = (state.cell_width * ZOOM_BASE.powf(factor)).clamp(2.0, 200.0);
            }
            Content::Profile { chart: Some(c), .. } => {
                let state = c.mut_state();
                state.cell_width = (state.cell_width * ZOOM_BASE.powf(factor)).clamp(2.0, 200.0);
            }
            _ => {}
        }
    }

    /// Get Big Trades study output and tick size for the debug modal.
    pub fn big_trades_debug_info(&self) -> Option<(&study::StudyOutput, f32)> {
        match self {
            Content::Kline { chart: Some(c), .. } => {
                let tick_size = c.tick_size();
                c.studies()
                    .iter()
                    .find(|s| s.id() == "big_trades")
                    .map(|s| (s.output(), tick_size))
            }
            _ => None,
        }
    }

    /// Serialize active study configs for persistence
    pub fn serialize_studies(&self) -> Vec<data::StudyInstanceConfig> {
        macro_rules! serialize {
            ($chart:expr, $study_ids:expr) => {
                $chart
                    .studies()
                    .iter()
                    .map(|s| {
                        let parameters = s
                            .config()
                            .values
                            .iter()
                            .filter_map(|(k, v)| {
                                serde_json::to_value(v)
                                    .ok()
                                    .map(|jv| (k.clone(), jv))
                            })
                            .collect();
                        data::StudyInstanceConfig {
                            study_id: s.id().to_string(),
                            enabled: $study_ids
                                .contains(&s.id().to_string()),
                            parameters,
                        }
                    })
                    .collect()
            };
        }
        match self {
            Content::Kline {
                chart: Some(c),
                study_ids,
                ..
            } => serialize!(c, study_ids),
            Content::Profile {
                chart: Some(c),
                study_ids,
                ..
            } => serialize!(c, study_ids),
            _ => vec![],
        }
    }

    /// Serialize drawings for persistence
    pub fn serialize_drawings(&self) -> Vec<data::SerializableDrawing> {
        self.drawing_chart()
            .map(|c| c.drawings().to_serializable())
            .unwrap_or_default()
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
                | (Content::Profile { .. }, Content::Profile { .. })
        )
    }
}
