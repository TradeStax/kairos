use crate::chart::drawing;
use crate::chart::{Chart, ChartState, Interaction, Message, TEXT_SIZE};
use crate::components::primitives::AZERET_MONO;
use data::state::pane::ProfileDisplayType;
use exchange::util::Price;
use iced::theme::palette::Extended;
use iced::widget::canvas::{self, Event, Frame, Geometry, Path, Stroke, Text};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, Vector, mouse};
use study::output::ProfileLevel;

use super::ProfileChart;

/// Minimum row height in screen pixels for readable bars.
const MIN_ROW_PX: f32 = 4.0;

impl canvas::Program<Message> for ProfileChart {
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

        if bounds.width == 0.0 || self.profile_levels.is_empty() {
            return vec![];
        }

        let bounds_size = bounds.size();
        let palette = theme.extended_palette();

        // ── Main cache layer ─────────────────────────────────────────
        let main_layer = chart.cache.main.draw(renderer, bounds_size, |frame| {
            let center = Vector::new(bounds.width / 2.0, bounds.height / 2.0);

            frame.translate(center);
            frame.scale(chart.scaling);
            frame.translate(chart.translation);

            let region = chart.visible_region(frame.size());
            let (vis_high, vis_low) = chart.price_range(&region);

            // Dynamic quantum for bar height
            let tick_units = chart.tick_size.units.max(1);
            let dynamic_quantum = compute_dynamic_quantum(chart, 1, tick_units);
            let display_levels = if dynamic_quantum > self.quantum {
                merge_levels_to_quantum(&self.profile_levels, dynamic_quantum)
            } else {
                self.profile_levels.clone()
            };

            if display_levels.is_empty() {
                return;
            }

            // Recompute POC/VA for merged levels if needed
            let (poc, value_area) = if dynamic_quantum > self.quantum {
                let poc = study::orderflow::profile_core::find_poc_index(
                    &display_levels,
                );
                let va = poc.and_then(|idx| {
                    study::orderflow::profile_core::calculate_value_area(
                        &display_levels,
                        idx,
                        self.display_config.value_area_pct as f64,
                    )
                });
                (poc, va)
            } else {
                (self.poc_index, self.value_area)
            };

            // Cull off-screen levels
            let vis_high_units = vis_high.units();
            let vis_low_units = vis_low.units();
            let vis_start = display_levels
                .partition_point(|l| l.price_units < vis_low_units)
                .saturating_sub(1);
            let vis_end = display_levels
                .partition_point(|l| l.price_units <= vis_high_units)
                .min(display_levels.len());
            let visible_levels = &display_levels[vis_start..vis_end];

            // Bar height from adjacent levels
            let bar_height = if display_levels.len() >= 2 {
                let y0 = chart.price_to_y(Price::from_units(
                    display_levels[0].price_units,
                ));
                let y1 = chart.price_to_y(Price::from_units(
                    display_levels[1].price_units,
                ));
                (y1 - y0).abs().max(1.0)
            } else {
                chart.cell_height.max(1.0)
            };

            // Max volume for normalization
            let max_vol = display_levels
                .iter()
                .map(|l| l.buy_volume + l.sell_volume)
                .fold(0.0_f32, f32::max);

            // Available width for bars (full chart width in screen coords)
            let max_bar_width = bounds.width / chart.scaling;

            // Adjust value_area indices for visible slice
            let vis_value_area = value_area.map(|(vah, val)| {
                (vah.saturating_sub(vis_start), val.saturating_sub(vis_start))
            });

            // ── Pass 1: VA highlight fill ────────────────────────────
            if self.display_config.show_va_highlight {
                if let Some((vah_idx, val_idx)) = value_area {
                    if let (Some(vah_level), Some(val_level)) = (
                        display_levels.get(vah_idx),
                        display_levels.get(val_idx),
                    ) {
                        let y_vah = chart.price_to_y(Price::from_units(
                            vah_level.price_units,
                        ));
                        let y_val = chart.price_to_y(Price::from_units(
                            val_level.price_units,
                        ));
                        let y_top = y_vah.min(y_val);
                        let y_height = (y_vah - y_val).abs().max(1.0);

                        let vah_rgba = self.display_config.vah_color.unwrap_or(
                            data::config::color::Rgba { r: 0.2, g: 0.4, b: 0.8, a: 1.0 },
                        );
                        let fill_color = Color {
                            r: vah_rgba.r,
                            g: vah_rgba.g,
                            b: vah_rgba.b,
                            a: 0.08,
                        };
                        frame.fill_rectangle(
                            Point::new(-max_bar_width / 2.0, y_top),
                            Size::new(max_bar_width, y_height),
                            fill_color,
                        );
                    }
                }
            }

            // ── Pass 2: Volume bars ──────────────────────────────────
            let opacity = self.display_config.opacity;
            let vol_rgba = self.display_config.volume_color.unwrap_or(
                data::config::color::Rgba { r: 0.3, g: 0.5, b: 0.9, a: 1.0 },
            );
            let bid_rgba = self.display_config.bid_color.unwrap_or(
                data::config::color::Rgba { r: 0.2, g: 0.8, b: 0.2, a: 1.0 },
            );
            let ask_rgba = self.display_config.ask_color.unwrap_or(
                data::config::color::Rgba { r: 0.9, g: 0.2, b: 0.2, a: 1.0 },
            );

            if max_vol > 0.0 {
                for (idx, level) in visible_levels.iter().enumerate() {
                    let total = level.buy_volume + level.sell_volume;
                    if total <= 0.0 {
                        continue;
                    }
                    let y = chart.price_to_y(Price::from_units(
                        level.price_units,
                    ));
                    let factor = va_factor(
                        idx,
                        vis_value_area,
                        self.display_config.show_va_highlight,
                    );

                    match self.display_config.display_type {
                        ProfileDisplayType::Volume => {
                            let bar_len = (total / max_vol) * max_bar_width;
                            let color = to_color(vol_rgba, opacity * factor);
                            draw_bar_right(frame, 0.0, y, bar_height, bar_len, color);
                        }
                        ProfileDisplayType::BidAskVolume => {
                            let bar_len = (total / max_vol) * max_bar_width;
                            let sell_len = (level.sell_volume / total) * bar_len;
                            let buy_len = (level.buy_volume / total) * bar_len;
                            let top = y - bar_height / 2.0;
                            if sell_len > 0.0 {
                                frame.fill_rectangle(
                                    Point::new(0.0, top),
                                    Size::new(sell_len, bar_height),
                                    to_color(ask_rgba, opacity * factor),
                                );
                            }
                            if buy_len > 0.0 {
                                frame.fill_rectangle(
                                    Point::new(sell_len, top),
                                    Size::new(buy_len, bar_height),
                                    to_color(bid_rgba, opacity * factor),
                                );
                            }
                        }
                        ProfileDisplayType::Delta => {
                            let max_abs_delta = display_levels
                                .iter()
                                .map(|l| (l.buy_volume - l.sell_volume).abs())
                                .fold(0.0_f32, f32::max);
                            if max_abs_delta > 0.0 {
                                let delta = level.buy_volume - level.sell_volume;
                                let bar_len =
                                    (delta.abs() / max_abs_delta) * max_bar_width;
                                let color = if delta > 0.0 {
                                    to_color(bid_rgba, opacity * factor)
                                } else {
                                    to_color(ask_rgba, opacity * factor)
                                };
                                draw_bar_right(
                                    frame, 0.0, y, bar_height, bar_len, color,
                                );
                            }
                        }
                        ProfileDisplayType::DeltaAndTotal => {
                            let total_len = (total / max_vol) * max_bar_width;
                            let top = y - bar_height / 2.0;
                            // Total background
                            frame.fill_rectangle(
                                Point::new(0.0, top),
                                Size::new(total_len, bar_height),
                                to_color(vol_rgba, opacity * factor * 0.5),
                            );
                            // Delta overlay
                            let delta = level.buy_volume - level.sell_volume;
                            let delta_len = (delta.abs() / max_vol) * max_bar_width;
                            let delta_color = if delta > 0.0 {
                                to_color(bid_rgba, opacity * factor)
                            } else {
                                to_color(ask_rgba, opacity * factor)
                            };
                            frame.fill_rectangle(
                                Point::new(0.0, top),
                                Size::new(delta_len, bar_height),
                                delta_color,
                            );
                        }
                        ProfileDisplayType::DeltaPercentage => {
                            let delta = level.buy_volume - level.sell_volume;
                            let pct = delta / total;
                            if pct.abs() > f32::EPSILON {
                                let bar_len = pct.abs() * max_bar_width;
                                let color = if pct > 0.0 {
                                    to_color(bid_rgba, opacity * factor)
                                } else {
                                    to_color(ask_rgba, opacity * factor)
                                };
                                draw_bar_right(
                                    frame, 0.0, y, bar_height, bar_len, color,
                                );
                            }
                        }
                    }
                }
            }

            // ── Pass 3: POC line ─────────────────────────────────────
            if self.display_config.show_poc {
                if let Some(poc_idx) = poc {
                    if let Some(level) = display_levels.get(poc_idx) {
                        let y = chart.price_to_y(Price::from_units(
                            level.price_units,
                        ));
                        let poc_rgba = self.display_config.poc_color.unwrap_or(
                            data::config::color::Rgba {
                                r: 1.0,
                                g: 0.84,
                                b: 0.0,
                                a: 1.0,
                            },
                        );
                        let line = Path::line(
                            Point::new(-max_bar_width / 2.0, y),
                            Point::new(max_bar_width, y),
                        );
                        frame.stroke(
                            &line,
                            Stroke::default()
                                .with_color(to_color(poc_rgba, 1.0))
                                .with_width(
                                    self.display_config.poc_line_width
                                        / chart.scaling,
                                ),
                        );
                    }
                }
            }

            // ── Pass 4: VAH/VAL lines ────────────────────────────────
            if self.display_config.show_va_highlight {
                if let Some((vah_idx, val_idx)) = value_area {
                    let vah_rgba = self.display_config.vah_color.unwrap_or(
                        data::config::color::Rgba { r: 0.2, g: 0.4, b: 0.8, a: 1.0 },
                    );
                    let val_rgba = self.display_config.val_color.unwrap_or(
                        data::config::color::Rgba { r: 0.8, g: 0.4, b: 0.2, a: 1.0 },
                    );

                    for (idx, rgba) in [(vah_idx, vah_rgba), (val_idx, val_rgba)] {
                        if let Some(level) = display_levels.get(idx) {
                            let y = chart.price_to_y(Price::from_units(
                                level.price_units,
                            ));
                            let line = Path::line(
                                Point::new(-max_bar_width / 2.0, y),
                                Point::new(max_bar_width, y),
                            );
                            frame.stroke(
                                &line,
                                Stroke::default()
                                    .with_color(to_color(rgba, 0.8))
                                    .with_width(1.0 / chart.scaling),
                            );
                        }
                    }
                }
            }

            // ── Pass 5: HVN/LVN lines ───────────────────────────────
            if self.display_config.show_hvn {
                let hvn_rgba = self.display_config.hvn_color.unwrap_or(
                    data::config::color::Rgba { r: 0.0, g: 0.8, b: 0.4, a: 1.0 },
                );
                for node in &self.hvn_nodes {
                    let y = chart.price_to_y(Price::from_units(
                        node.price_units,
                    ));
                    draw_dashed_line(
                        frame,
                        y,
                        to_color(hvn_rgba, 0.7),
                        1.0 / chart.scaling,
                        -max_bar_width / 2.0,
                        max_bar_width,
                    );
                }
            }
            if self.display_config.show_lvn {
                let lvn_rgba = self.display_config.lvn_color.unwrap_or(
                    data::config::color::Rgba { r: 0.8, g: 0.0, b: 0.4, a: 1.0 },
                );
                for node in &self.lvn_nodes {
                    let y = chart.price_to_y(Price::from_units(
                        node.price_units,
                    ));
                    draw_dashed_line(
                        frame,
                        y,
                        to_color(lvn_rgba, 0.7),
                        1.0 / chart.scaling,
                        -max_bar_width / 2.0,
                        max_bar_width,
                    );
                }
            }

            // ── Overlay studies ───────────────────────────────────────
            for s in &self.studies {
                let output = s.output();
                let placement = s.placement();
                if !matches!(output, study::StudyOutput::Empty)
                    && matches!(
                        placement,
                        study::StudyPlacement::Overlay
                            | study::StudyPlacement::Background
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
        });

        // ── Drawings cache layer ─────────────────────────────────────
        let drawings_layer =
            chart.cache.drawings.draw(renderer, bounds_size, |frame| {
                drawing::render::draw_completed_drawings(
                    frame,
                    chart,
                    &self.drawings,
                    bounds_size,
                    palette,
                );
            });

        // ── Crosshair cache layer ────────────────────────────────────
        let crosshair =
            chart.cache.crosshair.draw(renderer, bounds_size, |frame| {
                drawing::render::draw_overlay_drawings(
                    frame,
                    chart,
                    &self.drawings,
                    bounds_size,
                    palette,
                );

                if let Some(cursor_position) = cursor.position_in(bounds) {
                    // Ruler
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

                    // Crosshair
                    let _result = crate::chart::overlay::draw_crosshair(
                        chart,
                        frame,
                        theme,
                        bounds_size,
                        cursor_position,
                        interaction,
                    );

                    // Profile tooltip
                    draw_profile_tooltip(
                        &self.profile_levels,
                        &self.ticker_info,
                        frame,
                        palette,
                        chart,
                        cursor_position,
                        bounds_size,
                    );
                }
            });

        vec![main_layer, drawings_layer, crosshair]
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
                if cursor.is_over(bounds) {
                    mouse::Interaction::Crosshair
                } else {
                    mouse::Interaction::default()
                }
            }
        }
    }
}

