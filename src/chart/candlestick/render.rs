use crate::chart::drawing;
use crate::chart::indicator::kline::OverlayLine;
use crate::chart::indicator::plot::{AnySeries, Series};
use crate::chart::perf::{LodCalculator, LodIteratorExt};
use crate::chart::{Chart, ChartState, Interaction, Message, TEXT_SIZE, ViewState};
use crate::components::primitives::AZERET_MONO;
use data::state::pane::CandleStyle;
use data::util::count_decimals;
use data::{Candle, ChartBasis, FootprintType, Trade};
use exchange::FuturesTickerInfo;
use exchange::util::Price;
use iced::theme::palette::Extended;
use iced::widget::canvas::{self, Event, Geometry, Path, Stroke};
use iced::{Point, Rectangle, Renderer, Theme, Vector, mouse};

use super::KlineChart;
use super::candle::draw_candle;
use super::footprint::{ContentGaps, draw_clusters, effective_cluster_qty, should_show_text};

const MAX_TEXT_SIZE: f32 = 14.0;
const TEXT_SIZE_PADDING: f32 = 2.0;
const FOOTPRINT_CANDLE_WIDTH_RATIO: f32 = 0.8;

impl canvas::Program<Message> for KlineChart {
    type State = ChartState;

    fn update(
        &self,
        state: &mut ChartState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        crate::chart::canvas_interaction(
            self,
            &mut state.interaction,
            event,
            bounds,
            cursor,
            &mut state.last_selection_click,
            &mut state.shift_held,
        )
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
            let (earliest, latest) = chart.interval_range(&region);

            let price_to_y = |price: Price| chart.price_to_y(price);
            let interval_to_x = |interval| chart.interval_to_x(interval);

            // Calculate LOD level for adaptive rendering quality
            let visible_candle_count = match &self.basis {
                ChartBasis::Time(_) => self
                    .chart_data
                    .candles
                    .iter()
                    .filter(|c| c.time.0 >= earliest && c.time.0 <= latest)
                    .count(),
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

            if let Some(fp_config) = &self.footprint {
                // Ensure footprint cache is populated for visible range
                let candle_count = self.chart_data.candles.len();
                let (first_candle_idx, last_candle_idx) = match &self.basis {
                    ChartBasis::Time(_) => {
                        let first = self
                            .chart_data
                            .candles
                            .iter()
                            .position(|c| c.time.0 >= earliest)
                            .unwrap_or(0);
                        let last = self
                            .chart_data
                            .candles
                            .iter()
                            .rposition(|c| c.time.0 <= latest)
                            .map(|i| i + 1)
                            .unwrap_or(candle_count);
                        (first, last)
                    }
                    ChartBasis::Tick(_) => {
                        let ea = earliest as usize;
                        let la = latest as usize;
                        let start = candle_count.saturating_sub(la + 1);
                        let end = candle_count.saturating_sub(ea);
                        (start, end)
                    }
                };

                self.footprint_cache.borrow_mut().ensure_range(
                    first_candle_idx,
                    last_candle_idx,
                    &self.chart_data.candles,
                    &self.chart_data.trades,
                    chart.tick_size,
                    &self.basis,
                );

                let max_cluster_qty = self.calc_qty_scales_from_cache(
                    first_candle_idx,
                    last_candle_idx.saturating_sub(1),
                    fp_config.study_type,
                );

                let cell_height_unscaled = chart.cell_height * chart.scaling;
                let cell_width_unscaled = chart.cell_width * chart.scaling;

                let text_size = {
                    let text_size_from_height =
                        cell_height_unscaled.round().min(MAX_TEXT_SIZE) - TEXT_SIZE_PADDING;
                    let text_size_from_width = (cell_width_unscaled * FOOTPRINT_CANDLE_WIDTH_RATIO)
                        .round()
                        .min(MAX_TEXT_SIZE)
                        - TEXT_SIZE_PADDING;
                    text_size_from_height.min(text_size_from_width)
                };

                let candle_width = FOOTPRINT_CANDLE_WIDTH_RATIO * chart.cell_width;
                let content_spacing = ContentGaps::from_view(candle_width, chart.scaling);

                let show_text = {
                    let min_w = match fp_config.study_type {
                        FootprintType::Volume | FootprintType::Delta => 80.0,
                        FootprintType::BidAskSplit | FootprintType::DeltaAndVolume => 120.0,
                    };
                    lod_level.show_text()
                        && should_show_text(cell_height_unscaled, cell_width_unscaled, min_w)
                };

                // Render footprint from cache
                let cache = self.footprint_cache.borrow();
                match &self.basis {
                    ChartBasis::Tick(_) => {
                        let earliest_idx = earliest as usize;
                        let latest_idx = latest as usize;
                        self.chart_data
                            .candles
                            .iter()
                            .rev()
                            .enumerate()
                            .filter(|(i, _)| *i <= latest_idx && *i >= earliest_idx)
                            .for_each(|(index, candle)| {
                                let x_position = interval_to_x(index as u64);
                                let candle_idx = candle_count - 1 - index;
                                if let Some(footprint) = cache.get(candle_idx) {
                                    let cluster_scaling = effective_cluster_qty(
                                        fp_config.scaling,
                                        max_cluster_qty,
                                        footprint,
                                        fp_config.study_type,
                                    );
                                    draw_clusters(
                                        frame,
                                        price_to_y,
                                        x_position,
                                        chart.cell_width,
                                        chart.cell_height,
                                        candle_width,
                                        cluster_scaling,
                                        palette,
                                        text_size,
                                        self.tick_size(),
                                        show_text,
                                        candle,
                                        footprint,
                                        fp_config.study_type,
                                        fp_config.scaling,
                                        fp_config.candle_position,
                                        fp_config.mode,
                                        content_spacing,
                                    );
                                }
                            });
                    }
                    ChartBasis::Time(_) => {
                        if latest >= earliest {
                            self.chart_data
                                .candles
                                .iter()
                                .enumerate()
                                .filter(|(_, c)| c.time.0 >= earliest && c.time.0 <= latest)
                                .for_each(|(candle_idx, candle)| {
                                    let x_position = interval_to_x(candle.time.0);
                                    if let Some(footprint) = cache.get(candle_idx) {
                                        let cluster_scaling = effective_cluster_qty(
                                            fp_config.scaling,
                                            max_cluster_qty,
                                            footprint,
                                            fp_config.study_type,
                                        );
                                        draw_clusters(
                                            frame,
                                            price_to_y,
                                            x_position,
                                            chart.cell_width,
                                            chart.cell_height,
                                            candle_width,
                                            cluster_scaling,
                                            palette,
                                            text_size,
                                            self.tick_size(),
                                            show_text,
                                            candle,
                                            footprint,
                                            fp_config.study_type,
                                            fp_config.scaling,
                                            fp_config.candle_position,
                                            fp_config.mode,
                                            content_spacing,
                                        );
                                    }
                                });
                        }
                    }
                }
            } else {
                let candle_width = chart.cell_width * 0.8;
                let interval_ms = match &self.basis {
                    ChartBasis::Time(tf) => tf.to_milliseconds(),
                    ChartBasis::Tick(_) => 1000,
                };
                let style = &self.candle_style;

                let decimation = lod_level.decimation_factor();

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
                        draw_candle(
                            frame,
                            price_to_y,
                            candle_width,
                            palette,
                            style,
                            x_position,
                            candle,
                        );
                    },
                );
            }

