use data::Price as DomainPrice;

use super::ProfileChart;

impl ProfileChart {
    // ── Study management ──────────────────────────────────────────────

    pub fn add_study(&mut self, study: Box<dyn study::Study>) {
        let is_panel = study.placement() == study::StudyPlacement::Panel;

        // Profile charts don't support CandleReplace
        if study.placement() == study::StudyPlacement::CandleReplace {
            return;
        }

        self.studies.push(study);

        // Ensure splits has a default entry for the main/panel divider
        if is_panel && self.chart.layout.splits.is_empty() {
            self.chart.layout.splits.push(0.75);
        }

        self.studies_dirty = true;
        self.invalidate();
    }

    pub fn remove_study(&mut self, id: &str) {
        self.studies.retain(|s| s.id() != id);

        // If no panel studies remain, clear the splits vector
        let has_panels = self
            .studies
            .iter()
            .any(|s| s.placement() == study::StudyPlacement::Panel);
        if !has_panels {
            self.chart.layout.splits.clear();
        }

        self.invalidate();
    }

    pub fn studies(&self) -> &[Box<dyn study::Study>] {
        &self.studies
    }

    pub fn update_study_parameter(
        &mut self,
        study_id: &str,
        key: &str,
        value: study::ParameterValue,
    ) {
        if let Some(s) = self.studies.iter_mut().find(|s| s.id() == study_id) {
            if let Err(e) = s.set_parameter(key, value) {
                log::warn!("Failed to set study parameter: {}", e);
            }
        }
        self.recompute_studies();
        self.invalidate();
    }

    pub(super) fn recompute_studies(&mut self) {
        if self.studies.is_empty() {
            return;
        }
        let input = study::StudyInput {
            candles: &self.chart_data.candles,
            trades: Some(&self.chart_data.trades),
            basis: self.basis,
            tick_size: DomainPrice::from_f32(self.ticker_info.tick_size),
            visible_range: self.last_visible_range,
        };
        for s in &mut self.studies {
            if let Err(e) = s.compute(&input) {
                log::warn!("Study '{}' compute error: {}", s.id(), e);
            }
        }
    }
}
