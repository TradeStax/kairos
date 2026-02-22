use crate::chart::core::tokens;
use crate::chart::drawing;
use crate::chart::perf::{LodCalculator, LodIteratorExt};
use crate::chart::{Chart, ChartState, Interaction, Message, TEXT_SIZE};
use crate::components::primitives::AZERET_MONO;
use data::state::pane::CandleStyle;
use data::util::count_decimals;
use data::{Candle, ChartBasis, Trade};
use exchange::FuturesTickerInfo;
use exchange::util::Price;
use iced::theme::palette::Extended;
use iced::widget::canvas::{self, Event, Geometry};
use iced::{Color, Point, Rectangle, Renderer, Theme, Vector, mouse};
use std::time::Instant;

use super::KlineChart;
use super::candle::draw_candle;

impl canvas::Program<Message> for KlineChart {
    type State = ChartState;

    fn update(
        &self,
        state: &mut ChartState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        crate::chart::canvas_interaction(self, state, event, bounds, cursor)
    }

    fn draw(
        &self,
        state: &ChartState,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let interaction = &state.interaction;
        let chart = self.state();

        if bounds.width == 0.0 {
            return vec![];
        }

        let bounds_size = bounds.size();
        let palette = theme.extended_palette();

        let klines = chart.cache.main.draw(renderer, bounds_size, |frame| {
            let center = Vector::new(bounds.width / 2.0, bounds.height / 2.0);

            frame.translate(center);
            frame.scale(chart.scaling);
            frame.translate(chart.translation);

            let region = chart.visible_region(frame.size());

            // Draw grid lines behind all content
            crate::chart::overlay::draw_price_grid(chart, frame, palette, &region);
            crate::chart::overlay::draw_time_grid(chart, frame, palette, &region);

            let (earliest, latest) = chart.interval_range(&region);

            let price_to_y = |price: Price| chart.price_to_y(price);
            let interval_to_x = |interval| chart.interval_to_x(interval);

            // Calculate LOD level for adaptive rendering quality
            let visible_candle_count = match &self.basis {
                ChartBasis::Time(_) => {
                    let first = self
                        .chart_data
                        .candles
                        .partition_point(|c| c.time.0 < earliest);
                    let last = self
                        .chart_data
                        .candles
                        .partition_point(|c| c.time.0 <= latest);
                    last.saturating_sub(first)
                }
                ChartBasis::Tick(_) => {
                    let ea = earliest as usize;
                    let la = latest as usize;
                    la.saturating_sub(ea) + 1
                }
            };
            let lod = LodCalculator::new(
                chart.scaling,
                chart.cell_width,
                visible_candle_count,
                bounds.width,
            );
            let lod_level = lod.calculate_lod();

            // When zoomed out past a CandleReplace study's max_bars,
            // fall back to normal candle rendering instead.
            let candle_replace_fallback =
                self.has_candle_replace()
                    && self.studies.iter().any(|s| {
                        s.placement()
                            == study::StudyPlacement::CandleReplace
                            && match s.output() {
                                study::StudyOutput::Footprint(d) => {
                                    visible_candle_count
                                        > d.max_bars_to_show
                                }
                                _ => false,
                            }
                    });

            if !self.has_candle_replace()
                || candle_replace_fallback
            {
                // Standard candle rendering
                let candle_width = chart.cell_width * tokens::candle::WIDTH_RATIO;
                let interval_ms = match &self.basis {
                    ChartBasis::Time(tf) => tf.to_milliseconds(),
                    ChartBasis::Tick(_) => 1000,
                };
                let style = &self.candle_style;

                let decimation = lod_level.decimation_factor();

                // Pre-compute max volume for volume-based opacity
                let max_volume = if style.volume_opacity {
                    self.chart_data
                        .candles
                        .iter()
                        .map(|c| c.volume())
                        .fold(0.0_f32, f32::max)
                        .max(1.0)
                } else {
                    1.0
                };
                let use_volume_opacity = style.volume_opacity;

                render_candles(
                    &self.chart_data.candles,
                    &self.chart_data.trades,
                    &self.basis,
                    chart.tick_size,
                    interval_ms,
                    frame,
                    earliest,
                    latest,
                    decimation,
                    interval_to_x,
                    |frame, x_position, candle, _| {
                        let vol_ratio = if use_volume_opacity {
                            Some(candle.volume() / max_volume)
                        } else {
                            None
                        };
                        draw_candle(
                            frame,
                            price_to_y,
                            candle_width,
                            palette,
                            style,
                            x_position,
                            candle,
                            vol_ratio,
                        );
                    },
                );
            }

            // Render overlay, background, and CandleReplace studies
            for study in &self.studies {
                let output = study.output();
                let placement = study.placement();
                // Skip CandleReplace when in fallback mode
                let skip = candle_replace_fallback
                    && placement
                        == study::StudyPlacement::CandleReplace;
                if !skip
                    && !matches!(output, study::StudyOutput::Empty)
                    && matches!(
                        placement,
                        study::StudyPlacement::Overlay
                            | study::StudyPlacement::Background
                            | study::StudyPlacement::CandleReplace
                    )
                {
                    crate::chart::study_renderer::render_study_output(
                        frame,
                        output,
                        chart,
                        bounds_size,
                        placement,
                        Some(palette),
                    );
                }
            }

            crate::chart::overlay::draw_last_price_line(chart, frame, palette, region);

            // Draw data gap markers
            if !self.chart_data.gaps.is_empty() {
                crate::chart::overlay::draw_gap_markers(
                    frame,
                    chart,
                    &self.chart_data.gaps,
                    &region,
                );
            }
        });

        // Drawings cache layer - completed drawings only
        let drawings_layer = chart.cache.drawings.draw(renderer, bounds_size, |frame| {
            drawing::render::draw_completed_drawings(
                frame,
                chart,
                &self.drawings,
                bounds_size,
                palette,
            );
        });

        // Track frame timing for debug overlay
        if self.show_debug_info {
            let now = Instant::now();
            let elapsed = now
                .duration_since(self.last_draw_instant.get())
                .as_secs_f32()
                * 1000.0;
            // Smooth frame time with exponential moving average
            let prev = self.last_frame_time_ms.get();
            let smoothed = if prev == 0.0 {
                elapsed
            } else {
                prev * 0.8 + elapsed * 0.2
            };
            self.last_frame_time_ms.set(smoothed);
            self.last_draw_instant.set(now);
        }

        let crosshair = chart.cache.crosshair.draw(renderer, bounds_size, |frame| {
            // Draw overlay elements (selection handles + pending preview)
            drawing::render::draw_overlay_drawings(
                frame,
                chart,
                &self.drawings,
                bounds_size,
                palette,
            );

            if let Some(cursor_position) = cursor.position_in(bounds) {
                // Draw ruler if active
                if let Interaction::Ruler { start: Some(start) } = interaction {
                    crate::chart::overlay::draw_ruler(
                        chart,
                        frame,
                        palette,
                        bounds_size,
                        *start,
                        cursor_position,
                    );
                }

                // Draw crosshair
                let result = crate::chart::overlay::draw_crosshair(
                    chart,
                    frame,
                    theme,
                    bounds_size,
                    cursor_position,
                    interaction,
                );

                draw_crosshair_tooltip(
                    &self.chart_data.candles,
                    &self.basis,
                    &self.ticker_info,
                    frame,
                    palette,
                    result.interval,
                    &self.candle_style,
                );
            }

            // Debug performance overlay
            if self.show_debug_info {
                draw_debug_overlay(
                    frame,
                    chart,
                    bounds,
                    palette,
                    self.last_frame_time_ms.get(),
                    &self.chart_data.candles,
                    &self.basis,
                );
            }
        });

        vec![klines, drawings_layer, crosshair]
    }

