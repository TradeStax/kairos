use super::Content;
use crate::screen::dashboard::pane::config::{ContentKind, Settings};
use crate::screen::dashboard::pane::types::AiAssistantState;
use data::FuturesTickerInfo;
#[cfg(feature = "heatmap")]
use data::HeatmapIndicator;

impl Content {
    /// Create empty content for a given kind (will be populated when chart data loads)
    pub(crate) fn new_for_kind(
        kind: ContentKind,
        _ticker_info: FuturesTickerInfo,
        _settings: &Settings,
    ) -> Self {
        match kind {
            #[cfg(feature = "heatmap")]
            ContentKind::HeatmapChart => Content::Heatmap {
                chart: None,
                indicators: vec![HeatmapIndicator::Volume],
                layout: Self::center_latest_layout(),
                studies: vec![],
            },
            ContentKind::CandlestickChart => Content::Candlestick {
                chart: Box::new(None),
                layout: Self::fit_all_layout(),
                study_ids: vec![],
            },
            #[cfg(feature = "heatmap")]
            ContentKind::Ladder => {
                let state_config = _settings.visual_config.clone().and_then(|v| v.ladder());
                let panel_config =
                    state_config.map(|cfg| crate::screen::dashboard::ladder::Config {
                        levels: cfg.levels,
                        ..Default::default()
                    });
                Content::Ladder(Some(crate::screen::dashboard::ladder::Ladder::new(
                    panel_config,
                    _ticker_info,
                    _ticker_info.tick_size,
                )))
            }
            ContentKind::ComparisonChart => Content::Comparison(None),
            ContentKind::ProfileChart => Content::Profile {
                chart: Box::new(None),
                layout: Self::fit_all_layout(),
                study_ids: vec![],
            },
            ContentKind::Starter => Content::Starter,
            ContentKind::BacktestResult => Content::Starter,
            ContentKind::AiAssistant => Content::AiAssistant(AiAssistantState::new()),
        }
    }

    pub(crate) fn placeholder(kind: ContentKind) -> Self {
        match kind {
            ContentKind::Starter => Content::Starter,
            ContentKind::CandlestickChart => Content::Candlestick {
                chart: Box::new(None),
                layout: Self::fit_all_layout(),
                study_ids: vec![],
            },
            #[cfg(feature = "heatmap")]
            ContentKind::HeatmapChart => Content::Heatmap {
                chart: None,
                indicators: vec![HeatmapIndicator::Volume],
                studies: vec![],
                layout: Self::center_latest_layout(),
            },
            ContentKind::ComparisonChart => Content::Comparison(None),
            #[cfg(feature = "heatmap")]
            ContentKind::Ladder => Content::Ladder(None),
            ContentKind::ProfileChart => Content::Profile {
                chart: Box::new(None),
                layout: Self::fit_all_layout(),
                study_ids: vec![],
            },
            ContentKind::BacktestResult => Content::Starter,
            ContentKind::AiAssistant => Content::AiAssistant(AiAssistantState::new()),
        }
    }
}
