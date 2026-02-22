use crate::modals::layout::LayoutManager;
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
    pub downloaded_tickers: data::DownloadedTickersRegistry,
    pub data_feeds: data::DataFeedManager,
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
        market_data_service: Option<std::sync::Arc<data::MarketDataService>>,
        downloaded_tickers: std::sync::Arc<std::sync::Mutex<data::DownloadedTickersRegistry>>,
    ) -> Self {
        let sidebar = data::Sidebar::default();
        let data_index =
            std::sync::Arc::new(std::sync::Mutex::new(data::DataIndex::new()));
        SavedState {
            layout_manager: LayoutManager::new(
                market_data_service,
                data_index,
            ),
            main_window: None,
            scale_factor: data::ScaleFactor::default(),
            timezone: UserTimezone::default(),
            sidebar,
            theme: data::Theme::default(),
            custom_theme: None,
            downloaded_tickers: (*data::lock_or_recover(&downloaded_tickers)).clone(),
            data_feeds: data::DataFeedManager::default(),
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
        // Clone settings and sync drawings + studies from chart
        let mut settings = pane.settings.clone();
        settings.drawings = pane.content.serialize_drawings();
        settings.studies = pane.content.serialize_studies();

        data::Pane::Content {
            kind: pane.content.kind(),
            settings,
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
                    layout: data::ViewConfig::default(),
                    study_ids: vec![],
                },
                data::ContentKind::TimeAndSales => pane::Content::TimeAndSales(None),
                data::ContentKind::Ladder => pane::Content::Ladder(None),
                data::ContentKind::ComparisonChart => pane::Content::Comparison(None),
                data::ContentKind::ProfileChart => pane::Content::Starter,
                data::ContentKind::ScriptEditor => {
                    let loader = script::ScriptLoader::new();
                    let script_list = pane::build_script_list(&loader);
                    let script_path = settings
                        .visual_config
                        .as_ref()
                        .and_then(|vc| vc.clone().script_editor())
                        .and_then(|cfg| cfg.script_path.map(std::path::PathBuf::from));
                    let editor = if let Some(ref p) = script_path {
                        if let Ok(content) = std::fs::read_to_string(p) {
                            iced_code_editor::CodeEditor::new(&content, "js")
                                .with_line_numbers_enabled(true)
                        } else {
                            iced_code_editor::CodeEditor::new("", "js")
                                .with_line_numbers_enabled(true)
                        }
                    } else {
                        iced_code_editor::CodeEditor::new("", "js")
                            .with_line_numbers_enabled(true)
                    };
                    pane::Content::ScriptEditor {
                        editor,
                        script_path,
                        script_list,
                    }
                }
            };

            Configuration::Pane(pane::State::from_config(
                content, settings, link_group, None,
            ))
        }
    }
}

pub fn load_saved_state_without_registry(
    market_data_service: Option<std::sync::Arc<data::MarketDataService>>,
) -> SavedState {
    let downloaded_tickers =
        std::sync::Arc::new(std::sync::Mutex::new(data::DownloadedTickersRegistry::new()));
    let state_dir = crate::infra::platform::data_path(None);
    match data::load_state(state_dir.as_path(), "app-state.json") {
        Ok(state) => {
            let layout_manager = rebuild_layout_manager(
                &state,
                market_data_service.clone(),
                downloaded_tickers.clone(),
            );

            SavedState {
                theme: state.selected_theme,
                custom_theme: state.custom_theme,
                layout_manager,
                main_window: state.main_window,
                timezone: state.timezone,
                sidebar: state.sidebar,
                scale_factor: state.scale_factor,
                downloaded_tickers: state.downloaded_tickers,
                data_feeds: state.data_feeds,
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

/// Rebuild the runtime `LayoutManager` from persisted `AppState`.
fn rebuild_layout_manager(
    state: &data::AppState,
    market_data_service: Option<std::sync::Arc<data::MarketDataService>>,
    downloaded_tickers: std::sync::Arc<
        std::sync::Mutex<data::DownloadedTickersRegistry>,
    >,
) -> LayoutManager {
    let persisted = &state.layout_manager;
    let data_index =
        std::sync::Arc::new(std::sync::Mutex::new(data::DataIndex::new()));

    // Seed the data_index from the persisted downloaded_tickers registry
    // so that charts can load before a feed reconnect triggers a full scan.
    {
        let registry = data::lock_or_recover(&downloaded_tickers);
        let mut idx = data_index.lock().unwrap_or_else(|e| e.into_inner());
        let sentinel_feed = uuid::Uuid::nil();
        for ticker_str in registry.list_tickers() {
            if let Some(range) = registry.get_range_by_ticker_str(&ticker_str) {
                let mut dates = std::collections::BTreeSet::new();
                for d in range.dates() {
                    dates.insert(d);
                }
                idx.add_contribution(
                    data::DataKey {
                        ticker: ticker_str,
                        schema: "trades".to_string(),
                    },
                    sentinel_feed,
                    dates,
                    false,
                );
            }
        }
    }

    if persisted.layouts.is_empty() {
        log::info!("No persisted layouts found, creating default");
        return LayoutManager::new(
            market_data_service,
            data_index,
        );
    }

    let mut runtime_layouts = Vec::with_capacity(persisted.layouts.len());
    let mut active_uid = None;

    for saved in &persisted.layouts {
        let uid = uuid::Uuid::new_v4();
        let layout_id = LayoutId {
            unique: uid,
            name: saved.name.clone(),
        };

        if persisted.active_layout.as_deref() == Some(&saved.name) {
            active_uid = Some(uid);
        }

        let mut popout_windows = Vec::new();
        for (pane, window_spec) in &saved.dashboard.popout {
            popout_windows.push((
                configuration(pane.clone()),
                window_spec.clone(),
            ));
        }

        let dashboard = Dashboard::from_config(
            configuration(saved.dashboard.pane.clone()),
            popout_windows,
            market_data_service.clone(),
            data_index.clone(),
        );

        runtime_layouts.push(Layout {
            id: layout_id,
            dashboard,
        });
    }

    log::info!(
        "Restored {} layout(s) from persisted state",
        runtime_layouts.len()
    );

    LayoutManager::from_saved(
        runtime_layouts,
        active_uid,
        market_data_service,
        data_index,
    )
}