    fn mouse_interaction(
        &self,
        state: &ChartState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match &state.interaction {
            Interaction::Panning { .. } => mouse::Interaction::Grabbing,
            Interaction::Zoomin { .. } => mouse::Interaction::ZoomIn,
            Interaction::Drawing { .. } | Interaction::PlacingClone => {
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
            Interaction::None | Interaction::Ruler { .. } | Interaction::Decelerating { .. } => {
                if let Some(cursor_position) = cursor.position_in(bounds) {
                    // Check if hovering over a drawing handle
                    if self
                        .hit_test_drawing_handle(cursor_position, bounds.size())
                        .is_some()
                    {
                        mouse::Interaction::Grab
                    } else if self
                        .hit_test_drawing(cursor_position, bounds.size())
                        .is_some()
                    {
                        mouse::Interaction::Pointer
                    } else {
                        mouse::Interaction::Crosshair
                    }
                } else {
                    mouse::Interaction::default()
                }
            }
        }
    }
}

/// Return the slice of trades whose timestamp falls in `[start_ts, end_ts]`.
fn trades_for_candle<'t>(trades: &'t [Trade], start_ts: u64, end_ts: u64) -> &'t [Trade] {
    let lo = trades.partition_point(|t| t.time.0 < start_ts);
    let hi = trades.partition_point(|t| t.time.0 <= end_ts);
    &trades[lo..hi]
}

