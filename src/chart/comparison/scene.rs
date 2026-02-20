use super::types::SeriesLike;
use super::types::domain;

use iced::advanced::Layout;
use iced::{Point, Rectangle, mouse};

use super::legend::{EndLabel, IconKind, LegendLayout, LegendMode, resolve_label_overlaps};
use super::line_widget::TEXT_SIZE_PUB as TEXT_SIZE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HitZone {
    Plot,
    XAxis,
    YAxis,
    Outside,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Regions {
    pub plot: Rectangle,
    pub x_axis: Rectangle,
    pub y_axis: Rectangle,
}

impl Regions {
    pub fn from_layout(root: Layout<'_>) -> Self {
        let root_bounds = root.bounds();

        // root.children = [ row, x_axis ]
        let row = root.child(0);
        let x_abs = root.child(1).bounds();

        // row.children  = [ plot, y_axis ]
        let plot_abs = row.child(0).bounds();
        let y_abs = row.child(1).bounds();

        let to_local = |r: Rectangle| Rectangle {
            x: r.x - root_bounds.x,
            y: r.y - root_bounds.y,
            width: r.width,
            height: r.height,
        };

        Regions {
            plot: to_local(plot_abs),
            y_axis: to_local(y_abs),
            x_axis: to_local(x_abs),
        }
    }

    pub fn is_in_plot(&self, p: Point) -> bool {
        p.x >= self.plot.x
            && p.x <= self.plot.x + self.plot.width
            && p.y >= self.plot.y
            && p.y <= self.plot.y + self.plot.height
    }

    pub fn is_in_x_axis(&self, p: Point) -> bool {
        p.x >= self.x_axis.x
            && p.x <= self.x_axis.x + self.x_axis.width
            && p.y >= self.x_axis.y
            && p.y <= self.x_axis.y + self.x_axis.height
    }

    pub fn is_in_y_axis(&self, p: Point) -> bool {
        p.x >= self.y_axis.x
            && p.x <= self.y_axis.x + self.y_axis.width
            && p.y >= self.y_axis.y
            && p.y <= self.y_axis.y + self.y_axis.height
    }

    pub fn hit_test(&self, p: Point) -> HitZone {
        if self.is_in_plot(p) {
            HitZone::Plot
        } else if self.is_in_x_axis(p) {
            HitZone::XAxis
        } else if self.is_in_y_axis(p) {
            HitZone::YAxis
        } else {
            HitZone::Outside
        }
    }
}

pub(super) struct PlotContext {
    pub regions: Regions,
    pub min_x: u64,
    pub max_x: u64,
    pub min_pct: f32,
    pub max_pct: f32,
    pub px_per_ms: f32,
}

impl PlotContext {
    pub fn plot_rect(&self) -> Rectangle {
        self.regions.plot
    }

    pub fn gutter_width(&self) -> f32 {
        self.regions.y_axis.width
    }

    pub fn map_x(&self, x: u64) -> f32 {
        let dx = x.saturating_sub(self.min_x) as f32;
        dx * self.px_per_ms
    }

    pub fn map_y(&self, pct: f32) -> f32 {
        let span = (self.max_pct - self.min_pct).max(1e-6);
        let t = (pct - self.min_pct) / span;
        let plot = self.plot_rect();
        plot.height - t.clamp(0.0, 1.0) * plot.height
    }
}

#[derive(Clone, Copy)]
pub(super) struct CursorInfo {
    pub x_domain: u64,
    pub y_pct: f32,
}

pub(super) struct Scene {
    pub ctx: PlotContext,
    pub y_ticks: Vec<f32>,
    pub y_labels: Vec<String>,
    pub end_labels: Vec<EndLabel>,
    pub cursor: Option<CursorInfo>,
    pub reserved_y: Option<Rectangle>,
    pub y_step: f32,
    pub legend: Option<LegendLayout>,
    pub hovering_legend: bool,
    pub hovered_icon: Option<(usize, IconKind)>,
    pub hovered_row: Option<usize>,
}

impl<'a, S> super::line_widget::LineComparison<'a, S>
where
    S: SeriesLike,
{
    pub(super) fn compute_scene(&self, layout: Layout<'_>, cursor: mouse::Cursor) -> Option<Scene> {
        let ((min_x, max_x), (min_pct, max_pct)) = self.compute_domains(self.pan)?;

        let regions = Regions::from_layout(layout);
        let plot = regions.plot;
        let span_ms = max_x.saturating_sub(min_x).max(1) as f32;
        let px_per_ms = if plot.width > 0.0 {
            plot.width / span_ms
        } else {
            1.0
        };

        let ctx = PlotContext {
            regions,
            min_x,
            max_x,
            min_pct,
            max_pct,
            px_per_ms,
        };

        let total_ticks = (plot.height / TEXT_SIZE / 3.).floor() as usize;
        let (all_ticks, step) = super::types::ticks(min_pct, max_pct, total_ticks);
        let mut ticks: Vec<f32> = all_ticks
            .into_iter()
            .filter(|t| (*t >= min_pct - f32::EPSILON) && (*t <= max_pct + f32::EPSILON))
            .collect();
        if ticks.is_empty() {
            ticks = vec![min_pct, max_pct];
        }
        let labels: Vec<String> = ticks
            .iter()
            .map(|t| super::types::format_pct(*t, step, false))
            .collect();

        let mut end_labels = self.collect_end_labels(&ctx, step);
        let plot_rect = ctx.plot_rect();

        resolve_label_overlaps(&mut end_labels, plot_rect);

        let cursor_root_local = cursor.position_in(layout.bounds());

        let cursor_info: Option<CursorInfo> = if let Some(local) = cursor_root_local {
            match ctx.regions.hit_test(local) {
                HitZone::Plot => {
                    let cx = local.x.clamp(plot_rect.x, plot_rect.x + plot_rect.width);
                    let ms_from_min = ((cx - plot_rect.x) / ctx.px_per_ms).round() as u64;
                    let x_domain_raw = ctx.min_x.saturating_add(ms_from_min);

                    let dt = self.dt_ms_est().max(1);
                    let lower = Self::align_floor(x_domain_raw, dt);
                    let upper = Self::align_ceil(x_domain_raw, dt);
                    let snapped_x = if x_domain_raw.saturating_sub(lower)
                        <= upper.saturating_sub(x_domain_raw)
                    {
                        lower
                    } else {
                        upper
                    }
                    .clamp(ctx.min_x, ctx.max_x);

                    let t = ((local.y - plot_rect.y) / plot_rect.height).clamp(0.0, 1.0);
                    let pct = ctx.min_pct + (1.0 - t) * (ctx.max_pct - ctx.min_pct);
                    Some(CursorInfo {
                        x_domain: snapped_x,
                        y_pct: pct,
                    })
                }
                _ => None,
            }
        } else {
            None
        };

        let show_pct_in_compact = cursor_info.is_some();
        let compact_layout = self.compute_legend_layout(
            &ctx,
            cursor_info.map(|c| c.x_domain),
            step,
            LegendMode::Compact {
                include_pct: show_pct_in_compact,
            },
        );
        let expanded_layout = self.compute_legend_layout(
            &ctx,
            cursor_info.map(|c| c.x_domain),
            step,
            LegendMode::Expanded,
        );

        let mut hovering_legend = false;
        let mut hovered_row: Option<usize> = None;
        let mut hovered_icon: Option<(usize, IconKind)> = None;

        if let Some(local) = cursor_root_local {
            let in_compact = compact_layout
                .as_ref()
                .map(|l| l.bg.contains(local))
                .unwrap_or(false);
            let in_expanded = expanded_layout
                .as_ref()
                .map(|l| l.bg.contains(local))
                .unwrap_or(false);

            if in_compact || in_expanded {
                hovering_legend = true;
            }
        }

        let legend_layout = if hovering_legend {
            expanded_layout.clone()
        } else {
            compact_layout.clone()
        };

        if hovering_legend
            && let (Some(local), Some(layout)) = (cursor_root_local, expanded_layout.as_ref())
        {
            for (i, row) in layout.rows.iter().enumerate() {
                if row.row_rect.contains(local) {
                    hovered_row = Some(i);
                    if row.cog.contains(local) {
                        hovered_icon = Some((i, IconKind::Cog));
                    } else if row.has_close && row.close.contains(local) {
                        hovered_icon = Some((i, IconKind::Close));
                    }
                    break;
                }
            }
        }

        let should_draw_crosshair = !(hovering_legend && hovered_row.is_some());
        let mut reserved_y: Option<Rectangle> = None;
        if should_draw_crosshair && let Some(ci) = cursor_info {
            let plot_rect = ctx.plot_rect();

            let t =
                ((ci.y_pct - ctx.min_pct) / (ctx.max_pct - ctx.min_pct).max(1e-6)).clamp(0.0, 1.0);
            let cy_px = plot_rect.y + plot_rect.height - t * plot_rect.height;

            let pct_str = super::types::format_pct(ci.y_pct, step, true);
            let pct_est_w = (pct_str.len() as f32) * (TEXT_SIZE * 0.6) + 10.0;

            let gutter_w = ctx.gutter_width();
            let y_w = pct_est_w.clamp(40.0, gutter_w - 8.0);
            let y_h = TEXT_SIZE + 6.0;

            let ylbl_x_right = ctx.regions.y_axis.x + gutter_w - 2.0;
            let ylbl_x = (ylbl_x_right - y_w).max(ctx.regions.y_axis.x + 2.0);
            let ylbl_y = cy_px.clamp(
                plot_rect.y + y_h * 0.5,
                plot_rect.y + plot_rect.height - y_h * 0.5,
            );
            reserved_y = Some(Rectangle {
                x: ylbl_x,
                y: ylbl_y - y_h * 0.5,
                width: y_w,
                height: y_h,
            });
        }

        Some(Scene {
            ctx,
            y_ticks: ticks,
            y_labels: labels,
            end_labels,
            cursor: cursor_info,
            reserved_y,
            y_step: step,
            legend: legend_layout,
            hovering_legend,
            hovered_icon,
            hovered_row,
        })
    }

    pub(super) fn compute_domains(&self, pan_points: f32) -> Option<((u64, u64), (f32, f32))> {
        if self.series.is_empty() {
            return None;
        }

        let dt = self.dt_ms_est().max(1);
        let all_points: Vec<&[(u64, f32)]> = self.series.iter().map(|s| s.points()).collect();

        let (min_x, max_x) = domain::window(&all_points, self.zoom, pan_points, dt)?;
        let (min_pct, max_pct) = domain::pct_domain(&all_points, min_x, max_x)?;

        Some(((min_x, max_x), (min_pct, max_pct)))
    }
}
