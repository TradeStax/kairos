use crate::component::primitives::{AZERET_MONO, ICONS_FONT, Icon};
use crate::style;
use crate::widget::chart::SeriesLike;
use crate::widget::chart::domain;

use iced::theme::palette::Extended;
use iced::widget::canvas;
use iced::{Point, Rectangle, Size, Vector};

use super::legend::{EndLabel, IconKind, LegendLayout};
use super::scene::{PlotContext, Scene};
use super::{
    CHAR_W, GAP_BREAK_MULTIPLIER, LEGEND_LINE_H, LEGEND_PADDING, MIN_X_TICK_PX, TEXT_SIZE,
};

impl<'a, S> super::LineComparison<'a, S>
where
    S: SeriesLike,
{
    #[allow(unused_assignments)]
    pub(super) fn fill_main_geometry(&self, frame: &mut canvas::Frame, ctx: &PlotContext) {
        for s in self.series.iter() {
            let pts = s.points();
            if pts.is_empty() {
                continue;
            }

            let idx_right = pts.iter().position(|(x, _)| *x >= ctx.min_x);
            let y0 = match idx_right {
                Some(0) => pts[0].1,
                Some(i) => {
                    let (x0, y0_) = pts[i - 1];
                    let (x1, y1_) = pts[i];
                    let dx = (x1.saturating_sub(x0)) as f32;
                    if dx > 0.0 {
                        let t = (ctx.min_x.saturating_sub(x0)) as f32 / dx;
                        y0_ + (y1_ - y0_) * t.clamp(0.0, 1.0)
                    } else {
                        y0_
                    }
                }
                None => continue,
            };

            if y0 == 0.0 {
                continue;
            }

            let mut builder = canvas::path::Builder::new();

            let gap_thresh: u64 = ((self.dt_ms_est() as f32) * GAP_BREAK_MULTIPLIER)
                .max(1.0)
                .round() as u64;

            let mut prev_x: Option<u64> = None;
            match idx_right {
                Some(ir) if ir > 0 => {
                    let px0 = ctx.map_x(ctx.min_x);
                    let py0 = ctx.map_y(0.0);
                    builder.move_to(Point::new(px0, py0));
                    prev_x = Some(ctx.min_x);
                }
                Some(0) => {
                    let (fx, fy) = pts[0];
                    if fx <= ctx.max_x {
                        let pct = ((fy / y0) - 1.0) * 100.0;
                        builder.move_to(Point::new(ctx.map_x(fx), ctx.map_y(pct)));
                        prev_x = Some(fx);
                    } else {
                        continue;
                    }
                }
                _ => continue,
            }

            let start_idx = idx_right.unwrap_or(pts.len());

            for (x, y) in pts.iter().skip(start_idx) {
                if *x > ctx.max_x {
                    break;
                }
                let pct = ((*y / y0) - 1.0) * 100.0;
                let px = ctx.map_x(*x);
                let py = ctx.map_y(pct);

                let connect = match prev_x {
                    Some(prev) => x.saturating_sub(prev) <= gap_thresh,
                    None => false,
                };

                if connect {
                    builder.line_to(Point::new(px, py));
                } else {
                    builder.move_to(Point::new(px, py));
                }
                prev_x = Some(*x);
            }

            let path = builder.build();
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_color(s.color())
                    .with_width(self.stroke_width),
            );
        }
    }

    pub(super) fn fill_overlay_y_labels(
        &self,
        frame: &mut canvas::Frame,
        end_labels: &[EndLabel],
        plot_right_x: f32,
        gutter: f32,
        reserved_y: Option<&Rectangle>,
    ) {
        let split_x = plot_right_x;

        for label in end_labels {
            let label_h = TEXT_SIZE + 4.0;

            let rect = Rectangle {
                x: split_x + 2.0,
                y: label.pos.y - TEXT_SIZE * 0.5 - 2.0,
                width: (gutter - 1.0).max(0.0),
                height: label_h,
            };

            let intersects_reserved = reserved_y.map(|res| rect.intersects(res)).unwrap_or(false);

            if !intersects_reserved {
                frame.fill_rectangle(
                    Point {
                        x: rect.x,
                        y: rect.y,
                    },
                    Size {
                        width: rect.width,
                        height: rect.height,
                    },
                    label.bg_color,
                );

                frame.fill(
                    &canvas::Path::circle(Point::new(label.pos.x, label.pos.y), 4.0),
                    label.bg_color,
                );

                frame.fill_text(canvas::Text {
                    content: label.pct_change.clone(),
                    position: label.pos - Vector::new(4.0, 0.0),
                    color: label.text_color,
                    size: TEXT_SIZE.into(),
                    font: AZERET_MONO,
                    align_x: iced::Alignment::End.into(),
                    align_y: iced::Alignment::Center.into(),
                    ..Default::default()
                });
            }

            let sym_right = split_x - 1.0;
            let sym_h = TEXT_SIZE + 4.0;
            let sym_w = (label.symbol.len() as f32) * CHAR_W + 8.0;
            let sym_rect = Rectangle {
                x: sym_right - sym_w,
                y: label.pos.y - sym_h * 0.5,
                width: sym_w,
                height: sym_h,
            };

            frame.fill_rectangle(
                Point::new(sym_rect.x, sym_rect.y),
                Size::new(sym_rect.width, sym_rect.height),
                label.bg_color,
            );
            frame.fill_text(canvas::Text {
                content: label.symbol.clone(),
                position: Point::new(sym_rect.x + sym_rect.width - 4.0, label.pos.y),
                color: label.text_color,
                size: TEXT_SIZE.into(),
                font: AZERET_MONO,
                align_x: iced::Alignment::End.into(),
                align_y: iced::Alignment::Center.into(),
                ..Default::default()
            });
        }
    }

    pub(super) fn fill_y_axis_labels(
        &self,
        frame: &mut canvas::Frame,
        ctx: &PlotContext,
        ticks: &[f32],
        labels: &[String],
        palette: &Extended,
    ) {
        let plot = ctx.plot_rect();
        for (i, tick) in ticks.iter().enumerate() {
            let mut y_local = ctx.map_y(*tick);
            let half_txt = TEXT_SIZE * 0.5;
            y_local = y_local.clamp(half_txt, plot.height - half_txt);

            let right_x = ctx.gutter_width() - 4.0;
            frame.fill_text(canvas::Text {
                content: labels[i].clone(),
                position: Point::new(right_x, y_local),
                color: palette.background.base.text,
                size: TEXT_SIZE.into(),
                font: AZERET_MONO,
                align_x: iced::Alignment::End.into(),
                align_y: iced::Alignment::Center.into(),
                ..Default::default()
            });
        }
    }

    pub(super) fn fill_x_axis_labels(
        &self,
        frame: &mut canvas::Frame,
        ctx: &PlotContext,
        palette: &Extended,
    ) {
        let (ticks, step_ms) =
            super::super::time_ticks(ctx.min_x, ctx.max_x, ctx.px_per_ms, MIN_X_TICK_PX);

        let baseline_to_text = 4.0;
        let y_center_local = baseline_to_text + 2.0 + TEXT_SIZE * 0.5;

        let plot_rect = ctx.plot_rect();

        let mut last_right = f32::NEG_INFINITY;
        for t in ticks {
            let x_local = ctx.map_x(t).clamp(0.0, plot_rect.width);

            let label_ts = Self::to_tz_ms(t, self.timezone);
            let label = super::super::format_time_label(label_ts, step_ms);

            let est_w = (label.len() as f32) * CHAR_W + 8.0;
            let left = x_local - est_w * 0.5;
            let right = x_local + est_w * 0.5;

            if left <= last_right {
                continue;
            }

            frame.fill_text(canvas::Text {
                content: label,
                position: Point::new(x_local, y_center_local),
                color: palette.background.base.text,
                size: TEXT_SIZE.into(),
                font: AZERET_MONO,
                align_x: iced::Alignment::Center.into(),
                align_y: iced::Alignment::Center.into(),
                ..Default::default()
            });

            last_right = right;
        }
    }

    pub(super) fn fill_top_left_legend(
        &self,
        frame: &mut canvas::Frame,
        ctx: &PlotContext,
        cursor_x: Option<u64>,
        palette: &Extended,
        step: f32,
        legend_layout: Option<&LegendLayout>,
        hovering_legend: bool,
        hovered_icon: Option<(usize, IconKind)>,
        hovered_row: Option<usize>,
    ) {
        let padding = LEGEND_PADDING;
        let line_h = LEGEND_LINE_H;
        let show_buttons = hovering_legend;

        let icon_normal = palette.background.base.text;
        let icon_hover = palette.background.strongest.text;
        let row_hover_fill = palette.background.strong.color.scale_alpha(0.22);

        if let Some(layout) = legend_layout {
            frame.fill_rectangle(
                Point::new(layout.bg.x, layout.bg.y),
                Size::new(layout.bg.width, layout.bg.height),
                palette.background.weakest.color.scale_alpha(0.9),
            );

            let x0 = layout.bg.x + padding;

            for (i, s) in self.series.iter().take(layout.rows.len()).enumerate() {
                let row = &layout.rows[i];
                let y = (row.y_center).round() + 0.0;

                if show_buttons && hovered_row == Some(i) {
                    let hl = Rectangle {
                        x: row.row_rect.x + 1.0,
                        y: row.row_rect.y,
                        width: (row.row_rect.width - 2.0).max(0.0),
                        height: row.row_rect.height,
                    };
                    frame.fill_rectangle(hl.position(), hl.size(), row_hover_fill);
                }

                let pct_str = if hovering_legend {
                    None
                } else {
                    domain::interpolate_y_at(s.points(), ctx.min_x)
                        .filter(|&y0| y0 != 0.0)
                        .and_then(|y0| {
                            cursor_x.and_then(|cx| {
                                domain::interpolate_y_at(s.points(), cx).map(|yc| {
                                    let pct = ((yc / y0) - 1.0) * 100.0;
                                    super::super::format_pct(pct, step, true)
                                })
                            })
                        })
                };

                let symbol_and_exchange = s.ticker_info().ticker.symbol_and_exchange_string();
                let content = if let Some(pct) = pct_str {
                    format!("{symbol_and_exchange} {pct}")
                } else {
                    symbol_and_exchange
                };

                frame.fill_text(canvas::Text {
                    content,
                    position: Point::new(x0, y),
                    color: s.color(),
                    size: TEXT_SIZE.into(),
                    font: AZERET_MONO,
                    align_x: iced::Alignment::Start.into(),
                    align_y: iced::Alignment::Center.into(),
                    ..Default::default()
                });

                if show_buttons {
                    let (cog_col, close_col) = match hovered_icon {
                        Some((hi, IconKind::Cog)) if hi == i => (icon_hover, icon_normal),
                        Some((hi, IconKind::Close)) if hi == i => (icon_normal, icon_hover),
                        _ => (icon_normal, icon_normal),
                    };

                    frame.fill_text(canvas::Text {
                        content: char::from(Icon::Cog).to_string(),
                        position: Point {
                            x: row.cog.center_x(),
                            y,
                        },
                        color: cog_col,
                        size: TEXT_SIZE.into(),
                        font: ICONS_FONT,
                        align_x: iced::Alignment::Center.into(),
                        align_y: iced::Alignment::Center.into(),
                        ..Default::default()
                    });

                    if row.has_close {
                        frame.fill_text(canvas::Text {
                            content: char::from(Icon::Close).to_string(),
                            position: Point {
                                x: row.close.center_x(),
                                y,
                            },
                            color: close_col,
                            size: TEXT_SIZE.into(),
                            font: ICONS_FONT,
                            align_x: iced::Alignment::Center.into(),
                            align_y: iced::Alignment::Center.into(),
                            ..Default::default()
                        });
                    }
                }
            }
            return;
        }

        let mut max_chars: usize = 0;
        let mut rows_count: usize = 0;

        for s in self.series.iter() {
            rows_count += 1;

            let pct_len = if hovering_legend {
                0
            } else {
                domain::interpolate_y_at(s.points(), ctx.min_x)
                    .filter(|&y0| y0 != 0.0)
                    .and_then(|y0| {
                        cursor_x.and_then(|cx| {
                            domain::interpolate_y_at(s.points(), cx).map(|yc| {
                                let pct = ((yc / y0) - 1.0) * 100.0;
                                super::super::format_pct(pct, step, true)
                            })
                        })
                    })
                    .map(|s| s.len())
                    .unwrap_or(0)
            };

            let name_len = s.ticker_info().ticker.symbol_and_exchange_string().len();
            let total = if pct_len > 0 {
                name_len + 1 + pct_len
            } else {
                name_len
            };
            if total > max_chars {
                max_chars = total;
            }
        }

        let plot_rect = ctx.plot_rect();

        let max_chars_f = max_chars as f32;
        let char_w = TEXT_SIZE * 0.64;
        let text_w = max_chars_f * char_w;
        let bg_w = (text_w + padding * 2.0).clamp(80.0, (plot_rect.width * 0.6).max(80.0));

        let rows_count_f = rows_count as f32;
        if rows_count_f > 0.0 {
            let bg_h = (rows_count_f * line_h + padding * 2.0).min(plot_rect.height * 0.6);
            frame.fill_rectangle(
                Point::new(plot_rect.x + 4.0, plot_rect.y + 4.0),
                Size::new(bg_w, bg_h),
                palette.background.weakest.color.scale_alpha(0.9),
            );
        }

        let mut y = plot_rect.y + padding + TEXT_SIZE * 0.5;
        let x0 = plot_rect.x + padding;

        for s in self.series.iter() {
            if y > plot_rect.y + plot_rect.height - TEXT_SIZE {
                break;
            }

            let pct_str = if hovering_legend {
                None
            } else {
                domain::interpolate_y_at(s.points(), ctx.min_x)
                    .filter(|&y0| y0 != 0.0)
                    .and_then(|y0| {
                        cursor_x.and_then(|cx| {
                            domain::interpolate_y_at(s.points(), cx).map(|yc| {
                                let pct = ((yc / y0) - 1.0) * 100.0;
                                super::super::format_pct(pct, step, true)
                            })
                        })
                    })
            };

            let symbol_and_exchange = s.ticker_info().ticker.symbol_and_exchange_string();
            let content = if let Some(pct) = pct_str {
                format!("{symbol_and_exchange} {pct}")
            } else {
                symbol_and_exchange
            };

            frame.fill_text(canvas::Text {
                content,
                position: Point::new(x0, y),
                color: s.color(),
                size: TEXT_SIZE.into(),
                font: AZERET_MONO,
                align_x: iced::Alignment::Start.into(),
                align_y: iced::Alignment::Center.into(),
                ..Default::default()
            });

            y += line_h;
        }
    }

    pub(super) fn fill_crosshair(
        &self,
        frame: &mut canvas::Frame,
        scene: &Scene,
        palette: &Extended,
    ) {
        let Some(ci) = scene.cursor else {
            return;
        };
        let ctx = &scene.ctx;
        let plot_rect = ctx.plot_rect();

        let cx = {
            let dx = ci.x_domain.saturating_sub(ctx.min_x) as f32;
            plot_rect.x + dx * ctx.px_per_ms
        };
        let y_span = (ctx.max_pct - ctx.min_pct).max(1e-6);
        let t = ((ci.y_pct - ctx.min_pct) / y_span).clamp(0.0, 1.0);
        let cy = plot_rect.y + plot_rect.height - t * plot_rect.height;

        let stroke = style::dashed_line_from_palette(palette);

        // Vertical
        let mut b = canvas::path::Builder::new();
        b.move_to(Point::new(cx, plot_rect.y));
        b.line_to(Point::new(cx, plot_rect.y + plot_rect.height));
        frame.stroke(&b.build(), stroke);

        // Horizontal
        let mut b = canvas::path::Builder::new();
        b.move_to(Point::new(plot_rect.x, cy));
        b.line_to(Point::new(plot_rect.x + plot_rect.width, cy));
        frame.stroke(&b.build(), stroke);

        let time_str = Self::format_crosshair_time(ci.x_domain, self.timezone);

        let text_col = palette.secondary.base.text;
        let bg_col = palette.secondary.base.color;

        let est_w = (time_str.len() as f32) * (TEXT_SIZE * 0.67) + 12.0;
        let label_w = est_w.clamp(100.0, 240.0);
        let label_h = TEXT_SIZE + 6.0;

        let time_x = cx.clamp(
            plot_rect.x + label_w * 0.5,
            plot_rect.x + plot_rect.width - label_w * 0.5,
        );
        let time_y = plot_rect.y + plot_rect.height + 2.0 + label_h * 0.5;

        frame.fill_rectangle(
            Point::new(time_x - label_w * 0.5, time_y - label_h * 0.5),
            Size::new(label_w, label_h),
            bg_col,
        );
        frame.fill_text(canvas::Text {
            content: time_str,
            position: Point::new(time_x, time_y),
            color: text_col,
            size: TEXT_SIZE.into(),
            font: AZERET_MONO,
            align_x: iced::Alignment::Center.into(),
            align_y: iced::Alignment::Center.into(),
            ..Default::default()
        });

        let gutter = ctx.gutter_width();
        let pct_str = super::super::format_pct(ci.y_pct, scene.y_step, true);
        let label_h = TEXT_SIZE + 6.0;

        let split_x = plot_rect.x + plot_rect.width;
        let gutter_right = split_x + gutter;

        let ylbl_x_right = gutter_right;
        let ylbl_y = cy.clamp(
            plot_rect.y + label_h * 0.5,
            plot_rect.y + plot_rect.height - label_h * 0.5,
        );

        frame.fill_rectangle(
            Point::new(split_x + 2.0, ylbl_y - label_h * 0.5),
            Size::new((gutter - 1.0).max(0.0), label_h),
            bg_col,
        );
        frame.fill_text(canvas::Text {
            content: pct_str,
            position: Point::new(ylbl_x_right - 4.0, ylbl_y),
            color: text_col,
            size: TEXT_SIZE.into(),
            font: AZERET_MONO,
            align_x: iced::Alignment::End.into(),
            align_y: iced::Alignment::Center.into(),
            ..Default::default()
        });
    }
}
