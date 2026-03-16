//! Study Renderer
//!
//! Converts abstract `StudyOutput` render primitives from the study crate
//! into iced canvas draw calls via the platform-agnostic `Canvas` and
//! `ChartView` traits.

pub(crate) mod chart_views;
pub(crate) mod draw_context;
pub(crate) mod iced_canvas;
pub mod panel;
pub mod side_panel;

use crate::chart::ViewState;
use chart_views::{OverlayChartView, theme_from_palette};
use iced::Size;
use iced::theme::palette::Extended;
use iced::widget::canvas::Frame;
#[cfg(feature = "heatmap")]
use iced::{Color, Point};
use iced_canvas::IcedCanvas;
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
/// Delegates to the study crate's platform-agnostic renderer via
/// `IcedCanvas` and `OverlayChartView` adapters.
pub fn render_study_output(
    frame: &mut Frame,
    output: &StudyOutput,
    state: &ViewState,
    bounds: Size,
    placement: StudyPlacement,
    palette: Option<&Extended>,
) {
    let theme = palette.map(theme_from_palette).unwrap_or_default();
    let view = OverlayChartView::new(state, bounds, theme);
    let mut canvas = IcedCanvas::new(frame);
    output.render(&mut canvas, &view, placement, Some(&state.basis), true);
}
