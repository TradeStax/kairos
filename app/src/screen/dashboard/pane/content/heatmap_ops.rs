#[cfg(feature = "heatmap")]
use super::Content;
#[cfg(feature = "heatmap")]
use crate::components::layout::reorderable_list as column_drag;
#[cfg(feature = "heatmap")]
use data::HeatmapIndicator;

#[cfg(feature = "heatmap")]
impl Content {
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

    pub fn heatmap_studies(
        &self,
    ) -> Option<Vec<data::domain::chart::heatmap::heatmap::HeatmapStudy>> {
        match &self {
            Content::Heatmap { studies, .. } => Some(studies.clone()),
            _ => None,
        }
    }

    pub fn update_heatmap_studies(
        &mut self,
        studies: Vec<data::domain::chart::heatmap::heatmap::HeatmapStudy>,
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
                        data::domain::chart::heatmap::heatmap::HeatmapStudy::VolumeProfile(
                            kind,
                        ) => crate::chart::heatmap::HeatmapStudy::VolumeProfile(*kind),
                    })
                    .collect();
            }
            *previous = studies;
        }
    }

    pub fn reorder_indicators(&mut self, event: &column_drag::DragEvent) {
        if let Content::Heatmap { indicators, .. } = self {
            column_drag::reorder_vec(indicators, event);
        }
    }
}
