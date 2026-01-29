use crate::modal::layout_manager::LayoutManager;
use crate::screen::dashboard::{Dashboard, pane};
use data::{Axis, UserTimezone, WindowSpec};

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
    pub scale_factor: data::ScaleFactor,
    pub timezone: data::UserTimezone,
    pub sidebar: data::Sidebar,
    pub theme: data::Theme,
    pub custom_theme: Option<data::Theme>,
    pub audio_cfg: data::AudioStream,
    pub downloaded_tickers: data::DownloadedTickersRegistry,
}

impl SavedState {
    pub fn window(&self) -> (iced::window::Position, iced::Size) {
        let position = self.main_window.as_ref().and_then(|w| {
            if let (Some(x), Some(y)) = (w.x, w.y) {
                Some(iced::window::Position::Specific(iced::Point::new(x as f32, y as f32)))
            } else {
                None
            }
        }).unwrap_or(iced::window::Position::Centered);

        let size = self.main_window.as_ref().map_or_else(
            crate::window::default_size,
            |w| iced::Size::new(w.width as f32, w.height as f32)
        );

        (position, size)
    }

    pub fn default_with_service(
        market_data_service: Option<std::sync::Arc<data::MarketDataService>>,
        downloaded_tickers: std::sync::Arc<std::sync::Mutex<data::DownloadedTickersRegistry>>,
    ) -> Self {
        let sidebar = data::Sidebar::default();
        SavedState {
            layout_manager: LayoutManager::new(market_data_service, downloaded_tickers.clone(), sidebar.date_range_preset),
            main_window: None,
            scale_factor: data::ScaleFactor::default(),
            timezone: UserTimezone::default(),
            sidebar,
            theme: data::Theme::default(),
            custom_theme: None,
            audio_cfg: data::AudioStream::default(),
            downloaded_tickers: (*downloaded_tickers.lock().unwrap()).clone(),
        }
    }
}

impl From<&Dashboard> for data::Dashboard {
    fn from(dashboard: &Dashboard) -> Self {
        use pane_grid::Node;

        fn from_layout(panes: &pane_grid::State<pane::State>, node: pane_grid::Node) -> data::Pane {
            match node {
                Node::Split {
                    axis, ratio, a, b, ..
                } => data::Pane::Split {
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
                    .map_or(data::Pane::default(), data::Pane::from),
            }
        }

        let main_window_layout = dashboard.panes.layout().clone();

        let popouts_layout: Vec<(data::Pane, WindowSpec)> = dashboard
            .popout
            .iter()
            .map(|(_, (pane, spec))| (from_layout(pane, pane.layout().clone()), spec.clone()))
            .collect();

        data::Dashboard {
            pane: from_layout(&dashboard.panes, main_window_layout),
            popout: popouts_layout,
        }
    }
}

impl From<&pane::State> for data::Pane {
    fn from(pane: &pane::State) -> Self {
        data::Pane::Content {
            kind: pane.content.kind(),
            settings: pane.settings.clone(),
            link_group: pane.link_group,
        }
    }
}

pub fn configuration(pane: data::Pane) -> Configuration<pane::State> {
    match pane {
        data::Pane::Split { axis, ratio, a, b } => Configuration::Split {
            axis: match axis {
                Axis::Horizontal => pane_grid::Axis::Horizontal,
                Axis::Vertical => pane_grid::Axis::Vertical,
            },
            ratio,
            a: Box::new(configuration(*a)),
            b: Box::new(configuration(*b)),
        },
        data::Pane::Content {
            kind,
            settings,
            link_group,
        } => {
            let content = match kind {
                data::ContentKind::Starter => pane::Content::Starter,
                data::ContentKind::HeatmapChart => pane::Content::Heatmap {
                    chart: None,
                    indicators: vec![data::HeatmapIndicator::Volume],
                    layout: data::ViewConfig::default(),
                    studies: vec![],
                },
                data::ContentKind::CandlestickChart => pane::Content::Kline {
                    chart: None,
                    indicators: vec![],
                    layout: data::ViewConfig::default(),
                    kind: data::KlineChartKind::Candles,
                },
                data::ContentKind::FootprintChart => pane::Content::Kline {
                    chart: None,
                    indicators: vec![],
                    layout: data::ViewConfig::default(),
                    kind: data::KlineChartKind::Footprint {
                        clusters: data::ClusterKind::default(),
                        scaling: data::ClusterScaling::default(),
                        studies: vec![],
                    },
                },
                data::ContentKind::TimeAndSales => pane::Content::TimeAndSales(None),
                data::ContentKind::Ladder => pane::Content::Ladder(None),
                data::ContentKind::ComparisonChart => pane::Content::Comparison(None),
            };

            Configuration::Pane(pane::State::from_config(content, settings, link_group, None))
        }
    }
}

pub fn load_saved_state_without_registry(
    market_data_service: Option<std::sync::Arc<data::MarketDataService>>,
) -> SavedState {
    let downloaded_tickers = std::sync::Arc::new(std::sync::Mutex::new(data::DownloadedTickersRegistry::new()));
    match data::load_state("app-state.json") {
        Ok(state) => {
            // For now, use default layout manager since AppState doesn't have dashboard info yet
            // TODO: Implement proper dashboard persistence in AppState
            SavedState {
                theme: state.selected_theme,
                custom_theme: state.custom_theme,
                layout_manager: LayoutManager::new(market_data_service.clone(), downloaded_tickers.clone(), state.sidebar.date_range_preset),
                main_window: state.main_window,
                timezone: state.timezone,
                sidebar: state.sidebar,
                scale_factor: state.scale_factor,
                audio_cfg: data::AudioStream::default(), // TODO: Use proper audio config from state
                downloaded_tickers: state.downloaded_tickers,
            }
        }
        Err(e) => {
            log::error!(
                "Failed to load/find layout state: {}. Starting with a new layout.",
                e
            );

            SavedState::default_with_service(market_data_service, downloaded_tickers)
        }
    }
}