fn render_candles<F>(
    candles: &[Candle],
    trades: &[Trade],
    basis: &ChartBasis,
    _tick_size: exchange::util::PriceStep,
    interval_ms: u64,
    frame: &mut canvas::Frame,
    earliest: u64,
    latest: u64,
    decimation: usize,
    interval_to_x: impl Fn(u64) -> f32,
    draw_fn: F,
) where
    F: Fn(&mut canvas::Frame, f32, &Candle, &[Trade]),
{
    match basis {
        ChartBasis::Tick(_) => {
            let earliest_idx = earliest as usize;
            let latest_idx = latest as usize;

            candles
                .iter()
                .rev()
                .enumerate()
                .filter(|(index, _)| *index <= latest_idx && *index >= earliest_idx)
                .lod_filter(decimation)
                .for_each(|(index, candle)| {
                    let x_position = interval_to_x(index as u64);

                    let candle_trades =
                        trades_for_candle(trades, candle.time.0, candle.time.0 + interval_ms);

                    draw_fn(frame, x_position, candle, candle_trades);
                });
        }
        ChartBasis::Time(_) => {
            if latest < earliest {
                return;
            }

            candles
                .iter()
                .filter(|c| c.time.0 >= earliest && c.time.0 <= latest)
                .lod_filter(decimation)
                .for_each(|candle| {
                    let x_position = interval_to_x(candle.time.0);

                    let candle_trades =
                        trades_for_candle(trades, candle.time.0, candle.time.0 + interval_ms);

                    draw_fn(frame, x_position, candle, candle_trades);
                });
        }
    }
}

fn draw_crosshair_tooltip(
    candles: &[Candle],
    basis: &ChartBasis,
    ticker_info: &FuturesTickerInfo,
    frame: &mut canvas::Frame,
    palette: &Extended,
    at_interval: u64,
    candle_style: &CandleStyle,
) {
    let candle_opt = match basis {
        ChartBasis::Time(_) => candles
            .binary_search_by_key(&at_interval, |c| c.time.0)
            .ok()
            .map(|i| &candles[i])
            .or_else(|| {
                if candles.is_empty() {
                    None
                } else {
                    let last = candles.last()?;
                    if at_interval > last.time.0 {
                        Some(last)
                    } else {
                        None
                    }
                }
            }),
        ChartBasis::Tick(tick_count) => {
            let index = (at_interval / u64::from(*tick_count)) as usize;
            if index < candles.len() {
                Some(&candles[candles.len() - 1 - index])
            } else {
                None
            }
        }
    };

    if let Some(candle) = candle_opt {
        let change_pct =
            ((candle.close.to_f32() - candle.open.to_f32()) / candle.open.to_f32()) * 100.0;
        let change_color = if change_pct >= 0.0 {
            candle_style
                .bull_body_color
                .map(crate::style::theme_bridge::rgba_to_iced_color)
                .unwrap_or(palette.success.base.color)
        } else {
            candle_style
                .bear_body_color
                .map(crate::style::theme_bridge::rgba_to_iced_color)
                .unwrap_or(palette.danger.base.color)
        };

        let base_color = palette.background.base.text;
        let precision = count_decimals(ticker_info.tick_size);

        let open_str = format!("{:.prec$}", candle.open.to_f32(), prec = precision);
        let high_str = format!("{:.prec$}", candle.high.to_f32(), prec = precision);
        let low_str = format!("{:.prec$}", candle.low.to_f32(), prec = precision);
        let close_str = format!("{:.prec$}", candle.close.to_f32(), prec = precision);
        let pct_str = format!("{change_pct:+.2}%");

        let segments = [
            ("O", base_color, false),
            (&open_str, change_color, true),
            ("H", base_color, false),
            (&high_str, change_color, true),
            ("L", base_color, false),
            (&low_str, change_color, true),
            ("C", base_color, false),
            (&close_str, change_color, true),
            (&pct_str, change_color, true),
        ];

        let total_width: f32 = segments
            .iter()
            .map(|(s, _, _)| s.len() as f32 * (TEXT_SIZE * 0.8))
            .sum();

        let char_width = TEXT_SIZE * 0.8;
        let position = Point::new(
            crate::style::tokens::spacing::MD,
            crate::style::tokens::spacing::MD,
        );

        let tooltip_rect = Rectangle {
            x: position.x,
            y: position.y,
            width: total_width,
            height: crate::style::tokens::text::HEADING,
        };

        frame.fill_rectangle(
            tooltip_rect.position(),
            tooltip_rect.size(),
            palette.background.weakest.color.scale_alpha(0.9),
        );

        let mut x = position.x;
        for (text, seg_color, is_value) in segments {
            frame.fill_text(canvas::Text {
                content: text.to_string(),
                position: Point::new(x, position.y),
                size: iced::Pixels(TEXT_SIZE),
                color: seg_color,
                font: AZERET_MONO,
                ..canvas::Text::default()
            });
            x += text.len() as f32 * char_width;
            x += if is_value {
                crate::style::tokens::spacing::SM
            } else {
                crate::style::tokens::spacing::XXS
            };
        }
    }
}

