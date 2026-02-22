//! Chart Module
//!
//! This module provides the charting infrastructure for the application.
//! It re-exports core types from submodules and provides the main update/view functions.

pub mod candlestick;
pub mod comparison;
pub mod core;
pub mod drawing;
pub mod heatmap;
pub mod overlay;
pub mod perf;
pub mod profile;
pub(crate) mod scale;
pub mod study_renderer;

// Re-export KlineChart for backwards compatibility

// Re-export core types for public API
pub use core::{
    Chart, ChartState, Interaction, PanelStudyInfo, PlotConstants, ViewState,
    canvas_interaction,
};

use crate::components::layout::multi_split::MultiSplit;
use crate::style;
use data::{Autoscale, ChartBasis};
use scale::{AxisLabelsX, AxisLabelsY};

use iced::widget::canvas::Canvas;
use iced::{
    Alignment, Element, Length, Point, Rectangle, Theme, Vector,
    widget::{button, center, column, container, mouse_area, row, rule, text, tooltip},
};

use crate::style::tokens;

use study_renderer::panel::{PanelAxisLabelsY, StudyPanelCanvas};

/// Zoom sensitivity for scroll wheel operations
const ZOOM_SENSITIVITY: f32 = tokens::chart::ZOOM_SENSITIVITY;
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
    CrosshairMoved,
    YScaling(f32, f32, bool),
    XScaling(f32, f32, bool),
    BoundsChanged(Rectangle),
    SplitDragged(usize, f32),
    DoubleClick(AxisScaleClicked),
    // Drawing operations (bool = shift_held for snap constraints)
    DrawingClick(Point, bool),
    DrawingMove(Point, bool),
    DrawingCancel,
    DrawingDelete,
    // Drawing selection and editing
    DrawingSelect(data::DrawingId),
    DrawingDeselect,
    DrawingDrag(Point, bool),
    DrawingHandleDrag(Point, usize, bool),
    DrawingDragEnd,
    // Clone placement
    ClonePlacementMove(Point),
    ClonePlacementConfirm(Point),
    ClonePlacementCancel,
    // Context menu
    ContextMenu(Point, Option<data::DrawingId>),
    // Double-click on a selected drawing
    DrawingDoubleClick(data::DrawingId),
    // Indicator panel clicked (panel index among non-overlay indicators)
    IndicatorClicked(usize),
}

/// Chart action for side effects
pub enum Action {
    ErrorOccurred(crate::infra::error::InternalError),
}

