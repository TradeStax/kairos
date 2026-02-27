//! Study Renderer
//!
//! Converts abstract `StudyOutput` render primitives from the study crate
//! into iced canvas draw calls.

pub(crate) mod coord;
pub(crate) mod footprint;
pub mod panel;
mod primitives;
pub mod side_panel;
pub(crate) mod vbp;

use crate::chart::ViewState;
use iced::Size;
use iced::theme::palette::Extended;
use iced::widget::canvas::Frame;
use study::{StudyOutput, StudyPlacement};

/// Render a study output onto a chart canvas frame.
///
/// For overlay studies, coordinates are mapped via the chart's price/time axes.
/// For panel studies, a local Y scale is computed from the output's value range.
/// The optional `palette` is required for `Footprint` rendering.
pub fn render_study_output(
    frame: &mut Frame,
    output: &StudyOutput,
    state: &ViewState,
    bounds: Size,
    placement: StudyPlacement,
    palette: Option<&Extended>,
) {
    match output {
        StudyOutput::Lines(lines) => {
            primitives::line::render_lines(frame, lines, state, bounds, placement);
        }
        StudyOutput::Band {
            upper,
            middle,
            lower,
            fill_opacity,
        } => {
            primitives::band::render_band(
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
            primitives::bar::render_bars(frame, bars, state, bounds, placement);
        }
        StudyOutput::Histogram(bars) => {
            primitives::histogram::render_histogram(frame, bars, state, bounds, placement);
        }
        StudyOutput::Levels(levels) => {
            primitives::levels::render_levels(frame, levels, state, bounds);
        }
        StudyOutput::Profile(profiles, config) => {
            // Side-panel studies render exclusively in SidePanelCanvas — skip here
            match placement {
                StudyPlacement::SidePanel => {}
                StudyPlacement::Overlay
                | StudyPlacement::Panel
                | StudyPlacement::Background
                | StudyPlacement::CandleReplace => {
                    vbp::render_vbp_multi(frame, profiles, config, state, bounds);
                }
            }
        }
        StudyOutput::StudyCandles(candles) => {
            primitives::study_candle::render_study_candles(
                frame, candles, state, bounds, placement,
            );
        }
        StudyOutput::Markers(data) => {
            primitives::markers::render_markers(
                frame,
                &data.markers,
                state,
                bounds,
                &data.render_config,
            );
        }
        StudyOutput::Composite(outputs) => {
            for sub_output in outputs {
                render_study_output(frame, sub_output, state, bounds, placement, palette);
            }
        }
        StudyOutput::Footprint(data) => {
            if let Some(pal) = palette {
                footprint::render_footprint(frame, data, state, bounds, pal);
            }
        }
        StudyOutput::Empty => {}
    }
}
