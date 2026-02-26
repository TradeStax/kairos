use super::Content;
use crate::chart::drawing::ChartDrawingAccess;
use crate::chart::Chart;
use crate::drawing::DrawingTool;

impl Content {
    /// Get a reference to the chart's drawing system (if content is a
    /// drawable chart type with a loaded chart).
    pub fn drawing_chart(&self) -> Option<&dyn ChartDrawingAccess> {
        match self {
            Content::Candlestick { chart, .. } => {
                (**chart).as_ref().map(|c| c as &dyn ChartDrawingAccess)
            }
            #[cfg(feature = "heatmap")]
            Content::Heatmap { chart: Some(c), .. } => Some(c),
            Content::Profile { chart, .. } => {
                (**chart).as_ref().map(|c| c as &dyn ChartDrawingAccess)
            }
            _ => None,
        }
    }

    /// Get a mutable reference to the chart's drawing system.
    pub fn drawing_chart_mut(&mut self) -> Option<&mut dyn ChartDrawingAccess> {
        match self {
            Content::Candlestick { chart, .. } => {
                (**chart).as_mut().map(|c| c as &mut dyn ChartDrawingAccess)
            }
            #[cfg(feature = "heatmap")]
            Content::Heatmap { chart: Some(c), .. } => Some(c),
            Content::Profile { chart, .. } => {
                (**chart).as_mut().map(|c| c as &mut dyn ChartDrawingAccess)
            }
            _ => None,
        }
    }

    /// Set the active drawing tool on the chart
    pub fn set_drawing_tool(&mut self, tool: DrawingTool) {
        if let Some(chart) = self.drawing_chart_mut() {
            chart.drawings_mut().set_tool(tool);
            chart.invalidate_crosshair_cache();
        }
    }

    /// Toggle snap mode for drawing tools
    pub fn toggle_drawing_snap(&mut self) {
        if let Some(chart) = self.drawing_chart_mut() {
            chart.drawings_mut().toggle_snap();
        }
    }

    /// Get the current drawing tool (if chart is active)
    pub fn drawing_tool(&self) -> Option<DrawingTool> {
        self.drawing_chart().map(|c| c.drawings().active_tool())
    }

    /// Scroll the chart to show the latest data.
    pub(crate) fn scroll_to_latest(&mut self) {
        match self {
            Content::Candlestick { chart, .. } => {
                if let Some(c) = (**chart).as_mut() {
                    c.mut_state().layout.autoscale = Some(data::Autoscale::CenterLatest);
                }
            }
            #[cfg(feature = "heatmap")]
            Content::Heatmap { chart: Some(c), .. } => {
                c.mut_state().layout.autoscale = Some(data::Autoscale::CenterLatest);
            }
            Content::Profile { chart, .. } => {
                if let Some(c) = (**chart).as_mut() {
                    c.mut_state().layout.autoscale = Some(data::Autoscale::CenterLatest);
                }
            }
            _ => {}
        }
    }

    /// Apply a zoom step to the X-axis (positive = zoom in, negative = zoom out).
    pub(crate) fn zoom_step(&mut self, factor: f32) {
        const ZOOM_BASE: f32 = 1.5;
        match self {
            Content::Candlestick { chart, .. } => {
                if let Some(c) = (**chart).as_mut() {
                    let state = c.mut_state();
                    state.cell_width =
                        (state.cell_width * ZOOM_BASE.powf(factor)).clamp(2.0, 200.0);
                }
            }
            #[cfg(feature = "heatmap")]
            Content::Heatmap { chart: Some(c), .. } => {
                let state = c.mut_state();
                state.cell_width = (state.cell_width * ZOOM_BASE.powf(factor)).clamp(2.0, 200.0);
            }
            Content::Profile { chart, .. } => {
                if let Some(c) = (**chart).as_mut() {
                    let state = c.mut_state();
                    state.cell_width =
                        (state.cell_width * ZOOM_BASE.powf(factor)).clamp(2.0, 200.0);
                }
            }
            _ => {}
        }
    }
}
