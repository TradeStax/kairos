use super::types::SeriesLike;
use super::types::domain;

use exchange::TickerInfo;

use iced::{Color, Point, Rectangle};

use super::line_widget::{
    CHAR_W, ICON_BOX, ICON_GAP_AFTER_TEXT, ICON_SPACING, LEGEND_LINE_H, LEGEND_PADDING,
};
use super::TEXT_SIZE;
use super::scene::PlotContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IconKind {
    Cog,
    Close,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum LegendMode {
    Compact { include_pct: bool },
    Expanded,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct LegendRowHit {
    pub ticker: TickerInfo,
    pub cog: Rectangle,
    pub close: Rectangle,
    pub y_center: f32,
    pub row_rect: Rectangle,
    pub has_close: bool,
}

#[derive(Debug, Clone)]
pub(super) struct LegendLayout {
    pub bg: Rectangle,
    pub rows: Vec<LegendRowHit>,
}

#[derive(Clone)]
pub(super) struct EndLabel {
    pub pos: Point,
    pub bg_color: Color,
    pub text_color: Color,
    pub pct_change: String,
    pub symbol: String,
}

pub(super) fn resolve_label_overlaps(end_labels: &mut [EndLabel], plot: Rectangle) {
    if end_labels.len() <= 1 {
        return;
    }

    let half_h = TEXT_SIZE * 0.5 + 2.0;
    let mut min_y = plot.y + half_h;
    let mut max_y = plot.y + plot.height - half_h;
    if max_y < min_y {
        core::mem::swap(&mut min_y, &mut max_y);
    }

    let mut sep = TEXT_SIZE + 4.0;

    if end_labels.len() > 1 {
        let avail = (max_y - min_y).max(0.0);
        let needed = sep * (end_labels.len() as f32 - 1.0);
        if needed > avail {
            sep = if end_labels.len() > 1 {
                avail / (end_labels.len() as f32 - 1.0)
            } else {
                sep
            };
        }
    }

    end_labels.sort_by(|a, b| {
        a.pos
            .y
            .partial_cmp(&b.pos.y)
            .unwrap_or(core::cmp::Ordering::Equal)
    });

    let mut prev_y = f32::NAN;
    for i in 0..end_labels.len() {
        let low = if i == 0 { min_y } else { prev_y + sep };
        let high = max_y - sep * (end_labels.len() as f32 - 1.0 - i as f32);
        let target = end_labels[i].pos.y;
        let y = target.clamp(low, high);
        end_labels[i].pos.y = y;
        prev_y = y;
    }
}

impl<'a, S> super::line_widget::LineComparison<'a, S>
where
    S: SeriesLike,
{
    pub(super) fn compute_legend_layout(
        &self,
        ctx: &PlotContext,
        cursor_x: Option<u64>,
        step: f32,
        mode: LegendMode,
    ) -> Option<LegendLayout> {
        if self.series.is_empty() {
            return None;
        }

        let padding = LEGEND_PADDING;
        let line_h = LEGEND_LINE_H;

        let (include_icons, include_pct_in_width) = match mode {
            LegendMode::Expanded => (true, false),
            LegendMode::Compact { include_pct } => (false, include_pct),
        };

        let mut max_chars: usize = 0;
        let mut max_name_chars: usize = 0;
        let mut rows_count: usize = 0;

        for s in self.series.iter() {
            rows_count += 1;

            let name_len = s.ticker_info().ticker.symbol_and_exchange_string().len();
            max_name_chars = max_name_chars.max(name_len);

            let pct_len = if include_pct_in_width {
                domain::interpolate_y_at(s.points(), ctx.min_x)
                    .filter(|&y0| y0 != 0.0)
                    .and_then(|y0| {
                        cursor_x.and_then(|cx| {
                            domain::interpolate_y_at(s.points(), cx).map(|yc| {
                                let pct = ((yc / y0) - 1.0) * 100.0;
                                super::types::format_pct(pct, step, true)
                            })
                        })
                    })
                    .map(|s| s.len())
                    .unwrap_or(0)
            } else {
                0
            };

            let total = if pct_len > 0 {
                name_len + 1 + pct_len
            } else {
                name_len
            };
            max_chars = max_chars.max(total);
        }

        let text_w = (max_chars as f32) * CHAR_W;

        let icons_pack_w = if include_icons {
            2.0 * ICON_BOX + ICON_SPACING
        } else {
            0.0
        };
        let min_for_icons = if include_icons {
            (max_name_chars as f32) * CHAR_W + ICON_GAP_AFTER_TEXT + icons_pack_w
        } else {
            0.0
        };

        let plot_rect = ctx.plot_rect();

        let bg_w = (text_w.max(min_for_icons) + padding * 2.0)
            .clamp(80.0, (plot_rect.width * 0.6).max(80.0));

        if rows_count == 0 {
            return None;
        }

        let bg_max_h = ((rows_count as f32) * line_h + padding * 2.0)
            .min(plot_rect.height * 0.6)
            .max(line_h + padding * 2.0);

        let max_rows_fit = (((bg_max_h - padding * 2.0) / line_h).floor() as usize).max(1);
        let visible_rows = rows_count.min(max_rows_fit);
        let bg_h = (visible_rows as f32) * line_h + padding * 2.0;

        let bg = Rectangle {
            x: plot_rect.x + 4.0,
            y: plot_rect.y + 4.0,
            width: bg_w,
            height: bg_h,
        };

        let x_left = bg.x + padding;
        let x_right = bg.x + bg.width - padding;

        let mut rows: Vec<LegendRowHit> = Vec::with_capacity(visible_rows);
        let mut row_top = bg.y + padding;

        for (i, s) in self.series.iter().take(visible_rows).enumerate() {
            let y_center = row_top + line_h * 0.5;

            // Base ticker (i == 0) cannot be removed
            let has_close = i != 0;

            let name_len = s.ticker_info().ticker.symbol_and_exchange_string().len() as f32;
            let text_end_x = x_left + name_len * CHAR_W;

            let (cog, close, row_width) = if include_icons {
                let icons_pack_w = if has_close {
                    2.0 * ICON_BOX + ICON_SPACING
                } else {
                    ICON_BOX
                };

                let free_left = text_end_x + ICON_GAP_AFTER_TEXT;
                let free_right = x_right;

                let (cog_left, close_left_opt) = if free_right - free_left >= icons_pack_w {
                    let cog_left = free_left;
                    let close_left_opt = if has_close {
                        Some(cog_left + ICON_BOX + ICON_SPACING)
                    } else {
                        None
                    };
                    (cog_left, close_left_opt)
                } else if has_close {
                    let close_left = free_right - ICON_BOX;
                    let cog_left = (close_left - ICON_SPACING - ICON_BOX).max(free_left);
                    (cog_left, Some(close_left))
                } else {
                    let cog_left = (free_right - ICON_BOX).max(free_left);
                    (cog_left, None)
                };

                let cog = Rectangle {
                    x: cog_left,
                    y: y_center - ICON_BOX * 0.5,
                    width: ICON_BOX,
                    height: ICON_BOX,
                };
                let close = if let Some(cl) = close_left_opt {
                    Rectangle {
                        x: cl,
                        y: y_center - ICON_BOX * 0.5,
                        width: ICON_BOX,
                        height: ICON_BOX,
                    }
                } else {
                    Rectangle {
                        x: 0.0,
                        y: 0.0,
                        width: 0.0,
                        height: 0.0,
                    }
                };

                let content_right = if has_close {
                    close.x + close.width
                } else {
                    cog.x + cog.width
                };
                let row_width = (content_right + padding) - bg.x;
                (cog, close, row_width.clamp(0.0, bg.width))
            } else {
                let cog = Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                };
                let close = cog;
                let row_width = (text_end_x + padding) - bg.x;
                (cog, close, row_width.clamp(0.0, bg.width))
            };

            let row_rect = Rectangle {
                x: bg.x,
                y: row_top,
                width: row_width,
                height: line_h,
            };

            rows.push(LegendRowHit {
                ticker: *s.ticker_info(),
                cog,
                close,
                y_center,
                row_rect,
                has_close,
            });
            row_top += line_h;
        }

        Some(LegendLayout { bg, rows })
    }

    pub(super) fn collect_end_labels(&self, ctx: &PlotContext, step: f32) -> Vec<EndLabel> {
        let mut end_labels: Vec<EndLabel> = Vec::new();
        let plot_height = ctx.plot_rect().height;

        for s in self.series.iter() {
            let pts = s.points();
            if pts.is_empty() {
                continue;
            }
            let global_base = pts[0].1;
            if global_base == 0.0 {
                continue;
            }

            let last_vis = pts
                .iter()
                .rev()
                .find(|(x, _)| *x >= ctx.min_x && *x <= ctx.max_x);
            let (_x1, y1) = match last_vis {
                Some((_x, y)) => (0u64, *y),
                None => continue,
            };

            let idx_right = pts.iter().position(|(x, _)| *x >= ctx.min_x);
            let y0 = match idx_right {
                Some(0) => pts[0].1,
                Some(i) => {
                    let (x0, y0) = pts[i - 1];
                    let (x2, y2) = pts[i];
                    let dx = (x2.saturating_sub(x0)) as f32;
                    if dx > 0.0 {
                        let t = (ctx.min_x.saturating_sub(x0)) as f32 / dx;
                        y0 + (y2 - y0) * t.clamp(0.0, 1.0)
                    } else {
                        y0
                    }
                }
                None => continue,
            };

            if y0 == 0.0 {
                continue;
            }
            let pct_label = ((y1 / y0) - 1.0) * 100.0;

            let mut py_local = ctx.map_y(pct_label);
            let half_txt = TEXT_SIZE * 0.5;
            py_local = py_local.clamp(half_txt, plot_height - half_txt);

            let is_color_dark =
                data::config::theme::is_dark_rgba(crate::style::theme::iced_color_to_rgba(s.color()));
            let text_color = if is_color_dark {
                Color::WHITE
            } else {
                Color::BLACK
            };
            let bg_color = s.color();

            let label_text = super::types::format_pct(pct_label, step, true);

            end_labels.push(EndLabel {
                pos: Point::new(
                    ctx.regions.y_axis.x + ctx.regions.y_axis.width,
                    ctx.regions.plot.y + py_local,
                ),
                pct_change: label_text,
                bg_color,
                text_color,
                symbol: s.name(),
            });
        }

        end_labels
    }
}