            // Draw overlay indicators (SMA, EMA, Bollinger Bands)
            for (_kind, indi_opt) in &self.indicators {
                if let Some(indi) = indi_opt {
                    let lines = indi.overlay_lines();
                    for line in &lines {
                        draw_overlay_line(frame, chart, self.basis, line, earliest, latest);
                    }
                }
            }

            // Render overlay studies (Big Trades bubbles, etc.)
            for study in &self.studies {
                let output = study.output();
                if matches!(study.placement(), study::StudyPlacement::Overlay)
                    && !matches!(output, study::StudyOutput::Empty)
                {
                    let bubble_scale =
                        study.config().get_float("bubble_scale", 1.0) as f32;
                    crate::chart::study_renderer::render_study_output(
                        frame,
                        output,
                        chart,
                        bounds_size,
                        study.placement(),
                        bubble_scale,
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
            Interaction::None | Interaction::Ruler { .. } => {
                if cursor.is_over(bounds) {
                    mouse::Interaction::Crosshair
                } else {
                    mouse::Interaction::default()
                }
            }
        }
    }
}

/// Draw a single overlay indicator line on the main chart canvas.
///
/// Converts f32 price values → exchange `Price` → chart Y coordinates,
/// using the same transforms as candle rendering.
fn draw_overlay_line(
    frame: &mut canvas::Frame,
    chart: &ViewState,
    basis: ChartBasis,
    line: &OverlayLine<'_>,
    earliest: u64,
    latest: u64,
) {
    if line.data.is_empty() {
        return;
    }

    let stroke = Stroke::with_color(
        Stroke {
            width: line.stroke_width,
            ..Stroke::default()
        },
        line.color,
    );

    let series = AnySeries::for_basis(basis, line.data);
    let mut prev: Option<(f32, f32)> = None;

    series.for_each_in(earliest..=latest, |x, value| {
        let sx = chart.interval_to_x(x) - (chart.cell_width / 2.0);
        let price = Price::from_f32(*value);
        let sy = chart.price_to_y(price);

        if let Some((px, py)) = prev {
            frame.stroke(&Path::line(Point::new(px, py), Point::new(sx, sy)), stroke);
        }
        prev = Some((sx, sy));
    });
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

                    // Get trades for this candle by time range using binary search
                    let candle_start = candle.time.0;
                    let candle_end = candle.time.0 + interval_ms;

                    // Find start index using binary search
                    let start_idx = trades
                        .binary_search_by_key(&candle_start, |t| t.time.0)
                        .unwrap_or_else(|i| i);

                    // Find end index using binary search on the remaining slice
                    let end_idx = trades[start_idx..]
                        .binary_search_by_key(&candle_end, |t| t.time.0)
                        .map(|i| start_idx + i)
                        .unwrap_or_else(|i| start_idx + i);

                    let candle_trades = &trades[start_idx..end_idx];

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

                    // Get trades for this candle by time range using binary search
                    let candle_start = candle.time.0;
                    let candle_end = candle.time.0 + interval_ms;

                    // Find start index using binary search
                    let start_idx = trades
                        .binary_search_by_key(&candle_start, |t| t.time.0)
                        .unwrap_or_else(|i| i);

                    // Find end index using binary search on the remaining slice
                    let end_idx = trades[start_idx..]
                        .binary_search_by_key(&candle_end, |t| t.time.0)
                        .map(|i| start_idx + i)
                        .unwrap_or_else(|i| start_idx + i);

                    let candle_trades = &trades[start_idx..end_idx];

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
            .iter()
            .find(|c| c.time.0 == at_interval)
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
                .unwrap_or(palette.success.base.color)
        } else {
            candle_style
                .bear_body_color
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

        let position = Point::new(8.0, 8.0);

        let tooltip_rect = Rectangle {
            x: position.x,
            y: position.y,
            width: total_width,
            height: 16.0,
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
                size: iced::Pixels(12.0),
                color: seg_color,
                font: AZERET_MONO,
                ..canvas::Text::default()
            });
            x += text.len() as f32 * 8.0;
            x += if is_value { 6.0 } else { 2.0 };
        }
    }
}
