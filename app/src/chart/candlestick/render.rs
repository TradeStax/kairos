use crate::chart::core::tokens;
use crate::chart::drawing;
use crate::chart::perf::{LodCalculator, LodIteratorExt};
use crate::chart::{Chart, ChartState, Interaction, Message, TEXT_SIZE, base_mouse_interaction};
use crate::components::primitives::AZERET_MONO;
use crate::screen::dashboard::pane::config::CandleStyle;
use data::FuturesTickerInfo;
use data::Price;
use data::util::count_decimals;
use data::{Candle, ChartBasis, Trade};
use iced::theme::palette::Extended;
use iced::widget::canvas::{self, Event, Geometry, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Theme, Vector, mouse};
use std::cell::RefCell;
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
            crate::chart::overlay::draw_date_separators(chart, frame, palette, &region);

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
            let candle_replace_fallback = self.has_candle_replace()
                && self.studies.iter().any(|s| {
                    s.metadata().placement == study::StudyPlacement::CandleReplace
                        && match s.output() {
                            study::StudyOutput::Footprint(d) => {
                                visible_candle_count > d.max_bars_to_show
                            }
                            _ => false,
                        }
                });

            if !self.has_candle_replace() || candle_replace_fallback {
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
                let placement = study.metadata().placement;
                // Skip CandleReplace when in fallback mode
                let skip =
                    candle_replace_fallback && placement == study::StudyPlacement::CandleReplace;
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

                chart.crosshair.interval.set(Some(result.interval));

                let has_candle = draw_crosshair_tooltip(
                    &self.chart_data.candles,
                    &self.basis,
                    &self.ticker_info,
                    frame,
                    palette,
                    result.interval,
                    &self.candle_style,
                );

                if has_candle && !self.studies.is_empty() {
                    let y_start =
                        crate::style::tokens::spacing::MD + crate::style::tokens::text::HEADING;
                    draw_study_overlay(
                        &self.studies,
                        result.interval,
                        frame,
                        palette,
                        y_start,
                        &self.study_overlay_rects,
                        &self.study_detail_button_rects,
                        state.selected_study_overlay,
                        cursor_position,
                    );
                }
            } else if let Some(interval) = chart.crosshair.interval.get() {
                // Crosshair driven by study panel cursor
                crate::chart::overlay::draw_remote_crosshair(
                    chart,
                    frame,
                    theme,
                    bounds_size,
                    interval,
                );
            } else if let Some(interval) = chart.crosshair.remote {
                // Remote crosshair from linked pane (only when local cursor absent)
                crate::chart::overlay::draw_remote_crosshair(
                    chart,
                    frame,
                    theme,
                    bounds_size,
                    interval,
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
        if let Some(i) = base_mouse_interaction(&state.interaction, bounds, cursor) {
            return i;
        }
        if let Some(cursor_position) = cursor.position_in(bounds) {
            // Check study detail buttons or overlay labels
            if self
                .study_detail_button_rects
                .borrow()
                .iter()
                .any(|(_, r)| r.contains(cursor_position))
                || self
                    .study_overlay_rects
                    .borrow()
                    .iter()
                    .any(|(_, r)| r.contains(cursor_position))
            {
                mouse::Interaction::Pointer
            // Check if hovering over a drawing handle
            } else if self
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

/// Return the slice of trades whose timestamp falls in `[start_ts, end_ts]`.
fn trades_for_candle(trades: &[Trade], start_ts: u64, end_ts: u64) -> &[Trade] {
    let lo = trades.partition_point(|t| t.time.0 < start_ts);
    let hi = trades.partition_point(|t| t.time.0 <= end_ts);
    &trades[lo..hi]
}

fn render_candles<F>(
    candles: &[Candle],
    trades: &[Trade],
    basis: &ChartBasis,
    _tick_size: data::PriceStep,
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
) -> bool {
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
                .map(crate::style::theme::rgba_to_iced_color)
                .unwrap_or(palette.success.base.color)
        } else {
            candle_style
                .bear_body_color
                .map(crate::style::theme::rgba_to_iced_color)
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
        return true;
    }
    false
}

/// Find the value at (or just before) the given interval in a
/// sorted `(u64, f32)` point series. O(log n) via binary search.
#[inline]
fn find_line_value_at(points: &[(u64, f32)], at: u64) -> Option<f32> {
    if points.is_empty() {
        return None;
    }
    let idx = points.partition_point(|p| p.0 < at);
    if idx < points.len() && points[idx].0 == at {
        return Some(points[idx].1);
    }
    if idx > 0 {
        return Some(points[idx - 1].1);
    }
    None
}

/// Emit a study line as two-color segments: label in `base_color`,
/// value in `value_color`. Advances `y` by `line_height`.
///
/// `label` is the study/series name (drawn first).
/// `value_str` is the formatted number(s) — if non-empty, draws
/// ` ` + value in the accent color after the label.
/// `alpha` dims both colors (1.0 = full, <1.0 = dimmed on hover).
#[inline]
fn emit_study_line(
    frame: &mut canvas::Frame,
    label: &str,
    value_str: &str,
    base_color: Color,
    value_color: Color,
    x: f32,
    y: &mut f32,
    line_height: f32,
    alpha: f32,
) {
    let char_width = TEXT_SIZE * 0.8;
    let mut cx = x;

    // Label segment (study name) in base color
    frame.fill_text(canvas::Text {
        content: label.to_string(),
        position: Point::new(cx, *y),
        size: iced::Pixels(TEXT_SIZE),
        color: base_color.scale_alpha(alpha),
        font: AZERET_MONO,
        ..canvas::Text::default()
    });
    cx += label.len() as f32 * char_width;

    // Value segment in study color
    if !value_str.is_empty() {
        cx += crate::style::tokens::spacing::XXS;
        frame.fill_text(canvas::Text {
            content: value_str.to_string(),
            position: Point::new(cx, *y),
            size: iced::Pixels(TEXT_SIZE),
            color: value_color.scale_alpha(alpha),
            font: AZERET_MONO,
            ..canvas::Text::default()
        });
    }

    *y += line_height;
}

/// Draw all text entries for a single `StudyOutput`, writing
/// directly to the frame without intermediate collection.
/// `buf` is cleared and reused for each line to minimise allocations.
///
/// Label text uses `base_color` (theme text); value numbers use the
/// study's own series color.
fn draw_output_entries(
    output: &study::StudyOutput,
    name: &str,
    at: u64,
    frame: &mut canvas::Frame,
    base_color: Color,
    x: f32,
    y: &mut f32,
    line_height: f32,
    buf: &mut String,
    alpha: f32,
) {
    use std::fmt::Write;

    match output {
        study::StudyOutput::Lines(series) => {
            for s in series {
                let value_color = crate::style::theme::rgba_to_iced_color(s.color);
                buf.clear();
                if let Some(v) = find_line_value_at(&s.points, at) {
                    let _ = write!(buf, "{:.2}", v);
                }
                emit_study_line(
                    frame,
                    &s.label,
                    buf,
                    base_color,
                    value_color,
                    x,
                    y,
                    line_height,
                    alpha,
                );
            }
        }
        study::StudyOutput::Band {
            upper,
            middle,
            lower,
            ..
        } => {
            let label = middle.as_ref().map(|m| m.label.as_str()).unwrap_or(name);
            let sc = middle.as_ref().map(|m| m.color).unwrap_or(upper.color);
            let value_color = crate::style::theme::rgba_to_iced_color(sc);

            let u = find_line_value_at(&upper.points, at);
            let m = middle
                .as_ref()
                .and_then(|ms| find_line_value_at(&ms.points, at));
            let l = find_line_value_at(&lower.points, at);

            buf.clear();
            match (u, m, l) {
                (Some(uv), Some(mv), Some(lv)) => {
                    let _ = write!(buf, "{:.2} / {:.2} / {:.2}", uv, mv, lv);
                }
                (Some(uv), None, Some(lv)) => {
                    let _ = write!(buf, "{:.2} / {:.2}", uv, lv);
                }
                _ => {}
            }
            emit_study_line(
                frame,
                label,
                buf,
                base_color,
                value_color,
                x,
                y,
                line_height,
                alpha,
            );
        }
        study::StudyOutput::Bars(series) => {
            for s in series {
                let value_color = s
                    .points
                    .first()
                    .map(|p| crate::style::theme::rgba_to_iced_color(p.color))
                    .unwrap_or(base_color);

                buf.clear();
                // Binary search: BarPoints are sorted by x (interval)
                let idx = s.points.partition_point(|p| p.x < at);
                let val = s.points.get(idx).filter(|p| p.x == at).map(|p| p.value);
                if let Some(v) = val {
                    let _ = write!(buf, "{:.2}", v);
                }
                emit_study_line(
                    frame,
                    &s.label,
                    buf,
                    base_color,
                    value_color,
                    x,
                    y,
                    line_height,
                    alpha,
                );
            }
        }
        study::StudyOutput::Histogram(bars) => {
            let value_color = bars
                .first()
                .map(|b| crate::style::theme::rgba_to_iced_color(b.color))
                .unwrap_or(base_color);

            buf.clear();
            let idx = bars.partition_point(|b| b.x < at);
            let val = if idx < bars.len() && bars[idx].x == at {
                Some(bars[idx].value)
            } else if idx > 0 {
                Some(bars[idx - 1].value)
            } else {
                None
            };
            if let Some(v) = val {
                let _ = write!(buf, "{:.2}", v);
            }
            emit_study_line(
                frame,
                name,
                buf,
                base_color,
                value_color,
                x,
                y,
                line_height,
                alpha,
            );
        }
        study::StudyOutput::Composite(sub_outputs) => {
            // If all sub-outputs are Levels, merge into a single
            // summary line instead of repeating the study name for
            // each session.
            let all_levels = !sub_outputs.is_empty()
                && sub_outputs
                    .iter()
                    .all(|s| matches!(s, study::StudyOutput::Levels(_)));
            if all_levels {
                let total: usize = sub_outputs
                    .iter()
                    .map(|s| match s {
                        study::StudyOutput::Levels(l) => l.len(),
                        _ => 0,
                    })
                    .sum();
                buf.clear();
                let _ = write!(buf, "{} levels", total);
                emit_study_line(
                    frame,
                    name,
                    buf,
                    base_color,
                    base_color,
                    x,
                    y,
                    line_height,
                    alpha,
                );
                return;
            }

            // Prefer scalar sub-outputs for the legend; skip
            // non-scalar ones (Markers, Footprint, Profile) that would
            // just repeat the study name with no value.
            let has_scalar = sub_outputs.iter().any(|s| {
                matches!(
                    s,
                    study::StudyOutput::Lines(_)
                        | study::StudyOutput::Band { .. }
                        | study::StudyOutput::Bars(_)
                        | study::StudyOutput::Histogram(_)
                        | study::StudyOutput::Levels(_)
                        | study::StudyOutput::StudyCandles(_)
                        | study::StudyOutput::Composite(_)
                )
            });
            for sub in sub_outputs {
                // When the composite contains at least one scalar
                // output, skip the non-scalar entries to avoid
                // duplicate name-only legend lines.
                if has_scalar
                    && matches!(
                        sub,
                        study::StudyOutput::Markers(_)
                            | study::StudyOutput::Footprint(_)
                            | study::StudyOutput::Profile(_, _)
                    )
                {
                    continue;
                }
                draw_output_entries(
                    sub,
                    name,
                    at,
                    frame,
                    base_color,
                    x,
                    y,
                    line_height,
                    buf,
                    alpha,
                );
            }
        }
        study::StudyOutput::Levels(levels) => {
            buf.clear();
            let _ = write!(buf, "{} levels", levels.len());
            emit_study_line(
                frame,
                name,
                buf,
                base_color,
                base_color,
                x,
                y,
                line_height,
                alpha,
            );
        }
        study::StudyOutput::StudyCandles(series) => {
            for s in series {
                let value_color = s
                    .points
                    .first()
                    .map(|p| crate::style::theme::rgba_to_iced_color(p.border_color))
                    .unwrap_or(base_color);

                buf.clear();
                // Binary search: StudyCandlePoints are sorted by x
                let idx = s.points.partition_point(|p| p.x < at);
                if let Some(pt) = s.points.get(idx).filter(|p| p.x == at) {
                    use std::fmt::Write;
                    let _ = write!(buf, "{:.1}", pt.close);
                }
                emit_study_line(
                    frame,
                    &s.label,
                    buf,
                    base_color,
                    value_color,
                    x,
                    y,
                    line_height,
                    alpha,
                );
            }
        }
        // Non-scalar outputs: show study name only
        study::StudyOutput::Profile(_, _)
        | study::StudyOutput::Footprint(_)
        | study::StudyOutput::Markers(_)
        | study::StudyOutput::Zones(_)
        | study::StudyOutput::Custom(_) => {
            emit_study_line(
                frame,
                name,
                "",
                base_color,
                base_color,
                x,
                y,
                line_height,
                alpha,
            );
        }
        study::StudyOutput::Empty => {}
    }
}

/// Count the number of text lines a study output produces.
pub(super) fn study_line_count(output: &study::StudyOutput) -> usize {
    match output {
        study::StudyOutput::Lines(series) => series.len(),
        study::StudyOutput::Band { .. } => 1,
        study::StudyOutput::Bars(series) => series.len(),
        study::StudyOutput::Histogram(_) => 1,
        study::StudyOutput::Composite(subs) => {
            // All-levels composites collapse to a single summary line
            let all_levels = !subs.is_empty()
                && subs
                    .iter()
                    .all(|s| matches!(s, study::StudyOutput::Levels(_)));
            if all_levels {
                return 1;
            }

            let has_scalar = subs.iter().any(|s| {
                matches!(
                    s,
                    study::StudyOutput::Lines(_)
                        | study::StudyOutput::Band { .. }
                        | study::StudyOutput::Bars(_)
                        | study::StudyOutput::Histogram(_)
                        | study::StudyOutput::Levels(_)
                        | study::StudyOutput::StudyCandles(_)
                        | study::StudyOutput::Composite(_)
                )
            });
            subs.iter()
                .filter(|s| {
                    !has_scalar
                        || !matches!(
                            s,
                            study::StudyOutput::Markers(_)
                                | study::StudyOutput::Footprint(_)
                                | study::StudyOutput::Profile(_, _)
                        )
                })
                .map(study_line_count)
                .sum()
        }
        study::StudyOutput::StudyCandles(series) => series.len().max(1),
        study::StudyOutput::Levels(_)
        | study::StudyOutput::Profile(_, _)
        | study::StudyOutput::Footprint(_)
        | study::StudyOutput::Markers(_)
        | study::StudyOutput::Zones(_)
        | study::StudyOutput::Custom(_) => 1,
        study::StudyOutput::Empty => 0,
    }
}

/// Draw study indicator text below the OHLCV crosshair tooltip.
///
/// Runs inside the crosshair cache (invalidated on every cursor
/// move), so we minimise per-frame allocations:
/// - Single reusable `String` buffer for formatting
/// - No intermediate `Vec` collection — draw directly to frame
/// - Binary search (O(log n)) for value lookups
///
/// Reads precomputed `hit_rects` / `detail_button_rects` (populated by
/// `KlineChart::recompute_study_overlay_rects`) so the drawing path
/// does zero per-frame allocation.  Non-hovered studies are dimmed
/// when the cursor is over any study label.
fn draw_study_overlay(
    studies: &[Box<dyn study::Study>],
    at_interval: u64,
    frame: &mut canvas::Frame,
    palette: &Extended,
    y_start: f32,
    hit_rects: &RefCell<Vec<(usize, Rectangle)>>,
    detail_button_rects: &RefCell<Vec<(usize, Rectangle)>>,
    selected: Option<usize>,
    cursor_position: Point,
) {
    let line_height = TEXT_SIZE + crate::style::tokens::spacing::XXS;
    let x = crate::style::tokens::spacing::MD;
    let base_color = palette.background.base.text;

    let hit_width: f32 = 300.0;
    let pad: f32 = 2.0;
    let detail_icon_size: f32 = crate::style::tokens::text::TINY;
    let detail_icon_pad: f32 = crate::style::tokens::spacing::XS;

    // Hit-test against precomputed rects (no allocation)
    let hovered_index = hit_rects
        .borrow()
        .iter()
        .find(|(_, r)| r.contains(cursor_position))
        .map(|(idx, _)| *idx);

    let hovered_detail = detail_button_rects
        .borrow()
        .iter()
        .find(|(_, r)| r.contains(cursor_position))
        .map(|(idx, _)| *idx);

    // Draw with dim for non-hovered studies
    let mut buf = String::with_capacity(64);
    let mut y = y_start;

    for (index, study) in studies.iter().enumerate() {
        let output = study.output();
        if matches!(output, study::StudyOutput::Empty) {
            continue;
        }

        let alpha = match hovered_index {
            Some(h) if h != index => 0.35,
            _ => 1.0,
        };

        let y_before = y;

        draw_output_entries(
            output,
            study.metadata().name.as_str(),
            at_interval,
            frame,
            base_color,
            x,
            &mut y,
            line_height,
            &mut buf,
            alpha,
        );

        let rect_height = y - y_before;
        if rect_height > 0.0 && selected == Some(index) {
            let rect = Rectangle {
                x: x - pad,
                y: y_before - pad,
                width: hit_width + 2.0 * pad,
                height: rect_height + 2.0 * pad,
            };
            let mut builder = canvas::path::Builder::new();
            builder.rectangle(rect.position(), rect.size());
            frame.stroke(
                &builder.build(),
                Stroke::default()
                    .with_width(1.0)
                    .with_color(palette.primary.base.color.scale_alpha(0.4)),
            );
        }

        // Draw detail icon for studies with a detail modal
        if study.metadata().capabilities.has_detail_modal {
            let detail_btn = detail_button_rects
                .borrow()
                .iter()
                .find(|(idx, _)| *idx == index)
                .map(|(_, r)| *r);

            if let Some(btn_rect) = detail_btn {
                let icon_alpha = if hovered_detail == Some(index) {
                    1.0
                } else {
                    0.5
                };
                let icon_color = base_color.scale_alpha(icon_alpha * alpha);

                // Draw a simple list/detail icon (three horizontal lines)
                let cx = btn_rect.x + detail_icon_pad;
                let cy = btn_rect.y + detail_icon_pad;
                let line_w = detail_icon_size;
                let gap = detail_icon_size / 4.0;
                for i in 0..3 {
                    let ly = cy + gap * i as f32 + gap * 0.5;
                    frame.stroke(
                        &canvas::Path::line(Point::new(cx, ly), Point::new(cx + line_w, ly)),
                        Stroke::default().with_width(1.5).with_color(icon_color),
                    );
                }
            }
        }
    }
}

// Debug overlay layout constants
const DEBUG_LINE_HEIGHT: f32 = crate::style::tokens::text::TITLE;
const DEBUG_PADDING: f32 = crate::style::tokens::spacing::SM;
const DEBUG_BOX_WIDTH: f32 = 220.0;
const DEBUG_BOX_MARGIN: f32 = crate::style::tokens::spacing::MD;
const DEBUG_TEXT_SIZE: f32 = crate::style::tokens::text::SMALL;
const DEBUG_BG_ALPHA: f32 = 0.75;

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

    let lod = LodCalculator::new(chart.scaling, chart.cell_width, visible_count, bounds.width);
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
        format!(
            "LOD: {:?} (dec {}x)",
            lod_level,
            lod_level.decimation_factor()
        ),
        format!(
            "Zoom: {:.2}x  Cell: {:.1}px",
            chart.scaling, chart.cell_width
        ),
    ];

    let box_height = (lines.len() as f32 * DEBUG_LINE_HEIGHT) + DEBUG_PADDING * 2.0;
    let box_x = bounds.width - DEBUG_BOX_WIDTH - DEBUG_BOX_MARGIN;
    let box_y = bounds.height - box_height - DEBUG_BOX_MARGIN;

    // Background
    frame.fill_rectangle(
        Point::new(box_x, box_y),
        iced::Size::new(DEBUG_BOX_WIDTH, box_height),
        Color::from_rgba(0.0, 0.0, 0.0, DEBUG_BG_ALPHA),
    );

    // Text lines
    let label_color = Color::from_rgba(0.6, 0.9, 0.6, 0.9);
    for (i, line) in lines.iter().enumerate() {
        frame.fill_text(canvas::Text {
            content: line.clone(),
            position: Point::new(
                box_x + DEBUG_PADDING,
                box_y + DEBUG_PADDING + (i as f32 * DEBUG_LINE_HEIGHT),
            ),
            size: iced::Pixels(DEBUG_TEXT_SIZE),
            color: label_color,
            font: AZERET_MONO,
            ..canvas::Text::default()
        });
    }
}
