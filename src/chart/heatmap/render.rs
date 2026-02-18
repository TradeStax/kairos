//! Heatmap rendering implementation
//!
//! This module contains the canvas::Program implementation for HeatmapChart
//! and all rendering-related functions.

use crate::chart::{Chart, Interaction, Message, ViewState};
use crate::chart::drawing;
use crate::style;
use super::{HeatmapChart, VisualConfig};
use super::data::HeatmapData;
use super::trades::{
    TradeRenderingMode, render_sparse_trades, render_dense_trades,
    SPARSE_MODE_THRESHOLD, MAX_RENDER_BUDGET,
};
use data::{ChartBasis, HeatmapIndicator, Price as DataPrice};
use data::util::abbr_large_numbers;
use iced::theme::palette::Extended;
use iced::widget::canvas::{self, Event, Geometry};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, Vector, mouse};

// Re-export HeatmapStudy and ProfileKind from data module
pub use data::domain::chart_ui_types::heatmap::{HeatmapStudy, ProfileKind};

impl canvas::Program<Message> for HeatmapChart {
    type State = Interaction;

    fn update(
        &self,
        interaction: &mut Interaction,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        crate::chart::canvas_interaction(self, interaction, event, bounds, cursor)
    }

    fn draw(
        &self,
        interaction: &Interaction,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let chart = self.state();

        if bounds.width == 0.0 {
            return vec![];
        }

        let bounds_size = bounds.size();
        let palette = theme.extended_palette();

        // Main heatmap layer
        let heatmap = chart.cache.main.draw(renderer, bounds_size, |frame| {
            let center = Vector::new(bounds.width / 2.0, bounds.height / 2.0);

            frame.translate(center);
            frame.scale(chart.scaling);
            frame.translate(chart.translation);

            let region = chart.visible_region(frame.size());

            let (earliest, latest) = chart.interval_range(&region);
            let (highest_exch, lowest_exch) = chart.price_range(&region);

            // Convert exchange::util::Price to data::Price
            let highest = DataPrice::from_units(highest_exch.units);
            let lowest = DataPrice::from_units(lowest_exch.units);

            if latest < earliest {
                return;
            }

            let cell_height = chart.cell_height;
            let qty_scales = self.calc_qty_scales(earliest, latest, highest, lowest);

            let max_depth_qty = qty_scales.max_depth_qty;
            let (max_aggr_volume, max_trade_qty) =
                (qty_scales.max_aggr_volume, qty_scales.max_trade_qty);

            let volume_indicator = self.indicators[HeatmapIndicator::Volume].is_some();

            // Draw Depth Heatmap
            draw_depth_heatmap(
                frame,
                chart,
                &self.heatmap_data,
                palette,
                earliest,
                latest,
                highest,
                lowest,
                cell_height,
                max_depth_qty,
                self.visual_config.order_size_filter,
            );

            // Draw Latest Orderbook Bars
            draw_latest_orderbook(
                frame,
                chart,
                &self.heatmap_data,
                palette,
                highest,
                lowest,
                cell_height,
                &region,
            );

            // Draw Trade Markers
            draw_trade_markers(
                frame,
                chart,
                &self.heatmap_data,
                palette,
                earliest,
                latest,
                highest,
                lowest,
                cell_height,
                max_trade_qty,
                &self.visual_config,
            );

            // Draw Volume Indicator
            if volume_indicator {
                draw_volume_indicator(
                    frame,
                    chart,
                    &self.heatmap_data,
                    palette,
                    earliest,
                    latest,
                    max_aggr_volume,
                    bounds.height,
                    &region,
                );
            }

            // Draw Volume Profile Study
            if let Some(profile_kind) = self.studies.iter().map(|study| {
                match study {
                    HeatmapStudy::VolumeProfile(profile) => profile,
                }
            }).next() {
                draw_volume_profile(
                    frame,
                    &region,
                    profile_kind,
                    palette,
                    chart,
                    &self.heatmap_data,
                    (bounds.width / chart.scaling) * 0.1,
                    self.basis,
                );
            }

            // Draw data gap markers
            if !self.chart_data.gaps.is_empty() {
                crate::chart::overlay::draw_gap_markers(frame, chart, &self.chart_data.gaps, &region);
            }
        });

        // Crosshair layer (includes drawings)
        if !self.is_empty() {
            let crosshair = chart.cache.crosshair.draw(renderer, bounds_size, |frame| {
                // Draw all completed drawings and pending preview
                drawing::render::draw_drawings(frame, chart, &self.drawings, bounds_size, palette);

                if let Some(cursor_position) = cursor.position_in(bounds) {
                    // Draw ruler if active
                    if let Interaction::Ruler { start: Some(start) } = interaction {
                        crate::chart::overlay::draw_ruler(chart, frame, palette, bounds_size, *start, cursor_position);
                    }

                    // Draw crosshair
                    let _result = crate::chart::overlay::draw_crosshair(
                        chart,
                        frame,
                        theme,
                        bounds_size,
                        cursor_position,
                        interaction,
                    );

                    // Skip tooltip during interactions
                    if matches!(interaction, Interaction::Panning { .. })
                        || matches!(interaction, Interaction::Ruler { start } if start.is_some())
                    {
                    }
                }
            });

            vec![heatmap, crosshair]
        } else {
            vec![heatmap]
        }
    }

