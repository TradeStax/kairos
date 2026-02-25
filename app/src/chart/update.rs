//! Chart update logic — handles [`Message`] for all chart types.

use super::{AxisScaleClicked, Chart, Message, ZOOM_BASE, ZOOM_SENSITIVITY};
use data::{Autoscale, ChartBasis};

/// Update chart state based on message
pub fn update<T: Chart>(chart: &mut T, message: &Message) {
    match message {
        Message::DoubleClick(scale) => {
            let default_chart_width = chart.plot_limits().default_cell_width;
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
            let limits = chart.plot_limits();
            let min_cell_width = limits.min_cell_width;
            let max_cell_width = limits.max_cell_width;

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
            let limits = chart.plot_limits();
            let min_cell_height = limits.min_cell_height;
            let max_cell_height = limits.max_cell_height;

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
        Message::SideSplitDragged(split, size) => {
            let state = chart.mut_state();
            // Auto-populate defaults if first drag
            while state.layout.side_splits.len() <= *split {
                state.layout.side_splits.push(0.80);
            }
            state.layout.side_splits[*split] = (size * 100.0).round() / 100.0;
            return; // layout recomputes from state; no cache invalidation needed
        }
        Message::SidePanelCrosshairMoved(y) => {
            chart.state().crosshair.y.set(*y);
            return chart.invalidate_crosshair();
        }
        Message::CrosshairMoved(pos) => {
            if let Some(p) = pos {
                let state = chart.state();
                let bounds = state.bounds.size();
                if bounds.width > f32::EPSILON {
                    let region = state.visible_region(bounds);
                    let (interval, _) =
                        state.snap_x_to_index(p.x, bounds, region);
                    state.crosshair.interval.set(Some(interval));
                }
                chart.state().crosshair.y.set(Some(p.y));
            } else {
                chart.state().crosshair.interval.set(None);
                chart.state().crosshair.y.set(None);
            }
            return chart.invalidate_crosshair();
        }
        Message::CursorLeft => {
            chart.state().crosshair.interval.set(None);
            chart.state().crosshair.y.set(None);
            return chart.invalidate_crosshair();
        }
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
        | Message::StudyOverlaySelect(_)
        | Message::StudyOverlayDoubleClick(_)
        | Message::StudyOverlayContextMenu(_, _) => {
            // These are handled by the pane/dashboard, not the chart itself
            return;
        }
    }
    chart.invalidate_all();
}
