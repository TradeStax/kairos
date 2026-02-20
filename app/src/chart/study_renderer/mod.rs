//! Study Renderer
//!
//! Converts abstract `StudyOutput` render primitives from the study crate
//! into iced canvas draw calls.
//!
//! NOTE: This module is not yet wired into the chart rendering pipeline.
//! The `render_study_output` entry point will be called once the new study
//! system is integrated. All items are intentionally kept.

mod band;
mod bar;
mod histogram;
mod levels;
mod line;
mod markers;
mod profile;

use crate::chart::ViewState;
use iced::Size;
use iced::widget::canvas::Frame;
use study::output::MarkerRenderConfig;
use study::{StudyOutput, StudyPlacement};

/// Render a study output onto a chart canvas frame.
///
/// For overlay studies, coordinates are mapped via the chart's price/time axes.
/// For panel studies, a local Y scale is computed from the output's value range.
pub fn render_study_output(
    frame: &mut Frame,
    output: &StudyOutput,
    state: &ViewState,
    bounds: Size,
    placement: StudyPlacement,
    marker_config: Option<&MarkerRenderConfig>,
) {
    match output {
        StudyOutput::Lines(lines) => {
            line::render_lines(frame, lines, state, bounds, placement);
        }
        StudyOutput::Band {
            upper,
            middle,
            lower,
            fill_opacity,
        } => {
            band::render_band(
                frame,
                upper,
                middle.as_ref(),
                lower,
                *fill_opacity,
                state,
                bounds,
                placement,
            );
        }
        StudyOutput::Bars(bars) => {
            bar::render_bars(frame, bars, state, bounds, placement);
        }
        StudyOutput::Histogram(bars) => {
            histogram::render_histogram(frame, bars, state, bounds, placement);
        }
        StudyOutput::Levels(levels) => {
            levels::render_levels(frame, levels, state, bounds);
        }
        StudyOutput::Profile(profile_data) => {
            profile::render_profile(frame, profile_data, state, bounds);
        }
        StudyOutput::Markers(m) => {
            let default_config = MarkerRenderConfig::default();
            let config = marker_config.unwrap_or(&default_config);
            markers::render_markers(frame, m, state, bounds, config);
        }
        StudyOutput::Clusters(_) | StudyOutput::Empty => {}
    }
}

/// Helper: compute min/max Y for a set of f32 values.
fn value_range(values: impl Iterator<Item = f32>) -> Option<(f32, f32)> {
    let mut min = f32::MAX;
    let mut max = f32::MIN;
    let mut count = 0u32;

    for v in values {
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
        count += 1;
    }

    if count == 0 {
        None
    } else {
        // Add small padding so lines don't sit on the edge
        let range = max - min;
        let pad = if range > 0.0 { range * 0.05 } else { 1.0 };
        Some((min - pad, max + pad))
    }
}

/// Map a value to a Y pixel coordinate within a panel.
///
/// `min`/`max` define the value range; `height` is the pixel height.
/// Returns 0 at max, `height` at min (screen Y increases downward).
fn value_to_panel_y(value: f32, min: f32, max: f32, height: f32) -> f32 {
    if max <= min {
        height
    } else {
        height - ((value - min) / (max - min)) * height
    }
}