    fn mouse_interaction(
        &self,
        interaction: &Interaction,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match interaction {
            Interaction::Panning { .. } => mouse::Interaction::Grabbing,
            Interaction::Zoomin { .. } => mouse::Interaction::ZoomIn,
            Interaction::Drawing { .. } => {
                if cursor.is_over(bounds) {
                    mouse::Interaction::Crosshair
                } else {
                    mouse::Interaction::default()
                }
            }
            Interaction::EditingDrawing { .. } => {
                if cursor.is_over(bounds) {
                    mouse::Interaction::Grabbing
                } else {
                    mouse::Interaction::default()
                }
            }
            Interaction::None | Interaction::Ruler { .. } => {
                if cursor.is_over(bounds) {
                    return mouse::Interaction::Crosshair;
                }
                mouse::Interaction::default()
            }
        }
    }
}

/// Get depth color based on side and alpha
fn depth_color(palette: &Extended, is_bid: bool, alpha: f32) -> Color {
    if is_bid {
        palette.success.strong.color.scale_alpha(alpha)
    } else {
        palette.danger.strong.color.scale_alpha(alpha)
    }
}

/// Draw depth heatmap cells
#[allow(clippy::too_many_arguments)]
fn draw_depth_heatmap(
    frame: &mut canvas::Frame,
    chart: &ViewState,
    heatmap_data: &HeatmapData,
    palette: &Extended,
    earliest: u64,
    latest: u64,
    highest: DataPrice,
    lowest: DataPrice,
    cell_height: f32,
    max_depth_qty: f32,
    order_size_filter: f32,
) {
    for (price_units, runs) in
        heatmap_data.iter_depth_filtered(earliest, latest, highest, lowest)
    {
        let price = DataPrice::from_units(*price_units);
        let y_position = chart.price_to_y(exchange::util::Price::from_units(price.to_units()));

        for run in runs.iter() {
            if run.qty <= order_size_filter {
                continue;
            }

            let start_x = chart.interval_to_x(run.start_time.max(earliest));
            let end_x = chart.interval_to_x(run.until_time.min(latest)).min(0.0);

            let width = end_x - start_x;

            if width > 0.001 {
                let color_alpha = (run.qty / max_depth_qty).min(1.0);

                frame.fill_rectangle(
                    Point::new(start_x, y_position - (cell_height / 2.0)),
                    Size::new(width, cell_height),
                    depth_color(palette, run.is_bid, color_alpha),
                );
            }
        }
    }
}