/// Draw profile tooltip showing volume at cursor price.
fn draw_profile_tooltip(
    levels: &[ProfileLevel],
    ticker_info: &exchange::FuturesTickerInfo,
    frame: &mut Frame,
    palette: &Extended,
    chart: &crate::chart::ViewState,
    cursor_position: Point,
    bounds: Size,
) {
    if levels.is_empty() {
        return;
    }

    // Convert cursor Y to chart-space Y, then to price
    let chart_y = (cursor_position.y - bounds.height / 2.0) / chart.scaling
        - chart.translation.y;
    let price = chart.y_to_price(chart_y);
    let price_units = price.units();

    // Find nearest level
    let nearest_idx = levels
        .partition_point(|l| l.price_units < price_units)
        .min(levels.len().saturating_sub(1));

    // Check neighbors for closest match
    let nearest = if nearest_idx > 0 {
        let prev = &levels[nearest_idx.saturating_sub(1)];
        let curr = &levels[nearest_idx.min(levels.len() - 1)];
        if (prev.price_units - price_units).abs()
            < (curr.price_units - price_units).abs()
        {
            prev
        } else {
            curr
        }
    } else {
        &levels[0]
    };

    let total = nearest.buy_volume + nearest.sell_volume;
    let delta = nearest.buy_volume - nearest.sell_volume;
    let precision = data::util::count_decimals(ticker_info.tick_size);

    let price_str = format!("{:.prec$}", nearest.price, prec = precision);
    let vol_str = format_volume(total);
    let buy_str = format_volume(nearest.buy_volume);
    let sell_str = format_volume(nearest.sell_volume);
    let delta_str = format!(
        "{}{}",
        if delta >= 0.0 { "+" } else { "" },
        format_volume(delta),
    );

    let base_color = palette.background.base.text;
    let buy_color = palette.success.base.color;
    let sell_color = palette.danger.base.color;

    let segments = [
        ("P", base_color, false),
        (&price_str, base_color, true),
        ("V", base_color, false),
        (&vol_str, base_color, true),
        ("B", base_color, false),
        (&buy_str, buy_color, true),
        ("S", base_color, false),
        (&sell_str, sell_color, true),
        ("D", base_color, false),
        (
            &delta_str,
            if delta >= 0.0 { buy_color } else { sell_color },
            true,
        ),
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
        frame.fill_text(Text {
            content: text.to_string(),
            position: Point::new(x, position.y),
            size: iced::Pixels(12.0),
            color: seg_color,
            font: AZERET_MONO,
            ..Text::default()
        });
        x += text.len() as f32 * 8.0;
        x += if is_value { 6.0 } else { 2.0 };
    }
}