/// Update chart state based on message
pub fn update<T: Chart>(chart: &mut T, message: &Message) {
    match message {
        Message::DoubleClick(scale) => {
            let default_chart_width = T::default_cell_width(chart);
            let autoscaled_coords = chart.autoscaled_coords();
            let supports_fit_autoscaling = chart.supports_fit_autoscaling();

            let state = chart.mut_state();

            match scale {
                AxisScaleClicked::X => {
                    state.cell_width = default_chart_width;
                    state.translation = autoscaled_coords;
                }
                AxisScaleClicked::Y => {
                    if supports_fit_autoscaling {
                        state.layout.autoscale = Some(Autoscale::FitAll);
                        state.scaling = 1.0;
                    } else {
                        state.layout.autoscale = Some(Autoscale::CenterLatest);
                    }
                }
            }
        }
        Message::Translated(translation) => {
            let state = chart.mut_state();

            if let Some(Autoscale::FitAll) = state.layout.autoscale {
                state.translation.x = translation.x;
            } else {
                state.translation = *translation;
                state.layout.autoscale = None;
            }
        }
        Message::Scaled(scaling, translation) => {
            let state = chart.mut_state();
            state.scaling = *scaling;
            state.translation = *translation;

            state.layout.autoscale = None;
        }
        Message::AutoscaleToggled => {
            let supports_fit_autoscaling = chart.supports_fit_autoscaling();
            let state = chart.mut_state();

            let current_autoscale = state.layout.autoscale;
            state.layout.autoscale = {
                match current_autoscale {
                    None => Some(Autoscale::CenterLatest),
                    Some(Autoscale::CenterLatest) => {
                        if supports_fit_autoscaling {
                            Some(Autoscale::FitAll)
                        } else {
                            Some(Autoscale::Disabled)
                        }
                    }
                    Some(Autoscale::FitAll) => Some(Autoscale::Disabled),
                    Some(Autoscale::Disabled) => None,
                }
            };

            if state.layout.autoscale.is_some() {
                state.scaling = 1.0;
            }
        }
        Message::XScaling(delta, cursor_to_center_x, is_wheel_scroll) => {
            let min_cell_width = T::min_cell_width(chart);
            let max_cell_width = T::max_cell_width(chart);

            let state = chart.mut_state();

            if !(*delta < 0.0 && state.cell_width > min_cell_width
                || *delta > 0.0 && state.cell_width < max_cell_width)
            {
                return;
            }

            let is_fit_to_visible_zoom =
                !is_wheel_scroll && matches!(state.layout.autoscale, Some(Autoscale::FitAll));

            let zoom_factor = if is_fit_to_visible_zoom {
                ZOOM_SENSITIVITY / 1.5
            } else if *is_wheel_scroll {
                ZOOM_SENSITIVITY
            } else {
                ZOOM_SENSITIVITY * 3.0
            };

            let new_width = (state.cell_width * ZOOM_BASE.powf(delta / zoom_factor))
                .clamp(min_cell_width, max_cell_width);

            if is_fit_to_visible_zoom {
                let anchor_interval = {
                    let latest_x_coord = state.interval_to_x(state.latest_x);
                    if state.is_interval_x_visible(latest_x_coord) {
                        state.latest_x
                    } else {
                        let visible_region = state.visible_region(state.bounds.size());
                        state.x_to_interval(visible_region.x + visible_region.width)
                    }
                };

                let old_anchor_chart_x = state.interval_to_x(anchor_interval);

                state.cell_width = new_width;

                let new_anchor_chart_x = state.interval_to_x(anchor_interval);

                let shift = new_anchor_chart_x - old_anchor_chart_x;
                state.translation.x -= shift;
            } else {
                let (old_scaling, old_translation_x) = { (state.scaling, state.translation.x) };

                let latest_x = state.interval_to_x(state.latest_x);
                let is_interval_x_visible = state.is_interval_x_visible(latest_x);

                let cursor_chart_x = {
                    if *is_wheel_scroll || !is_interval_x_visible {
                        cursor_to_center_x / old_scaling - old_translation_x
                    } else {
                        latest_x / old_scaling - old_translation_x
                    }
                };

                let new_cursor_x = match state.basis {
                    ChartBasis::Time(_) => {
                        let cursor_time = state.x_to_interval(cursor_chart_x);
                        state.cell_width = new_width;

                        state.interval_to_x(cursor_time)
                    }
                    ChartBasis::Tick(_) => {
                        let tick_index = cursor_chart_x / state.cell_width;
                        state.cell_width = new_width;

                        tick_index * state.cell_width
                    }
                };

                if *is_wheel_scroll || !is_interval_x_visible {
                    if !new_cursor_x.is_nan() && !cursor_chart_x.is_nan() {
                        state.translation.x -= new_cursor_x - cursor_chart_x;
                    }

                    state.layout.autoscale = None;
                }
            }
        }
        Message::YScaling(delta, cursor_to_center_y, is_wheel_scroll) => {
            let min_cell_height = T::min_cell_height(chart);
            let max_cell_height = T::max_cell_height(chart);

            let state = chart.mut_state();

            if !(*delta < 0.0 && state.cell_height > min_cell_height
                || *delta > 0.0 && state.cell_height < max_cell_height)
            {
                return;
            }

            let zoom_factor = if *is_wheel_scroll {
                ZOOM_SENSITIVITY
            } else {
                ZOOM_SENSITIVITY * 3.0
            };

            let new_height = (state.cell_height * ZOOM_BASE.powf(delta / zoom_factor))
                .clamp(min_cell_height, max_cell_height);

            let (old_scaling, old_translation_y) = (state.scaling, state.translation.y);

            let cursor_chart_y = cursor_to_center_y / old_scaling - old_translation_y;
            let cursor_price = state.y_to_price(cursor_chart_y);

            state.cell_height = new_height;

            let new_cursor_y = state.price_to_y(cursor_price);

            if !new_cursor_y.is_nan() && !cursor_chart_y.is_nan() {
                state.translation.y -= new_cursor_y - cursor_chart_y;
            }

            if *is_wheel_scroll {
                state.layout.autoscale = None;
            }
        }
        Message::BoundsChanged(bounds) => {
            let state = chart.mut_state();

            // calculate how center shifted
            let old_center_x = state.bounds.width / 2.0;
            let new_center_x = bounds.width / 2.0;
            let center_delta_x = (new_center_x - old_center_x) / state.scaling;

            state.bounds = *bounds;

            if state.layout.autoscale != Some(Autoscale::CenterLatest) {
                state.translation.x += center_delta_x;
            }
        }
        Message::SplitDragged(split, size) => {
            let state = chart.mut_state();

            if let Some(split) = state.layout.splits.get_mut(*split) {
                *split = (size * 100.0).round() / 100.0;
            }
        }
        Message::CrosshairMoved => return chart.invalidate_crosshair(),
        // Drawing messages are handled at the pane level where we have mutable access
        Message::DrawingClick(_, _)
        | Message::DrawingMove(_, _)
        | Message::DrawingCancel
        | Message::DrawingDelete
        | Message::DrawingSelect(_)
        | Message::DrawingDeselect
        | Message::DrawingDrag(_, _)
        | Message::DrawingHandleDrag(_, _, _)
        | Message::DrawingDragEnd
        | Message::ClonePlacementMove(_)
        | Message::ClonePlacementConfirm(_)
        | Message::ClonePlacementCancel
        | Message::ContextMenu(_, _)
        | Message::DrawingDoubleClick(_)
        | Message::IndicatorClicked(_) => {
            // These are handled by the pane/dashboard, not the chart itself
            return;
        }
    }
    chart.invalidate_all();
}