/// Draw latest orderbook bars
#[allow(clippy::too_many_arguments)]
fn draw_latest_orderbook(
    frame: &mut canvas::Frame,
    chart: &ViewState,
    heatmap_data: &HeatmapData,
    palette: &Extended,
    highest: DataPrice,
    lowest: DataPrice,
    cell_height: f32,
    region: &Rectangle,
) {
    if heatmap_data.trades_by_time.is_empty() {
        return;
    }

    let latest_timestamp = heatmap_data.latest_depth_time;
    let latest_runs: Vec<_> = heatmap_data
        .latest_order_runs(highest, lowest, latest_timestamp)
        .collect();

    let max_qty = latest_runs
        .iter()
        .map(|(_, run)| run.qty())
        .fold(f32::MIN, f32::max)
        .ceil()
        * 5.0
        / 5.0;

    if !max_qty.is_infinite() && max_qty > 0.0 {
        // Draw bars
        for (price, run) in latest_runs {
            let y_position = chart.price_to_y(exchange::util::Price::from_units(price.to_units()));
            let bar_width = (run.qty() / max_qty) * 50.0;

            frame.fill_rectangle(
                Point::new(0.0, y_position - (cell_height / 2.0)),
                Size::new(bar_width, cell_height),
                depth_color(palette, run.is_bid, 0.5),
            );
        }

        // Draw max quantity label
        let text_size = 9.0 / chart.scaling;
        let text_content = abbr_large_numbers(max_qty);
        let text_position = Point::new(50.0, region.y);

        frame.fill_text(canvas::Text {
            content: text_content,
            position: text_position,
            size: iced::Pixels(text_size),
            color: palette.background.base.text,
            font: style::AZERET_MONO,
            ..canvas::Text::default()
        });
    }
}

/// Draw trade markers (circles or rectangles based on mode)
#[allow(clippy::too_many_arguments)]
fn draw_trade_markers(
    frame: &mut canvas::Frame,
    chart: &ViewState,
    heatmap_data: &HeatmapData,
    palette: &Extended,
    earliest: u64,
    latest: u64,
    highest: DataPrice,
    lowest: DataPrice,
    cell_height: f32,
    max_trade_qty: f32,
    visual_config: &VisualConfig,
) {
    // Count total visible trades for density calculation
    let visible_trade_count: usize = heatmap_data
        .trades_by_time
        .range(earliest..=latest)
        .map(|(_, dp)| dp.grouped_trades.len())
        .sum();

    // Calculate LOD level for adaptive quality
    let lod_calc = crate::chart::perf::lod::LodCalculator::new(
        chart.scaling,
        chart.cell_width,
        visible_trade_count,
        chart.visible_region(chart.bounds.size()).width,
    );
    let lod_level = lod_calc.calculate_lod();

    // Determine rendering mode (considering LOD)
    let effective_mode = match visual_config.trade_rendering_mode {
        TradeRenderingMode::Sparse => {
            // Even in sparse mode, switch to dense if LOD is low
            if matches!(lod_level, crate::chart::perf::lod::LodLevel::Low) && visible_trade_count > SPARSE_MODE_THRESHOLD {
                TradeRenderingMode::Dense
            } else {
                TradeRenderingMode::Sparse
            }
        }
        TradeRenderingMode::Dense => TradeRenderingMode::Dense,
        TradeRenderingMode::Auto => {
            if visible_trade_count > SPARSE_MODE_THRESHOLD || matches!(lod_level, crate::chart::perf::lod::LodLevel::Low) {
                TradeRenderingMode::Dense
            } else {
                TradeRenderingMode::Sparse
            }
        }
    };

    match effective_mode {
        TradeRenderingMode::Sparse => {
            let max_markers = visual_config.max_trade_markers.min(MAX_RENDER_BUDGET);
            let decimation_factor = lod_calc.effective_decimation(max_markers);

            render_sparse_trades(
                frame,
                chart,
                heatmap_data,
                palette,
                earliest,
                latest,
                visual_config.trade_size_filter,
                visual_config.trade_size_scale,
                max_trade_qty,
                cell_height,
                max_markers,
                decimation_factor,
            );
        }
        TradeRenderingMode::Dense => {
            render_dense_trades(
                frame,
                chart,
                heatmap_data,
                palette,
                earliest,
                latest,
                highest,
                lowest,
                visual_config.trade_size_filter,
                max_trade_qty,
                cell_height,
            );
        }
        TradeRenderingMode::Auto => unreachable!(), // Already resolved above
    }
}

