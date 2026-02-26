use super::super::State;
use crate::drawing::{DrawingId, DrawingTool};

impl State {
    /// Open the drawing properties modal for the given drawing
    pub(in crate::screen::dashboard::pane) fn handle_open_drawing_properties(
        &mut self,
        id: DrawingId,
    ) {
        use crate::modals::drawing::properties::DrawingPropertiesModal;
        use crate::modals::pane::Modal;

        // AiContext: re-show the bubble instead of properties modal
        let is_ai_context = self
            .content
            .drawing_chart()
            .and_then(|c| c.drawings().get(id))
            .is_some_and(|d| d.tool == DrawingTool::AiContext);
        if is_ai_context {
            self.show_ai_context_bubble(id);
            return;
        }

        let ticker_info = self.ticker_info;
        let modal = self.content.drawing_chart().and_then(|c| {
            c.drawings().get(id).map(|d| {
                let snapshot = d.to_serializable();
                DrawingPropertiesModal::new(
                    d.id,
                    d.tool,
                    &d.style,
                    d.locked,
                    d.visible,
                    d.label.clone(),
                    snapshot,
                    ticker_info,
                )
            })
        });

        if let Some(m) = modal {
            self.modal = Some(Modal::DrawingProperties(Box::new(m)));
        }
    }

    /// Apply a drawing style update without recording undo.
    /// Used for live preview and cancel-revert.
    pub(in crate::screen::dashboard::pane) fn apply_drawing_style(
        &mut self,
        id: DrawingId,
        update: &crate::modals::drawing::properties::DrawingUpdate,
    ) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            if let Some(d) = chart.drawings_mut().get_mut(id) {
                d.style = update.style.clone();
                d.locked = update.locked;
                d.visible = update.visible;
                d.label = update.label.clone();
                // Sync VBP config to embedded study
                if d.tool.is_vbp() {
                    if let (Some(cfg), Some(study)) = (&d.style.vbp_config, &mut d.vbp_study) {
                        study.import_config(&cfg.params);
                        // Force Custom period to preserve drawing anchors
                        if d.points.len() >= 2 {
                            let t1 = d.points[0].time;
                            let t2 = d.points[1].time;
                            study.set_range(t1.min(t2), t1.max(t2));
                        }
                    }
                    chart.drawings_mut().queue_vbp_compute(id);
                }
            }
            chart.compute_pending_vbp();
            chart.invalidate_all_drawing_caches();
        }
    }

    /// Record undo for a property change and sync the default style.
    pub(in crate::screen::dashboard::pane) fn finalize_drawing_properties(
        &mut self,
        id: DrawingId,
        before_snapshot: crate::drawing::SerializableDrawing,
    ) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            chart
                .drawings_mut()
                .record_property_change(id, before_snapshot);
            if let Some(style) = chart.drawings().get(id).map(|d| d.style.clone()) {
                chart.drawings_mut().set_default_style(style);
            }
            chart.compute_pending_vbp();
        }
    }

    /// Delete a specific drawing by ID (from context menu)
    pub(in crate::screen::dashboard::pane) fn handle_drawing_context_delete(
        &mut self,
        id: DrawingId,
    ) {
        if self
            .ai_context_bubble
            .as_ref()
            .is_some_and(|b| b.drawing_id == id)
        {
            self.ai_context_bubble = None;
        }
        if let Some(chart) = self.content.drawing_chart_mut() {
            chart.drawings_mut().delete(id);
            chart.invalidate_all_drawing_caches();
        }
    }

    /// Toggle lock state of a drawing
    pub(in crate::screen::dashboard::pane) fn handle_drawing_toggle_lock(
        &mut self,
        id: DrawingId,
    ) {
        if let Some(chart) = self.content.drawing_chart_mut() {
            if let Some(d) = chart.drawings_mut().get_mut(id) {
                d.locked = !d.locked;
            }
            chart.invalidate_drawings_cache();
        }
    }
}