/// Render chart view
pub fn view<'a, T: Chart>(
    chart: &'a T,
    timezone: data::UserTimezone,
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

    // Collect panel studies
    let panels = chart.panel_studies();
    let panel_cache = chart.panel_cache();

    let content: Element<'_, Message> = {
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
        })
        .width(Length::Fill)
        .height(Length::Fill);

        row![
            container(Canvas::new(chart).width(Length::Fill).height(Length::Fill))
                .width(Length::FillPortion(10))
                .height(Length::FillPortion(120)),
            rule::vertical(1).style(style::split_ruler),
            container(
                mouse_area(axis_labels_y)
                    .on_double_click(Message::DoubleClick(AxisScaleClicked::Y))
            )
            .width(y_labels_width)
            .height(Length::FillPortion(120))
        ]
        .into()
    };

    // Build the x-axis row (shared between panel and non-panel layouts)
    let x_axis_row = row![
        container(
            mouse_area(axis_labels_x)
                .on_double_click(Message::DoubleClick(AxisScaleClicked::X))
        )
        .padding(iced::padding::right(1))
        .width(Length::FillPortion(10))
        .height(Length::Fixed(26.0)),
        buttons.width(y_labels_width).height(Length::Fixed(26.0))
    ];

    // Build panel row if panel studies exist
    let has_panels = !panels.is_empty() && panel_cache.is_some();
    let panel_labels_cache = chart.panel_labels_cache();

    let chart_body: Element<'_, Message> = if has_panels {
        let cache = panel_cache.unwrap();

        // Build panel Y-axis labels canvas
        let panel_y_axis: Element<'_, Message> =
            if let Some(labels_cache) = panel_labels_cache {
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

        let panel_canvas =
            Canvas::new(StudyPanelCanvas {
                panels,
                state,
                cache,
            })
            .width(Length::Fill)
            .height(Length::Fill);

        let panel_row: Element<'_, Message> = row![
            container(panel_canvas)
                .width(Length::FillPortion(10)),
            rule::vertical(1).style(style::split_ruler),
            container(panel_y_axis).width(y_labels_width)
        ]
        .into();

        // Ensure layout.splits has at least one entry for the
        // main/panel divider. Default to 0.75 (75% main chart).
        let splits = &state.layout.splits;
        if splits.is_empty() {
            // No splits yet — use a fixed column layout as fallback
            // (the SplitDragged handler will populate splits on first drag)
            column![content, rule::horizontal(1).style(style::split_ruler), panel_row]
                .height(Length::Fill)
                .into()
        } else {
            MultiSplit::new(
                vec![content, panel_row],
                splits,
                |idx, pos| Message::SplitDragged(idx, pos),
            )
            .into()
        }
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