/// Draw volume indicator bars
#[allow(clippy::too_many_arguments)]
fn draw_volume_indicator(
    frame: &mut canvas::Frame,
    chart: &ViewState,
    heatmap_data: &HeatmapData,
    palette: &Extended,
    earliest: u64,
    latest: u64,
    max_aggr_volume: f32,
    bounds_height: f32,
    region: &Rectangle,
) {
    for (time, dp) in heatmap_data.trades_by_time.range(earliest..=latest) {
        let x_position = chart.interval_to_x(*time);

        let bar_width = (chart.cell_width / 2.0) * 0.9;
        let area_height = (bounds_height / chart.scaling) * 0.1;

        let (buy_volume, sell_volume) = (dp.buy_volume, dp.sell_volume);

        crate::chart::draw_volume_bar(
            frame,
            x_position,
            (region.y + region.height) - area_height,
            buy_volume,
            sell_volume,
            max_aggr_volume,
            area_height,
            bar_width,
            palette.success.base.color,
            palette.danger.base.color,
            1.0,
            false,
        );
    }

    // Draw max volume label
    if max_aggr_volume > 0.0 {
        let text_size = 9.0 / chart.scaling;
        let text_content = abbr_large_numbers(max_aggr_volume);
        let text_width = (text_content.len() as f32 * text_size) / 1.5;

        let text_position = Point::new(
            (region.x + region.width) - text_width,
            (region.y + region.height) - (bounds_height / chart.scaling) * 0.1 - text_size,
        );

        frame.fill_text(canvas::Text {
            content: text_content,
            position: text_position,
            size: text_size.into(),
            color: palette.background.base.text,
            font: style::AZERET_MONO,
            ..canvas::Text::default()
        });
    }
}

