mod chart_access;
mod data_routing;
mod factory;
mod heatmap_ops;
mod serialization;
mod study_ops;
mod visual_config;

#[cfg(feature = "heatmap")]
use crate::chart::heatmap::HeatmapChart;
use crate::chart::{candlestick::KlineChart, comparison::ComparisonChart, profile::ProfileChart};
#[cfg(feature = "heatmap")]
use crate::screen::dashboard::ladder::Ladder;

use crate::screen::dashboard::pane::config::ContentKind;
use crate::screen::dashboard::pane::types::AiAssistantState;
#[cfg(feature = "heatmap")]
use data::HeatmapIndicator;
use data::ViewConfig;
use std::time::Instant;

#[derive(Default)]
pub enum Content {
    #[default]
    Starter,
    #[cfg(feature = "heatmap")]
    Heatmap {
        chart: Option<HeatmapChart>,
        indicators: Vec<HeatmapIndicator>,
        layout: ViewConfig,
        studies: Vec<data::domain::chart::heatmap::heatmap::HeatmapStudy>,
    },
    Candlestick {
        chart: Box<Option<KlineChart>>,
        layout: ViewConfig,
        study_ids: Vec<String>,
    },
    #[cfg(feature = "heatmap")]
    Ladder(Option<Ladder>),
    Comparison(Option<ComparisonChart>),
    Profile {
        chart: Box<Option<ProfileChart>>,
        layout: ViewConfig,
        study_ids: Vec<String>,
    },
    AiAssistant(AiAssistantState),
}

impl Content {
    pub(crate) fn fit_all_layout() -> ViewConfig {
        ViewConfig {
            splits: vec![],
            autoscale: Some(data::Autoscale::FitAll),
            side_splits: vec![],
        }
    }

    #[cfg(feature = "heatmap")]
    pub(crate) fn center_latest_layout() -> ViewConfig {
        ViewConfig {
            splits: vec![],
            autoscale: Some(data::Autoscale::CenterLatest),
            side_splits: vec![],
        }
    }

    pub fn last_tick(&self) -> Option<Instant> {
        match self {
            #[cfg(feature = "heatmap")]
            Content::Heatmap { chart, .. } => Some(chart.as_ref()?.last_update()),
            Content::Candlestick { chart, .. } => Some((**chart).as_ref()?.last_update()),
            #[cfg(feature = "heatmap")]
            Content::Ladder(panel) => Some(panel.as_ref()?.last_update()),
            Content::Comparison(chart) => Some(chart.as_ref()?.last_update()),
            Content::Profile { chart, .. } => Some((**chart).as_ref()?.last_update()),
            Content::Starter | Content::AiAssistant(_) => None,
        }
    }

    pub(crate) fn kind(&self) -> ContentKind {
        match self {
            #[cfg(feature = "heatmap")]
            Content::Heatmap { .. } => ContentKind::HeatmapChart,
            Content::Candlestick { .. } => ContentKind::CandlestickChart,
            #[cfg(feature = "heatmap")]
            Content::Ladder(_) => ContentKind::Ladder,
            Content::Comparison(_) => ContentKind::ComparisonChart,
            Content::Profile { .. } => ContentKind::ProfileChart,
            Content::Starter => ContentKind::Starter,
            Content::AiAssistant(_) => ContentKind::AiAssistant,
        }
    }

    pub(crate) fn initialized(&self) -> bool {
        match self {
            #[cfg(feature = "heatmap")]
            Content::Heatmap { chart, .. } => chart.is_some(),
            Content::Candlestick { chart, .. } => chart.is_some(),
            #[cfg(feature = "heatmap")]
            Content::Ladder(panel) => panel.is_some(),
            Content::Comparison(chart) => chart.is_some(),
            Content::Profile { chart, .. } => chart.is_some(),
            Content::Starter => true,
            Content::AiAssistant(_) => true,
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
        #[cfg(feature = "heatmap")]
        if matches!(
            (self, other),
            (Content::Heatmap { .. }, Content::Heatmap { .. })
                | (Content::Ladder(_), Content::Ladder(_))
        ) {
            return true;
        }
        matches!(
            (self, other),
            (Content::Starter, Content::Starter)
                | (Content::Candlestick { .. }, Content::Candlestick { .. })
                | (Content::Comparison(_), Content::Comparison(_))
                | (Content::Profile { .. }, Content::Profile { .. })
                | (Content::AiAssistant(_), Content::AiAssistant(_))
        )
    }
}
