use crate::config::{ScaleFactor, Sidebar, Theme, UserTimezone};
use crate::modals::layout::LayoutManager;
use crate::persistence as persist;
use crate::persistence::{AiPreferences, Axis, WindowSpec};
use crate::screen::dashboard::pane::config::ContentKind;
use crate::screen::dashboard::{Dashboard, pane};

use iced::widget::pane_grid::{self, Configuration};
use std::vec;
use uuid::Uuid;

pub struct Layout {
    pub id: LayoutId,
    pub dashboard: Dashboard,
}

#[derive(Debug, Clone)]
pub struct LayoutId {
    pub unique: Uuid,
    pub name: String,
}

pub struct SavedState {
    pub layout_manager: LayoutManager,
    pub main_window: Option<WindowSpec>,
    pub scale_factor: ScaleFactor,
    pub timezone: UserTimezone,
    pub sidebar: Sidebar,
    pub theme: Theme,
    pub custom_theme: Option<Theme>,
    pub downloaded_tickers: data::DownloadedTickersRegistry,
    pub data_feeds: data::ConnectionManager,
    pub ai_preferences: AiPreferences,
    pub auto_update: crate::persistence::AutoUpdatePreferences,
}

impl SavedState {
    pub fn window(&self) -> (iced::window::Position, iced::Size) {
        let position = self
            .main_window
            .as_ref()
            .and_then(|w| {
                if let (Some(x), Some(y)) = (w.x, w.y) {
                    Some(iced::window::Position::Specific(iced::Point::new(
                        x as f32, y as f32,
                    )))
                } else {
                    None
                }
            })
            .unwrap_or(iced::window::Position::Centered);

        let size = self
            .main_window
            .as_ref()
            .map_or_else(crate::window::default_size, |w| {
                iced::Size::new(w.width as f32, w.height as f32)
            });

        (position, size)
    }

    pub fn default_with_service(
        downloaded_tickers: std::sync::Arc<std::sync::Mutex<data::DownloadedTickersRegistry>>,
    ) -> Self {
        let sidebar = Sidebar::default();
        let data_index = std::sync::Arc::new(std::sync::Mutex::new(data::DataIndex::new()));
        SavedState {
            layout_manager: LayoutManager::new(data_index),
            main_window: None,
            scale_factor: ScaleFactor::default(),
            timezone: UserTimezone::default(),
            sidebar,
            theme: Theme::default(),
            custom_theme: None,
            downloaded_tickers: (*data::lock_or_recover(&downloaded_tickers)).clone(),
            data_feeds: data::ConnectionManager::default(),
            ai_preferences: AiPreferences::default(),
            auto_update: crate::persistence::AutoUpdatePreferences::default(),
        }
    }
}

impl From<&Dashboard> for persist::Dashboard {
    fn from(dashboard: &Dashboard) -> Self {
        use pane_grid::Node;

        fn from_layout(
            panes: &pane_grid::State<pane::State>,
            node: pane_grid::Node,
        ) -> persist::Pane {
            match node {
                Node::Split {
                    axis, ratio, a, b, ..
                } => persist::Pane::Split {
                    axis: match axis {
                        pane_grid::Axis::Horizontal => Axis::Horizontal,
                        pane_grid::Axis::Vertical => Axis::Vertical,
                    },
                    ratio,
                    a: Box::new(from_layout(panes, *a)),
                    b: Box::new(from_layout(panes, *b)),
                },
                Node::Pane(pane) => panes
                    .get(pane)
                    .map_or(persist::Pane::default(), persist::Pane::from),
            }
        }

        let main_window_layout = dashboard.panes.layout().clone();

        let popouts_layout: Vec<(persist::Pane, WindowSpec)> = dashboard
            .popout
            .iter()
            .map(|(_, (pane, spec))| (from_layout(pane, pane.layout().clone()), spec.clone()))
            .collect();

        persist::Dashboard {
            pane: from_layout(&dashboard.panes, main_window_layout),
            popout: popouts_layout,
        }
    }
}

impl From<&pane::State> for persist::Pane {
    fn from(pane: &pane::State) -> Self {
        // Clone settings and sync drawings + studies from chart
        let mut settings = pane.settings.clone();
        settings.drawings = pane.content.serialize_drawings();
        settings.studies = pane.content.serialize_studies();

        persist::Pane::Content {
            kind: pane.content.kind(),
            settings: Box::new(settings),
            link_group: pane.link_group,
        }
    }
}

pub fn configuration(pane: persist::Pane) -> Configuration<pane::State> {
    match pane {
        persist::Pane::Split { axis, ratio, a, b } => Configuration::Split {
            axis: match axis {
                Axis::Horizontal => pane_grid::Axis::Horizontal,
                Axis::Vertical => pane_grid::Axis::Vertical,
            },
            ratio,
            a: Box::new(configuration(*a)),
            b: Box::new(configuration(*b)),
        },
        persist::Pane::Content {
            kind,
            settings,
            link_group,
        } => {
            let content = match kind {
                ContentKind::Starter => pane::Content::Starter,
                #[cfg(feature = "heatmap")]
                ContentKind::HeatmapChart => pane::Content::Heatmap {
                    chart: None,
                    indicators: vec![data::HeatmapIndicator::Volume],
                    layout: data::ViewConfig::default(),
                    studies: vec![],
                },
                ContentKind::CandlestickChart => pane::Content::Candlestick {
                    chart: Box::new(None),
                    layout: data::ViewConfig::default(),
                    study_ids: vec![],
                },
                #[cfg(feature = "heatmap")]
                ContentKind::Ladder => pane::Content::Ladder(None),
                ContentKind::ComparisonChart => pane::Content::Comparison(None),
                ContentKind::ProfileChart => pane::Content::Starter,
                // BacktestResult panes are transient (not persisted); restore as Starter
                ContentKind::BacktestResult => pane::Content::Starter,
                // AiAssistant panes: restore the panel (conversation is empty on reload)
                ContentKind::AiAssistant => pane::Content::AiAssistant(
                    crate::screen::dashboard::pane::types::AiAssistantState::new(),
                ),
            };

            Configuration::Pane(pane::State::from_config(
                content, *settings, link_group, None,
            ))
        }
    }
}