/// Draw volume profile on the left side of the chart
#[allow(clippy::too_many_arguments)]
pub fn draw_volume_profile(
    frame: &mut canvas::Frame,
    region: &Rectangle,
    kind: &ProfileKind,
    palette: &Extended,
    chart: &ViewState,
    heatmap_data: &HeatmapData,
    area_width: f32,
    basis: ChartBasis,
) {
    let (highest_exch, lowest_exch) = chart.price_range(region);

    // Convert to data::Price
    let highest = DataPrice::from_units(highest_exch.units);
    let lowest = DataPrice::from_units(lowest_exch.units);

    // Calculate time range based on profile kind
    let time_range = match kind {
        ProfileKind::VisibleRange => {
            let earliest = chart.x_to_interval(region.x);
            let latest = chart.x_to_interval(region.x + region.width);
            earliest..=latest
        }
        ProfileKind::FixedWindow { candles: datapoints } | ProfileKind::Fixed(datapoints) => {
            let basis_interval: u64 = match basis {
                ChartBasis::Time(timeframe) => timeframe.to_millis(),
                ChartBasis::Tick(_) => return,
            };

            let latest = chart
                .latest_x
                .min(chart.x_to_interval(region.x + region.width));
            let earliest = latest.saturating_sub((*datapoints as u64) * basis_interval);

            earliest..=latest
        }
    };

    let step = chart.tick_size;
    let step_as_price = DataPrice::from_units(step.units);

    let first_tick = lowest.round_to_side_step(false, step_as_price);
    let last_tick = highest.round_to_side_step(true, step_as_price);

    let num_ticks = match exchange::util::Price::steps_between_inclusive(
        exchange::util::Price::from_units(first_tick.to_units()),
        exchange::util::Price::from_units(last_tick.to_units()),
        step,
    ) {
        Some(n) => n,
        None => return,
    };

    if num_ticks > 4096 {
        return;
    }

    // Draw background gradient
    let min_segment_width = 2.0;
    let segments = ((area_width / min_segment_width).floor() as usize).clamp(10, 40);

    for i in 0..segments {
        let segment_width = area_width / segments as f32;
        let segment_x = region.x + (i as f32 * segment_width);

        let alpha = 0.95 - (0.85 * (i as f32 / (segments - 1) as f32).powf(2.0));

        frame.fill_rectangle(
            Point::new(segment_x, region.y),
            Size::new(segment_width, region.height),
            palette.background.weakest.color.scale_alpha(alpha),
        );
    }

    // Build volume profile
    let mut profile = vec![(0.0f32, 0.0f32); num_ticks];
    let mut max_aggr_volume = 0.0f32;

    heatmap_data
        .trades_by_time
        .range(time_range)
        .for_each(|(_, dp)| {
            dp.grouped_trades
                .iter()
                .filter(|trade| {
                    let trade_price: DataPrice = trade.price;
                    trade_price >= lowest && trade_price <= highest
                })
                .for_each(|trade| {
                    let grouped_price = if trade.is_sell {
                        trade.price.round_to_side_step(true, step_as_price)
                    } else {
                        trade.price.round_to_side_step(false, step_as_price)
                    };

                    let first_tick_price: DataPrice = first_tick;
                    let last_tick_price: DataPrice = last_tick;
                    let grouped_price_units = grouped_price.to_units();

                    if grouped_price_units < first_tick_price.to_units()
                        || grouped_price_units > last_tick_price.to_units()
                    {
                        return;
                    }

                    let index = ((grouped_price_units - first_tick_price.to_units())
                        / step.units) as usize;

                    if let Some(entry) = profile.get_mut(index) {
                        if trade.is_sell {
                            entry.1 += trade.qty;
                        } else {
                            entry.0 += trade.qty;
                        }
                        max_aggr_volume = max_aggr_volume.max(entry.0 + entry.1);
                    }
                });
        });

    // Draw volume bars
    profile
        .iter()
        .enumerate()
        .for_each(|(index, (buy_v, sell_v))| {
            if *buy_v > 0.0 || *sell_v > 0.0 {
                let price: DataPrice = first_tick;
                let price = price.add_steps(index as i64, step_as_price);
                let y_position = chart.price_to_y(exchange::util::Price::from_units(price.to_units()));

                let next_price = price.add_steps(1, step_as_price);
                let next_y_position = chart.price_to_y(exchange::util::Price::from_units(next_price.to_units()));
                let bar_height = (next_y_position - y_position).abs();

                crate::chart::draw_volume_bar(
                    frame,
                    region.x,
                    y_position,
                    *buy_v,
                    *sell_v,
                    max_aggr_volume,
                    area_width,
                    bar_height,
                    palette.success.weak.color,
                    palette.danger.weak.color,
                    1.0,
                    true,
                );
            }
        });

    // Draw max volume label
    if max_aggr_volume > 0.0 {
        let text_size = 9.0 / chart.scaling;
        let text_content = abbr_large_numbers(max_aggr_volume);

        let text_position = Point::new(region.x + area_width, region.y);

        frame.fill_text(canvas::Text {
            content: text_content,
            position: text_position,
            size: iced::Pixels(text_size),
            color: palette.background.base.text,
            font: style::AZERET_MONO,
            ..canvas::Text::default()
        });
    }
}
