//! Platform-agnostic rendering engine for study outputs.
//!
//! The study crate owns all rendering algorithms. The app crate
//! provides platform-specific implementations of [`Canvas`] and
//! [`ChartView`], then calls [`StudyOutput::render()`] to draw.
//!
//! # Architecture
//!
//! ```text
//! StudyOutput::render(&self, canvas, view, placement)
//!        │
//!        ▼
//!   render_output()  ─── dispatches by variant ──►  primitives::*
//!                                                   footprint::*
//!                                                   vbp::*
//! ```

pub mod canvas;
pub mod chart_view;
pub mod constants;
pub mod coord;
pub mod footprint;
pub mod primitives;
pub mod types;
pub mod vbp;

pub use canvas::Canvas;
pub use chart_view::{ChartView, ThemeColors, VisibleRegion};
pub use types::{FontHint, LineStyle, TextAlign};

use crate::core::metadata::StudyPlacement;
use crate::output::StudyOutput;
use data::ChartBasis;

/// Render a [`StudyOutput`] onto a platform-agnostic canvas.
///
/// Dispatches by variant to the appropriate primitive / footprint / VBP
/// renderer. The `placement` parameter controls variant-specific behavior
/// (e.g. `Profile` is skipped for `SidePanel` since it has its own renderer).
pub fn render_output(
    output: &StudyOutput,
    canvas: &mut dyn Canvas,
    view: &dyn ChartView,
    placement: StudyPlacement,
    basis: Option<&ChartBasis>,
    show_text: bool,
) {
    match output {
        StudyOutput::Lines(lines) => {
            primitives::line::render_lines(canvas, lines, view);
        }
        StudyOutput::Band {
            upper,
            middle,
            lower,
            fill_opacity,
        } => {
            primitives::band::render_band(
                canvas,
                upper,
                middle.as_ref(),
                lower,
                *fill_opacity,
                view,
            );
        }
        StudyOutput::Bars(bars) => {
            primitives::bar::render_bars(canvas, bars, view);
        }
        StudyOutput::Histogram(bars) => {
            primitives::histogram::render_histogram(canvas, bars, view);
        }
        StudyOutput::Levels(levels) => {
            primitives::levels::render_levels(canvas, levels, view);
        }
        StudyOutput::Zones(zones) => {
            primitives::zones::render_zones(canvas, zones, view);
        }
        StudyOutput::Profile(profiles, config) => {
            if placement != StudyPlacement::SidePanel {
                vbp::render_vbp_multi(canvas, profiles, config, view);
            }
        }
        StudyOutput::Footprint(data) => {
            if let Some(b) = basis {
                footprint::render_footprint(canvas, data, view, b, show_text);
            }
        }
        StudyOutput::Markers(data) => {
            primitives::markers::render_markers(canvas, &data.markers, view, &data.render_config);
        }
        StudyOutput::StudyCandles(candles) => {
            primitives::study_candle::render_study_candles(canvas, candles, view);
        }
        StudyOutput::Composite(outputs) => {
            for sub in outputs {
                render_output(sub, canvas, view, placement, basis, show_text);
            }
        }
        StudyOutput::Custom(_) | StudyOutput::Empty => {}
    }
}

/// Compute the value range for a panel-placement study output.
///
/// Returns `(min, max)` with 5% padding, suitable for mapping values
/// to the panel's Y axis. Returns `None` for output types that don't
/// have a meaningful value range (e.g. `Levels`, `Profile`, `Footprint`).
pub fn panel_value_range(output: &StudyOutput) -> Option<(f32, f32)> {
    match output {
        StudyOutput::Lines(lines) => {
            coord::value_range(lines.iter().flat_map(|s| s.points.iter().map(|(_, v)| *v)))
        }
        StudyOutput::Bars(bars) => {
            coord::value_range(bars.iter().flat_map(|s| s.points.iter().map(|p| p.value)))
                .map(|(lo, hi)| (lo.min(0.0), hi))
        }
        StudyOutput::Histogram(bars) => coord::value_range(bars.iter().map(|b| b.value))
            .map(|(lo, hi)| (lo.min(0.0), hi.max(0.0))),
        StudyOutput::Band {
            upper,
            middle,
            lower,
            ..
        } => coord::value_range(
            upper
                .points
                .iter()
                .chain(lower.points.iter())
                .chain(middle.iter().flat_map(|m| m.points.iter()))
                .map(|(_, v)| *v),
        ),
        StudyOutput::StudyCandles(series) => coord::value_range(
            series
                .iter()
                .flat_map(|s| s.points.iter())
                .flat_map(|p| [p.low, p.high]),
        )
        .map(|(lo, hi)| (lo.min(0.0), hi)),
        StudyOutput::Composite(outputs) => {
            let mut min = f32::MAX;
            let mut max = f32::MIN;
            let mut found = false;
            for sub in outputs {
                if let Some((lo, hi)) = panel_value_range(sub) {
                    min = min.min(lo);
                    max = max.max(hi);
                    found = true;
                }
            }
            if found { Some((min, max)) } else { None }
        }
        _ => None,
    }
}
