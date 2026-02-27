use super::{KlineChart, StudiesDirtyReason};
use crate::chart::shared::study_helper as sh;

impl KlineChart {
    // ── Study management ──────────────────────────────────────────────

    pub fn add_study(&mut self, study: Box<dyn study::Study>) {
        let is_panel = study.placement() == study::StudyPlacement::Panel;

        // Enforce: only one CandleReplace study at a time
        if study.placement() == study::StudyPlacement::CandleReplace {
            self.studies
                .retain(|s| s.placement() != study::StudyPlacement::CandleReplace);
        }
        self.studies.push(study);

        // Ensure splits has a default entry for the main/panel divider
        if is_panel && self.chart.layout.splits.is_empty() {
            self.chart.layout.splits.push(0.75);
        }

        self.studies_dirty = Some(StudiesDirtyReason::FullRecompute);
        self.invalidate();
    }

    pub fn remove_study(&mut self, id: &str) {
        let has_panels = sh::remove_study_by_id(&mut self.studies, id);

        // If no panel studies remain, clear the splits vector
        if !has_panels {
            self.chart.layout.splits.clear();
        }

        self.invalidate();
    }

    /// Mark studies as needing recomputation (e.g. after parameter changes).
    pub fn mark_studies_dirty(&mut self) {
        self.studies_dirty = Some(StudiesDirtyReason::FullRecompute);
    }

    pub fn studies(&self) -> &[Box<dyn study::Study>] {
        &self.studies
    }

    pub fn studies_mut(&mut self) -> &mut Vec<Box<dyn study::Study>> {
        &mut self.studies
    }

    pub fn update_study_parameter(
        &mut self,
        study_id: &str,
        key: &str,
        value: study::ParameterValue,
    ) {
        if let Some(s) = self.studies.iter_mut().find(|s| s.id() == study_id)
            && let Err(e) = s.set_parameter(key, value)
        {
            log::warn!("Failed to set study parameter: {}", e);
        }
        self.recompute_studies(StudiesDirtyReason::FullRecompute);
        self.invalidate();
    }

    pub(super) fn recompute_studies(&mut self, reason: StudiesDirtyReason) {
        if self.studies.is_empty() {
            return;
        }
        let input = sh::build_study_input(
            &self.chart_data.candles,
            &self.chart_data.trades,
            self.basis,
            &self.ticker_info,
            self.last_visible_range,
        );
        match reason {
            StudiesDirtyReason::FullRecompute => {
                sh::recompute_all(&mut self.studies, &input);
            }
            StudiesDirtyReason::NewTradesAppended => {
                let trades = &self.chart_data.trades;
                sh::append_trades_to_studies(&mut self.studies, trades, &input);
            }
        }
    }
}
