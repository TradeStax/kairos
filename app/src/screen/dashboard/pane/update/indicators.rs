use crate::modals::{self, pane::Modal};
use super::super::{Action, Content, State};

impl State {
    pub(in super::super) fn open_indicator_manager(&mut self) {
        use crate::modals::pane::indicator::IndicatorManagerModal;

        let content_kind = self.content.kind();
        let active_study_ids = match &self.content {
            Content::Candlestick { study_ids, .. } | Content::Profile { study_ids, .. } => {
                study_ids.clone()
            }
            _ => vec![],
        };
        let studies: Vec<Box<dyn study::Study>> = match &self.content {
            Content::Candlestick { chart, .. } => {
                if let Some(c) = (**chart).as_ref() {
                    c.studies().iter().map(|s| s.clone_study()).collect()
                } else {
                    vec![]
                }
            }
            Content::Profile { chart, .. } => {
                if let Some(c) = (**chart).as_ref() {
                    c.studies().iter().map(|s| s.clone_study()).collect()
                } else {
                    vec![]
                }
            }
            _ => vec![],
        };

        let manager = IndicatorManagerModal::new(content_kind, active_study_ids, studies);
        self.modal = Some(Modal::IndicatorManager(manager));
    }

    /// Open the indicator manager with a specific study pre-selected.
    pub(in super::super) fn open_indicator_manager_for_study(&mut self, study_index: usize) {
        use crate::modals::pane::indicator::{IndicatorManagerModal, SelectedIndicator};

        // Resolve the study ID from the index
        let study_id = match &self.content {
            Content::Candlestick { chart, .. } => (**chart)
                .as_ref()
                .and_then(|c| c.studies().get(study_index).map(|s| s.id().to_string())),
            Content::Profile { chart, .. } => (**chart)
                .as_ref()
                .and_then(|c| c.studies().get(study_index).map(|s| s.id().to_string())),
            _ => None,
        };

        let Some(study_id) = study_id else {
            // Index out of bounds — fall back to normal manager
            self.open_indicator_manager();
            return;
        };

        let content_kind = self.content.kind();
        let active_study_ids = match &self.content {
            Content::Candlestick { study_ids, .. } | Content::Profile { study_ids, .. } => {
                study_ids.clone()
            }
            _ => vec![],
        };
        let studies: Vec<Box<dyn study::Study>> = match &self.content {
            Content::Candlestick { chart, .. } => {
                if let Some(c) = (**chart).as_ref() {
                    c.studies().iter().map(|s| s.clone_study()).collect()
                } else {
                    vec![]
                }
            }
            Content::Profile { chart, .. } => {
                if let Some(c) = (**chart).as_ref() {
                    c.studies().iter().map(|s| s.clone_study()).collect()
                } else {
                    vec![]
                }
            }
            _ => vec![],
        };

        let mut manager = IndicatorManagerModal::new(content_kind, active_study_ids, studies);
        manager.selected = Some(SelectedIndicator::Study(study_id));
        self.modal = Some(Modal::IndicatorManager(manager));
    }

    pub(in super::super) fn handle_indicator_manager(
        &mut self,
        message: modals::pane::indicator::Message,
    ) -> Option<Action> {
        if let Some(Modal::IndicatorManager(ref mut manager)) = self.modal {
            let mut manager = manager.clone();
            if let Some(action) = manager.update(message) {
                use modals::pane::indicator::Action;
                match action {
                    Action::ToggleStudy(study_id) => {
                        self.content.toggle_study(&study_id);
                    }
                    Action::ReorderIndicators(_event) => {
                        #[cfg(feature = "heatmap")]
                        self.content.reorder_indicators(&_event);
                    }
                    Action::StudyParameterUpdated {
                        study_id,
                        key,
                        value,
                    } => {
                        self.content
                            .update_study_parameter(&study_id, &key, value.clone());
                        // Reload chart data when days_to_load changes
                        if study_id == "big_trades"
                            && key == "days_to_load"
                            && let study::ParameterValue::Integer(days) = value
                        {
                            self.modal = Some(Modal::IndicatorManager(manager));
                            return self.rebuild_chart_with_days(days);
                        }
                    }
                    Action::Close => {
                        self.modal = None;
                        return None;
                    }
                }
            }
            self.modal = Some(Modal::IndicatorManager(manager));
        }
        None
    }

    /// Open the level detail modal for a study at the given index.
    ///
    /// Reads `interactive_data()` from the study, downcasts to
    /// `LevelAnalyzerData`, and creates a `LevelDetailModal`.
    pub(in super::super) fn open_level_detail_modal(&mut self, study_index: usize) {
        use crate::modals::pane::level_detail::LevelDetailModal;

        let data = match &self.content {
            Content::Candlestick { chart, .. } => {
                let c = match (**chart).as_ref() {
                    Some(c) => c,
                    None => return,
                };
                let study = match c.studies().get(study_index) {
                    Some(s) => s,
                    None => return,
                };
                study.interactive_data().and_then(|any| {
                    any.downcast_ref::<study::orderflow::level_analyzer::types::LevelAnalyzerData>()
                })
            }
            _ => None,
        };

        if let Some(data) = data {
            let modal = LevelDetailModal::new(study_index, data);
            self.modal = Some(Modal::LevelDetail(modal));
        }
    }
}
