//! Indicator Manager Modal
//!
//! Two-column modal for managing chart indicators and studies.
//! Left panel: search, category filter, active/available lists.
//! Right panel: selected indicator settings (parameters, style, display).

mod catalog_view;
mod helpers;
mod settings_view;

use crate::components::layout::reorderable_list as column_drag;

use data::ContentKind;
use palette::Hsva;

// ── Category Filter ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoryFilter {
    All,
    Trend,
    Momentum,
    Volume,
    Volatility,
    OrderFlow,
}

impl CategoryFilter {
    pub const ALL: &'static [CategoryFilter] = &[
        CategoryFilter::All,
        CategoryFilter::Trend,
        CategoryFilter::Momentum,
        CategoryFilter::Volume,
        CategoryFilter::Volatility,
        CategoryFilter::OrderFlow,
    ];

    pub(super) fn matches(&self, category: study::StudyCategory) -> bool {
        match self {
            CategoryFilter::All => true,
            CategoryFilter::Trend => {
                category == study::StudyCategory::Trend
            }
            CategoryFilter::Momentum => {
                category == study::StudyCategory::Momentum
            }
            CategoryFilter::Volume => {
                category == study::StudyCategory::Volume
            }
            CategoryFilter::Volatility => {
                category == study::StudyCategory::Volatility
            }
            CategoryFilter::OrderFlow => {
                category == study::StudyCategory::OrderFlow
            }
        }
    }
}

impl std::fmt::Display for CategoryFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CategoryFilter::All => write!(f, "All"),
            CategoryFilter::Trend => write!(f, "Trend"),
            CategoryFilter::Momentum => write!(f, "Momentum"),
            CategoryFilter::Volume => write!(f, "Volume"),
            CategoryFilter::Volatility => write!(f, "Volatility"),
            CategoryFilter::OrderFlow => write!(f, "Order Flow"),
        }
    }
}

// ── Settings Tab ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Parameters,
    Style,
    Display,
}

impl SettingsTab {
    pub const ALL: &'static [SettingsTab] = &[
        SettingsTab::Parameters,
        SettingsTab::Style,
        SettingsTab::Display,
    ];

    /// Returns the tabs to show for a specific study.
    pub fn tabs_for_study(study_id: &str) -> &'static [SettingsTab] {
        match study_id {
            "big_trades" => &[
                SettingsTab::Parameters,
                SettingsTab::Style,
            ],
            _ => Self::ALL,
        }
    }

    /// Returns the display label for a tab, customized per study.
    pub fn label_for_study(&self, study_id: &str) -> &'static str {
        match (study_id, self) {
            ("big_trades", SettingsTab::Parameters) => "Data Settings",
            ("footprint", SettingsTab::Parameters) => "General",
            ("footprint", SettingsTab::Display) => "Colors",
            (_, SettingsTab::Parameters) => "Parameters",
            (_, SettingsTab::Style) => "Style",
            (_, SettingsTab::Display) => "Display",
        }
    }
}

impl std::fmt::Display for SettingsTab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettingsTab::Parameters => write!(f, "Parameters"),
            SettingsTab::Style => write!(f, "Style"),
            SettingsTab::Display => write!(f, "Display"),
        }
    }
}

// ── Selected Item ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum SelectedIndicator {
    Study(String),
}

// ── State ────────────────────────────────────────────────────────────

pub struct IndicatorManagerModal {
    pub(super) search_query: String,
    pub(super) category_filter: CategoryFilter,
    pub(super) selected: Option<SelectedIndicator>,
    pub(super) settings_tab: SettingsTab,
    /// Study snapshots for reading params (created once on modal open)
    pub(super) study_snapshots: Vec<(String, Box<dyn study::Study>)>,
    /// Active study IDs on modal open
    pub(super) active_study_ids: Vec<String>,
    /// Color picker sub-state
    pub(super) editing_color_key: Option<String>,
    pub(super) editing_color_hsva: Option<Hsva>,
    /// Content kind to determine which indicators to show
    pub(super) content_kind: ContentKind,
}

impl std::fmt::Debug for IndicatorManagerModal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndicatorManagerModal")
            .field("search_query", &self.search_query)
            .field("category_filter", &self.category_filter)
            .field("selected", &self.selected)
            .field("settings_tab", &self.settings_tab)
            .field("active_study_ids", &self.active_study_ids)
            .field("editing_color_key", &self.editing_color_key)
            .field("content_kind", &self.content_kind)
            .finish()
    }
}

impl Clone for IndicatorManagerModal {
    fn clone(&self) -> Self {
        Self {
            search_query: self.search_query.clone(),
            category_filter: self.category_filter,
            selected: self.selected.clone(),
            settings_tab: self.settings_tab,
            study_snapshots: self
                .study_snapshots
                .iter()
                .map(|(id, s)| (id.clone(), s.clone_study()))
                .collect(),
            active_study_ids: self.active_study_ids.clone(),
            editing_color_key: self.editing_color_key.clone(),
            editing_color_hsva: self.editing_color_hsva,
            content_kind: self.content_kind,
        }
    }
}

