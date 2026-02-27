//! Chart Module
//!
//! This module provides the charting infrastructure for the application.
//! It re-exports core types from submodules and provides the main update/view functions.

pub mod candlestick;
pub mod comparison;
pub mod core;
pub mod drawing;
#[cfg(feature = "heatmap")]
pub mod heatmap;
pub mod overlay;
pub mod perf;
pub mod profile;
pub(crate) mod scale;
pub(crate) mod shared;
pub mod study_renderer;
mod update;

// Re-export KlineChart for backwards compatibility

// Re-export core types for public API
pub use core::{
    Chart, ChartState, Interaction, PlotLimits, ViewState, base_mouse_interaction,
    canvas_interaction,
};

use crate::components::layout::multi_split::MultiSplit;
use crate::style;
use data::Autoscale;
use scale::{AxisLabelsX, AxisLabelsY};

use iced::widget::canvas::Canvas;
use iced::{
    Alignment, Element, Length, Point, Rectangle, Theme, Vector,
    widget::{button, center, column, container, mouse_area, row, rule, text, tooltip},
};

use crate::style::tokens;

use study_renderer::panel::{PanelAxisLabelsY, StudyPanelCanvas};
use study_renderer::side_panel::SidePanelCanvas;

/// Zoom sensitivity for scroll wheel operations
const ZOOM_SENSITIVITY: f32 = tokens::chart::ZOOM_SENSITIVITY;

/// Default horizontal split for the side panel (80% main / 20% side).
fn default_side_splits() -> &'static Vec<f32> {
    use std::sync::OnceLock;
    static D: OnceLock<Vec<f32>> = OnceLock::new();
    D.get_or_init(|| vec![0.80])
}

/// Default vertical split for chart vs. bottom panel (75% chart / 25% panel).
fn default_panel_splits() -> &'static Vec<f32> {
    use std::sync::OnceLock;
    static D: OnceLock<Vec<f32>> = OnceLock::new();
    D.get_or_init(|| vec![0.75])
}
/// Exponential zoom base (ratio per unit)
const ZOOM_BASE: f32 = tokens::chart::ZOOM_BASE;
/// Text size for labels
pub const TEXT_SIZE: f32 = tokens::text::BODY;

/// Axis scale click target
#[derive(Debug, Clone, Copy)]
pub enum AxisScaleClicked {
    X,
    Y,
}

/// Chart message for user interactions
#[derive(Debug, Clone, Copy)]
pub enum Message {
    Translated(Vector),
    Scaled(f32, Vector),
    AutoscaleToggled,
    CrosshairMoved(Option<Point>),
    CursorLeft,
    YScaling(f32, f32, bool),
    XScaling(f32, f32, bool),
    BoundsChanged(Rectangle),
    SplitDragged(usize, f32),
    SideSplitDragged(usize, f32),
    SidePanelCrosshairMoved(Option<f32>),
    DoubleClick(AxisScaleClicked),
    // Drawing operations (bool = shift_held for snap constraints)
    DrawingClick(Point, bool),
    DrawingMove(Point, bool),
    DrawingCancel,
    DrawingDelete,
    // Drawing selection and editing
    DrawingSelect(crate::drawing::DrawingId),
    DrawingDeselect,
    DrawingDrag(Point, bool),
    DrawingHandleDrag(Point, usize, bool),
    DrawingDragEnd,
    // Clone placement
    ClonePlacementMove(Point),
    ClonePlacementConfirm(Point),
    ClonePlacementCancel,
    // Context menu
    ContextMenu(Point, Option<crate::drawing::DrawingId>),
    // Double-click on a selected drawing
    DrawingDoubleClick(crate::drawing::DrawingId),
    // Study overlay interactions (index into chart.studies())
    StudyOverlaySelect(usize),
    StudyOverlayDoubleClick(usize),
    StudyOverlayContextMenu(Point, usize),
    /// Open the detail modal for a study that has one.
    StudyDetailClick(usize),
    /// Center the chart vertically on the given price.
    CenterOnPrice(f64),
}

/// Chart action for side effects
pub enum Action {
    ErrorOccurred(crate::services::error::InternalError),
}