fn draw_debug_overlay(
    frame: &mut canvas::Frame,
    chart: &crate::chart::ViewState,
    bounds: Rectangle,
    _palette: &Extended,
    frame_time_ms: f32,
    candles: &[Candle],
    basis: &ChartBasis,
) {
    let region = chart.visible_region(bounds.size());
    let (earliest, latest) = chart.interval_range(&region);

    let visible_count = match basis {
        ChartBasis::Time(_) => {
            let first = candles.partition_point(|c| c.time.0 < earliest);
            let last = candles.partition_point(|c| c.time.0 <= latest);
            last.saturating_sub(first)
        }
        ChartBasis::Tick(_) => {
            let ea = earliest as usize;
            let la = latest as usize;
            la.saturating_sub(ea) + 1
        }
    };

    let lod = LodCalculator::new(
        chart.scaling,
        chart.cell_width,
        visible_count,
        bounds.width,
    );
    let lod_level = lod.calculate_lod();

    let fps = if frame_time_ms > 0.0 {
        1000.0 / frame_time_ms
    } else {
        0.0
    };

    let lines = [
        format!("FPS: {:.0}", fps),
        format!("Frame: {:.1}ms", frame_time_ms),
        format!("Candles: {} vis / {} total", visible_count, candles.len()),
        format!("LOD: {:?} (dec {}x)", lod_level, lod_level.decimation_factor()),
        format!("Zoom: {:.2}x  Cell: {:.1}px", chart.scaling, chart.cell_width),
    ];

    let line_height = 14.0_f32;
    let padding = 6.0_f32;
    let box_width = 220.0_f32;
    let box_height = (lines.len() as f32 * line_height) + padding * 2.0;
    let box_x = bounds.width - box_width - 8.0;
    let box_y = bounds.height - box_height - 8.0;

    // Background
    frame.fill_rectangle(
        Point::new(box_x, box_y),
        iced::Size::new(box_width, box_height),
        Color::from_rgba(0.0, 0.0, 0.0, 0.75),
    );

    // Text lines
    let label_color = Color::from_rgba(0.6, 0.9, 0.6, 0.9);
    for (i, line) in lines.iter().enumerate() {
        frame.fill_text(canvas::Text {
            content: line.clone(),
            position: Point::new(
                box_x + padding,
                box_y + padding + (i as f32 * line_height),
            ),
            size: iced::Pixels(11.0),
            color: label_color,
            font: AZERET_MONO,
            ..canvas::Text::default()
        });
    }
}