impl PartialEq for IndicatorManagerModal {
    fn eq(&self, other: &Self) -> bool {
        self.search_query == other.search_query
            && self.category_filter == other.category_filter
            && self.selected == other.selected
            && self.settings_tab == other.settings_tab
            && self.content_kind == other.content_kind
    }
}

impl IndicatorManagerModal {
    pub fn new(
        content_kind: ContentKind,
        active_study_ids: Vec<String>,
        studies: Vec<Box<dyn study::Study>>,
    ) -> Self {
        let study_snapshots: Vec<(String, Box<dyn study::Study>)> = studies
            .into_iter()
            .map(|s| (s.id().to_string(), s))
            .collect();

        Self {
            search_query: String::new(),
            category_filter: CategoryFilter::All,
            selected: None,
            settings_tab: SettingsTab::Parameters,
            study_snapshots,
            active_study_ids,
            editing_color_key: None,
            editing_color_hsva: None,
            content_kind,
        }
    }

    pub fn update(&mut self, msg: Message) -> Option<Action> {
        match msg {
            Message::SearchChanged(query) => {
                self.search_query = query;
            }
            Message::CategorySelected(cat) => {
                self.category_filter = cat;
            }
            Message::SelectIndicator(sel) => {
                self.selected = Some(sel);
                self.settings_tab = SettingsTab::Parameters;
                self.editing_color_key = None;
                self.editing_color_hsva = None;
            }
            Message::TabChanged(tab) => {
                self.settings_tab = tab;
                self.editing_color_key = None;
                self.editing_color_hsva = None;
            }
            Message::ToggleStudy(study_id) => {
                if self.active_study_ids.contains(&study_id) {
                    self.active_study_ids.retain(|id| id != &study_id);
                    self.study_snapshots
                        .retain(|(id, _)| id != &study_id);
                } else {
                    self.active_study_ids.push(study_id.clone());
                    // Create a snapshot for the newly-enabled study
                    let registry = crate::app::services::create_unified_registry();
                    if let Some(s) = registry.create(&study_id) {
                        self.study_snapshots
                            .push((study_id.clone(), s));
                    }
                }
                return Some(Action::ToggleStudy(study_id));
            }
            Message::ReorderIndicator(event) => {
                return Some(Action::ReorderIndicators(event));
            }
            Message::ParameterChanged { study_id, key, value } => {
                // Update local snapshot
                if let Some((_, snapshot)) = self
                    .study_snapshots
                    .iter_mut()
                    .find(|(id, _)| id == &study_id)
                {
                    if let Err(e) = snapshot.set_parameter(&key, value.clone()) {
                        log::warn!("Failed to set study parameter: {}", e);
                    }
                }
                return Some(Action::StudyParameterUpdated {
                    study_id,
                    key,
                    value,
                });
            }
            Message::EditColor(key) => {
                if self.editing_color_key.as_deref() == Some(&key) {
                    self.editing_color_key = None;
                    self.editing_color_hsva = None;
                } else {
                    self.editing_color_key = Some(key);
                    self.editing_color_hsva = None;
                }
            }
            Message::ColorChanged(hsva) => {
                self.editing_color_hsva = Some(hsva);
                // Apply immediately via ParameterChanged
                if let Some(ref key) = self.editing_color_key
                    && let Some(SelectedIndicator::Study(ref sid)) =
                        self.selected
                {
                    let sc = data::config::theme::hsva_to_rgba(hsva);
                    let value =
                        study::ParameterValue::Color(sc);
                    if let Some((_, snapshot)) = self
                        .study_snapshots
                        .iter_mut()
                        .find(|(id, _)| id == sid)
                    {
                        if let Err(e) = snapshot
                            .set_parameter(key, value.clone())
                        {
                            log::warn!("Failed to set study parameter: {}", e);
                        }
                    }
                    return Some(Action::StudyParameterUpdated {
                        study_id: sid.clone(),
                        key: key.clone(),
                        value,
                    });
                }
            }
            Message::DismissColorPicker => {
                self.editing_color_key = None;
                self.editing_color_hsva = None;
            }
            Message::OpenBigTradesDebug => {
                return Some(Action::OpenBigTradesDebug);
            }
            Message::Close => {
                return Some(Action::Close);
            }
        }
        None
    }
}

// ── Messages ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Message {
    SearchChanged(String),
    CategorySelected(CategoryFilter),
    SelectIndicator(SelectedIndicator),
    TabChanged(SettingsTab),
    ToggleStudy(String),
    ReorderIndicator(column_drag::DragEvent),
    ParameterChanged {
        study_id: String,
        key: String,
        value: study::ParameterValue,
    },
    EditColor(String),
    ColorChanged(Hsva),
    DismissColorPicker,
    OpenBigTradesDebug,
    Close,
}

// ── Actions ──────────────────────────────────────────────────────────

pub enum Action {
    ToggleStudy(String),
    ReorderIndicators(column_drag::DragEvent),
    StudyParameterUpdated {
        study_id: String,
        key: String,
        value: study::ParameterValue,
    },
    OpenBigTradesDebug,
    Close,
}