/// Format volume with K/M suffixes.
fn format_volume(vol: f32) -> String {
    let abs = vol.abs();
    let formatted = if abs >= 1_000_000.0 {
        format!("{:.1}M", abs / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{:.1}K", abs / 1_000.0)
    } else {
        format!("{:.0}", abs)
    };
    if vol < 0.0 {
        format!("-{}", formatted)
    } else {
        formatted
    }
}

/// Compute the dynamic grouping quantum for automatic mode.
fn compute_dynamic_quantum(
    state: &crate::chart::ViewState,
    factor: i64,
    tick_units: i64,
) -> i64 {
    let pixel_per_tick = state.cell_height * state.scaling;
    let base_ticks = (MIN_ROW_PX / pixel_per_tick).ceil() as i64;
    (base_ticks * factor).max(1) * tick_units
}

/// Merge profile levels to a coarser quantum boundary.
fn merge_levels_to_quantum(
    levels: &[ProfileLevel],
    target_quantum: i64,
) -> Vec<ProfileLevel> {
    if levels.is_empty() {
        return Vec::new();
    }

    let mut merged = Vec::with_capacity(levels.len() / 2 + 1);
    let mut cur_bucket =
        (levels[0].price_units / target_quantum) * target_quantum;
    let mut buy_acc: f64 = 0.0;
    let mut sell_acc: f64 = 0.0;

    for level in levels {
        let bucket =
            (level.price_units / target_quantum) * target_quantum;
        if bucket != cur_bucket {
            merged.push(ProfileLevel {
                price: Price::from_units(cur_bucket).to_f64(),
                price_units: cur_bucket,
                buy_volume: buy_acc as f32,
                sell_volume: sell_acc as f32,
            });
            cur_bucket = bucket;
            buy_acc = 0.0;
            sell_acc = 0.0;
        }
        buy_acc += level.buy_volume as f64;
        sell_acc += level.sell_volume as f64;
    }
    merged.push(ProfileLevel {
        price: Price::from_units(cur_bucket).to_f64(),
        price_units: cur_bucket,
        buy_volume: buy_acc as f32,
        sell_volume: sell_acc as f32,
    });

    merged
}

/// Determines opacity multiplier based on VA membership.
fn va_factor(
    idx: usize,
    value_area: Option<(usize, usize)>,
    show_va_highlight: bool,
) -> f32 {
    if !show_va_highlight {
        return 1.0;
    }
    if let Some((vah_idx, val_idx)) = value_area {
        if idx >= val_idx && idx <= vah_idx {
            1.0
        } else {
            0.4
        }
    } else {
        1.0
    }
}

fn to_color(rgba: data::config::color::Rgba, opacity: f32) -> Color {
    Color {
        r: rgba.r,
        g: rgba.g,
        b: rgba.b,
        a: rgba.a * opacity,
    }
}

fn draw_bar_right(
    frame: &mut Frame,
    anchor_x: f32,
    y: f32,
    bar_h: f32,
    bar_len: f32,
    color: Color,
) {
    let top = y - bar_h / 2.0;
    frame.fill_rectangle(
        Point::new(anchor_x, top),
        Size::new(bar_len, bar_h),
        color,
    );
}

fn draw_dashed_line(
    frame: &mut Frame,
    y: f32,
    color: Color,
    width: f32,
    x_start: f32,
    x_end: f32,
) {
    let line = Path::line(
        Point::new(x_start, y),
        Point::new(x_end, y),
    );
    frame.stroke(
        &line,
        Stroke {
            width,
            line_dash: iced::widget::canvas::LineDash {
                segments: &[4.0, 4.0],
                offset: 0,
            },
            ..Stroke::default()
        }
        .with_color(color),
    );
}
