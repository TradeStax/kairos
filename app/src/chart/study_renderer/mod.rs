//! Study Renderer
//!
//! Converts abstract `StudyOutput` render primitives from the study crate
//! into iced canvas draw calls.

pub(crate) mod coord;
pub(crate) mod draw_context;
pub(crate) mod footprint;
pub mod panel;
mod primitives;
pub mod side_panel;
pub(crate) mod vbp;

use crate::chart::ViewState;
use iced::Size;
use iced::theme::palette::Extended;
use iced::widget::canvas::Frame;
#[cfg(feature = "heatmap")]
use iced::{Color, Point};
use study::{StudyOutput, StudyPlacement};

/// Specification for a stacked buy/sell volume bar used by the heatmap chart.
#[cfg(feature = "heatmap")]
pub struct VolumeBarSpec {
    pub buy_qty: f32,
    pub sell_qty: f32,
    pub max_qty: f32,
    pub buy_color: Color,
    pub sell_color: Color,
    pub alpha: f32,
}

/// Draw a stacked buy/sell volume bar at the given position.
///
/// When `horizontal` is true the bar grows from left to right;
/// when false it grows upward from `y`.
#[cfg(feature = "heatmap")]
pub fn draw_volume_bar(
    frame: &mut Frame,
    x: f32,
    y: f32,
    spec: &VolumeBarSpec,
    extent: f32,
    thickness: f32,
    horizontal: bool,
) {
    if spec.max_qty <= 0.0 {
        return;
    }
    let total = spec.buy_qty + spec.sell_qty;
    if total <= 0.0 {
        return;
    }
    let ratio = total / spec.max_qty;
    let buy_frac = spec.buy_qty / total;

    if horizontal {
        let bar_len = extent * ratio;
        let buy_len = bar_len * buy_frac;
        let sell_len = bar_len - buy_len;
        if buy_len > 0.0 {
            let mut c = spec.buy_color;
            c.a = spec.alpha;
            frame.fill_rectangle(Point::new(x, y), iced::Size::new(buy_len, thickness), c);
        }
        if sell_len > 0.0 {
            let mut c = spec.sell_color;
            c.a = spec.alpha;
            frame.fill_rectangle(
                Point::new(x + buy_len, y),
                iced::Size::new(sell_len, thickness),
                c,
            );
        }
    } else {
        let bar_height = extent * ratio;
        let buy_h = bar_height * buy_frac;
        let sell_h = bar_height - buy_h;
        if sell_h > 0.0 {
            let mut c = spec.sell_color;
            c.a = spec.alpha;
            frame.fill_rectangle(
                Point::new(x, y + extent - bar_height),
                iced::Size::new(thickness, sell_h),
                c,
            );
        }
        if buy_h > 0.0 {
            let mut c = spec.buy_color;
            c.a = spec.alpha;
            frame.fill_rectangle(
                Point::new(x, y + extent - bar_height + sell_h),
                iced::Size::new(thickness, buy_h),
                c,
            );
        }
    }
}

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
        StudyOutput::Custom(_) => {
            // Custom outputs are rendered by their own registered renderer
            // (future: dispatch via CustomRendererRegistry)
        }
        StudyOutput::Empty => {}
    }
}