/// Update chart state based on message.
///
/// Delegates to the `update` sub-module to keep `mod.rs` focused on
/// type/message definitions and module declarations.
pub use update::update;

/// Render chart view
pub fn view<'a, T: Chart>(
    chart: &'a T,
    timezone: crate::config::UserTimezone,
) -> Element<'a, Message> {
    if chart.is_empty() {
        return center(text("Waiting for data...").size(16)).into();
    }

    let state = chart.state();

    let axis_labels_x = Canvas::new(AxisLabelsX {
        labels_cache: &state.cache.x_labels,
        scaling: state.scaling,
        translation_x: state.translation.x,
        max: state.latest_x,
        basis: state.basis,
        cell_width: state.cell_width,
        timezone,
        chart_bounds: state.bounds,
        interval_keys: chart.interval_keys(),
        autoscaling: state.layout.autoscale,
        remote_crosshair: state.crosshair.remote,
        crosshair_interval: state.crosshair.interval.get(),
    })
    .width(Length::Fill)
    .height(Length::Fill);

    let buttons = {
        let (autoscale_btn_placeholder, autoscale_btn_tooltip) = match state.layout.autoscale {
            Some(Autoscale::CenterLatest) => (text("C"), Some("Center last price")),
            Some(Autoscale::FitAll) => (text("A"), Some("Auto")),
            Some(Autoscale::Disabled) => (text("D"), Some("Disabled")),
            None => (text("C"), Some("Toggle autoscaling")),
        };
        let is_active = state.layout.autoscale.is_some();

        let autoscale_button = button(
            autoscale_btn_placeholder
                .size(10)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center),
        )
        .height(Length::Fill)
        .on_press(Message::AutoscaleToggled)
        .style(move |theme: &Theme, status| style::button::transparent(theme, status, is_active));

        row![
            iced::widget::space::horizontal(),
            tooltip(
                autoscale_button,
                autoscale_btn_tooltip,
                iced::widget::tooltip::Position::Top
            ),
        ]
        .padding(2)
    };

    let y_labels_width = state.y_labels_width();

    // Collect panel and side panel studies
    let panels = chart.panel_studies();
    let panel_cache = chart.panel_cache();
    let sp_cache = chart.side_panel_cache();
    let has_side_panel = !chart.side_panel_studies().is_empty() && sp_cache.is_some();

    // Effective side splits — default to [0.80] until user drags to resize.
    let effective_side_splits: &Vec<f32> = if has_side_panel && state.layout.side_splits.is_empty()
    {
        default_side_splits()
    } else {
        &state.layout.side_splits
    };

    // Compute fill-portion integers (×1000) for panel/axis alignment rows.
    let (main_fill, side_fill): (u16, u16) = if has_side_panel {
        let r = effective_side_splits.first().copied().unwrap_or(0.80);
        let m = (r * 1000.0).round() as u16;
        (m, 1000u16.saturating_sub(m))
    } else {
        (10, 0)
    };

    let axis_labels_y = Canvas::new(AxisLabelsY {
        labels_cache: &state.cache.y_labels,
        translation_y: state.translation.y,
        scaling: state.scaling,
        decimals: state.decimals,
        min: state.base_price_y.to_f32(),
        last_price: state.last_price,
        tick_size: state.tick_size.to_f32_lossy(),
        cell_height: state.cell_height,
        basis: state.basis,
        chart_bounds: state.bounds,
        crosshair_y: state.crosshair.y.get(),
    })
    .width(Length::Fill)
    .height(Length::Fill);

    let main_canvas_elem: Element<'_, Message> = container(
        mouse_area(Canvas::new(chart).width(Length::Fill).height(Length::Fill))
            .on_exit(Message::CursorLeft),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into();

    // Build the content row: [chart (+side panel) | rule | y_axis].
    // The side panel canvas lives here so it shares the exact same height
    // as the main chart canvas — both are siblings inside this row.
    let content: Element<'_, Message> = if has_side_panel {
        let cache = sp_cache.unwrap();
        let xhair_cache = chart.side_panel_crosshair_cache().unwrap_or(cache);

        let side_canvas_elem: Element<'_, Message> = container(
            mouse_area(
                Canvas::new(SidePanelCanvas {
                    studies: chart.side_panel_studies(),
                    state,
                    cache,
                    crosshair_cache: xhair_cache,
                })
                .width(Length::Fill)
                .height(Length::Fill),
            )
            .on_move(|p| Message::SidePanelCrosshairMoved(Some(p.y)))
            .on_exit(Message::CursorLeft),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

        // Always use MultiSplit::horizontal so the separator is draggable.
        let chart_and_side: Element<'_, Message> = MultiSplit::horizontal(
            vec![main_canvas_elem, side_canvas_elem],
            effective_side_splits,
            Message::SideSplitDragged,
        )
        .into();

        row![
            container(chart_and_side)
                .width(Length::FillPortion(main_fill + side_fill))
                .height(Length::FillPortion(120)),
            rule::vertical(1).style(style::split_ruler),
            container(
                mouse_area(axis_labels_y)
                    .on_double_click(Message::DoubleClick(AxisScaleClicked::Y)),
            )
            .width(y_labels_width)
            .height(Length::FillPortion(120)),
        ]
        .into()
    } else {
        row![
            container(main_canvas_elem)
                .width(Length::FillPortion(10))
                .height(Length::FillPortion(120)),
            rule::vertical(1).style(style::split_ruler),
            container(
                mouse_area(axis_labels_y)
                    .on_double_click(Message::DoubleClick(AxisScaleClicked::Y)),
            )
            .width(y_labels_width)
            .height(Length::FillPortion(120)),
        ]
        .into()
    };

    // X-axis row always spans the full available width so time labels align
    // with both the main chart canvas and the ATR panel below.
    let x_axis_row = row![
        container(
            mouse_area(axis_labels_x).on_double_click(Message::DoubleClick(AxisScaleClicked::X)),
        )
        .padding(iced::padding::right(1))
        .width(Length::FillPortion(10))
        .height(Length::Fixed(26.0)),
        buttons.width(y_labels_width).height(Length::Fixed(26.0)),
    ];

    // Build panel row if panel studies exist
    let has_panels = !panels.is_empty() && panel_cache.is_some();
    let panel_labels_cache = chart.panel_labels_cache();

    let chart_body: Element<'_, Message> = if has_panels {
        let cache = panel_cache.unwrap();

        let panel_y_axis: Element<'_, Message> = if let Some(labels_cache) = panel_labels_cache {
            Canvas::new(PanelAxisLabelsY {
                panels: chart.panel_studies(),
                cache: labels_cache,
            })
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else {
            column![].into()
        };

        let panel_crosshair_cache = chart.panel_crosshair_cache();
        let crosshair_cache_ref = panel_crosshair_cache.unwrap_or(cache);

        let panel_canvas = Canvas::new(StudyPanelCanvas {
            panels,
            state,
            cache,
            crosshair_cache: crosshair_cache_ref,
        })
        .width(Length::Fill)
        .height(Length::Fill);

        // The panel row (ATR, RSI, etc.) always spans the full available width —
        // the VBP side panel lives in the content row above, not here.
        // Keeping full width prevents the panel from appearing cut off under the
        // side panel area and keeps the time axis aligned.
        let panel_row: Element<'_, Message> = row![
            container(mouse_area(panel_canvas).on_exit(Message::CursorLeft),)
                .width(Length::FillPortion(10)),
            rule::vertical(1).style(style::split_ruler),
            container(panel_y_axis).width(y_labels_width),
        ]
        .into();

        // Always use MultiSplit — never the column fallback.  The fallback
        // splits Fill height 50/50 which squishes or covers the bottom panel.
        let splits = &state.layout.splits;
        let effective_splits = if splits.is_empty() {
            default_panel_splits()
        } else {
            splits
        };
        MultiSplit::new(vec![content, panel_row], effective_splits, |idx, pos| {
            Message::SplitDragged(idx, pos)
        })
        .into()
    } else {
        content
    };

    let layout = column![chart_body]
        .push(rule::horizontal(1).style(style::split_ruler))
        .push(x_axis_row);

    layout
        .padding(iced::padding::left(1).right(1).bottom(1))
        .into()
}
