use crate::chart::{Chart, Interaction, Message, TEXT_SIZE, ViewState};
use crate::style;
use data::util::count_decimals;
use data::{Candle, ChartBasis, ClusterKind, FootprintStudy, KlineChartKind, Trade};
use exchange::FuturesTickerInfo;
use exchange::util::Price;
use iced::theme::palette::Extended;
use iced::widget::canvas::{self, Event, Geometry};
use iced::{Point, Rectangle, Renderer, Theme, Vector, mouse};
use std::collections::BTreeMap;

use super::candle::draw_candle;
use super::footprint::{ContentGaps, draw_all_npocs, draw_clusters, effective_cluster_qty, should_show_text};
use super::{KlineChart, TradeGroup, domain_to_exchange_price};

impl canvas::Program<Message> for KlineChart {
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

        let klines = chart.cache.main.draw(renderer, bounds_size, |frame| {
            let center = Vector::new(bounds.width / 2.0, bounds.height / 2.0);

            frame.translate(center);
            frame.scale(chart.scaling);
            frame.translate(chart.translation);

            let region = chart.visible_region(frame.size());
            let (earliest, latest) = chart.interval_range(&region);

            let price_to_y = |price: Price| chart.price_to_y(price);
            let interval_to_x = |interval| chart.interval_to_x(interval);

            match &self.kind {
                KlineChartKind::Footprint {
                    clusters,
                    scaling,
                    studies,
                } => {
                    let (highest, lowest) = chart.price_range(&region);

                    let max_cluster_qty = self.calc_qty_scales(
                        earliest,
                        latest,
                        highest,
                        lowest,
                        chart.tick_size,
                        *clusters,
                    );

                    let cell_height_unscaled = chart.cell_height * chart.scaling;
                    let cell_width_unscaled = chart.cell_width * chart.scaling;

                    let text_size = {
                        let text_size_from_height = cell_height_unscaled.round().min(16.0) - 3.0;
                        let text_size_from_width =
                            (cell_width_unscaled * 0.1).round().min(16.0) - 3.0;

                        text_size_from_height.min(text_size_from_width)
                    };

                    let candle_width = 0.1 * chart.cell_width;
                    let content_spacing = ContentGaps::from_view(candle_width, chart.scaling);

                    let imbalance = studies.iter().find_map(|study| {
                        if let FootprintStudy::Imbalance {
                            threshold,
                            color_scale,
                            ignore_zeros,
                        } = study
                        {
                            Some((*threshold as usize, if *color_scale { Some(0) } else { None }, *ignore_zeros))
                        } else {
                            None
                        }
                    });

                    let show_text = {
                        let min_w = match clusters {
                            ClusterKind::VolumeProfile | ClusterKind::DeltaProfile => 80.0,
                            ClusterKind::BidAsk => 120.0,
                            ClusterKind::Delta | ClusterKind::Volume | ClusterKind::Trades => 120.0,
                        };
                        should_show_text(cell_height_unscaled, cell_width_unscaled, min_w)
                    };

                    // Draw nPOCs first (if study is enabled)
                    if let Some(lookback) = studies.iter().find_map(|study| {
                        if let FootprintStudy::NPoC { lookback } = study {
                            Some(*lookback)
                        } else {
                            None
                        }
                    }) {
                        draw_all_npocs(
                            &self.chart_data.candles,
                            &self.chart_data.trades,
                            &self.basis,
                            frame,
                            &price_to_y,
                            &interval_to_x,
                            candle_width,
                            chart.cell_width,
                            chart.cell_height,
                            chart.tick_size,
                            palette,
                            lookback,
                            earliest,
                            latest,
                            *clusters,
                            content_spacing,
                            imbalance.is_some(),
                        );
                    }

                    // Draw candles and footprint
                    let interval_ms = match &self.basis {
                        ChartBasis::Time(tf) => tf.to_milliseconds(),
                        ChartBasis::Tick(_) => 1000, // default estimate
                    };

                    render_candles(
                        &self.chart_data.candles,
                        &self.chart_data.trades,
                        &self.basis,
                        chart.tick_size,
                        interval_ms,
                        frame,
                        earliest,
                        latest,
                        interval_to_x,
                        |frame, x_position, candle, trades| {
                            let footprint = self.build_footprint(trades, highest, lowest);

                            let cluster_scaling =
                                effective_cluster_qty(*scaling, max_cluster_qty, &footprint, *clusters);

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
                                imbalance,
                                candle,
                                &footprint,
                                *clusters,
                                content_spacing,
                            );
                        },
                    );
                }
                KlineChartKind::Candles => {
                    let candle_width = chart.cell_width * 0.8;
                    let interval_ms = match &self.basis {
                        ChartBasis::Time(tf) => tf.to_milliseconds(),
                        ChartBasis::Tick(_) => 1000,
                    };

                    render_candles(
                        &self.chart_data.candles,
                        &self.chart_data.trades,
                        &self.basis,
                        chart.tick_size,
                        interval_ms,
                        frame,
                        earliest,
                        latest,
                        interval_to_x,
                        |frame, x_position, candle, _| {
                            draw_candle(
                                frame,
                                price_to_y,
                                candle_width,
                                palette,
                                x_position,
                                candle,
                            );
                        },
                    );
                }
            }

            crate::chart::overlay::draw_last_price_line(chart, frame, palette, region);
        });

        let crosshair = chart.cache.crosshair.draw(renderer, bounds_size, |frame| {
            if let Some(cursor_position) = cursor.position_in(bounds) {
                // Draw ruler if active
                if let Interaction::Ruler { start: Some(start) } = interaction {
                    crate::chart::overlay::draw_ruler(chart, frame, palette, bounds_size, *start, cursor_position);
                }

                // Draw crosshair
                let result = crate::chart::overlay::draw_crosshair(chart, frame, theme, bounds_size, cursor_position, interaction);

                draw_crosshair_tooltip(
                    &self.chart_data.candles,
                    &self.basis,
                    &self.ticker_info,
                    frame,
                    palette,
                    result.interval,
                );
            }
        });

        vec![klines, crosshair]
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

fn render_candles<F>(
    candles: &[Candle],
    trades: &[Trade],
    basis: &ChartBasis,
    _tick_size: exchange::util::PriceStep,
    interval_ms: u64,
    frame: &mut canvas::Frame,
    earliest: u64,
    latest: u64,
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
        let change_pct = ((candle.close.to_f32() - candle.open.to_f32()) / candle.open.to_f32()) * 100.0;
        let change_color = if change_pct >= 0.0 {
            palette.success.base.color
        } else {
            palette.danger.base.color
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
                font: style::AZERET_MONO,
                ..canvas::Text::default()
            });
            x += text.len() as f32 * 8.0;
            x += if is_value { 6.0 } else { 2.0 };
        }
    }
}
