use super::types::SeriesLike;
use super::types::Zoom;
use crate::config::UserTimezone;
use data::{FuturesTickerInfo, Timeframe};

use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{self, Clipboard, Layout, Shell, Widget, layout, renderer};
use iced::widget::canvas;
use iced::{Element, Length, Point, Rectangle, Renderer, Size, Theme, Vector, mouse, window};
use iced_core::renderer::Quad;

use chrono::TimeZone;

use super::scene::{HitZone, Regions, Scene};
use crate::chart::core::tokens as chart_tokens;
use crate::style::tokens;

const Y_AXIS_GUTTER: f32 = tokens::chart::Y_AXIS_GUTTER;
const X_AXIS_HEIGHT: f32 = tokens::chart::X_AXIS_HEIGHT;
pub(super) const MIN_X_TICK_PX: f32 = tokens::chart::MIN_X_TICK_PX;
const ZOOM_STEP_PCT: f32 = tokens::chart::ZOOM_STEP_PCT;
pub(super) const GAP_BREAK_MULTIPLIER: f32 = tokens::chart::GAP_BREAK_MULTIPLIER;

pub const DEFAULT_ZOOM_POINTS: usize = 150;
pub const MIN_ZOOM_POINTS: usize = 2;
pub const MAX_ZOOM_POINTS: usize = 5000;

// Legend constants live in `crate::chart::core::tokens::legend`.
// Re-exported here for backward compatibility with sibling modules (legend.rs, render.rs, scene.rs).
pub(super) use chart_tokens::legend::CHAR_W;
pub(super) use chart_tokens::legend::ICON_BOX;
pub(super) use chart_tokens::legend::ICON_GAP_AFTER_TEXT;
pub(super) use chart_tokens::legend::ICON_SPACING;
pub(super) use chart_tokens::legend::LINE_H as LEGEND_LINE_H;
pub(super) use chart_tokens::legend::PADDING as LEGEND_PADDING;

#[derive(Debug, Clone)]
pub enum LineComparisonEvent {
    ZoomChanged(Zoom),
    PanChanged(f32),
    SeriesCog(FuturesTickerInfo),
    SeriesRemove(FuturesTickerInfo),
    XAxisDoubleClick,
}

/// Cache key for `compute_scene` — invalidated when layout bounds, cursor position, or
/// data version changes. Allows `draw()` and `mouse_interaction()` to share a single
/// scene computation per frame.
#[derive(Clone, PartialEq)]
struct SceneCacheKey {
    bounds: Rectangle,
    cursor_pos: Option<[u32; 2]>,
    version: u64,
}

impl SceneCacheKey {
    fn new(bounds: Rectangle, cursor: mouse::Cursor, version: u64) -> Self {
        let cursor_pos = cursor.position().map(|p| [p.x.to_bits(), p.y.to_bits()]);
        Self {
            bounds,
            cursor_pos,
            version,
        }
    }
}

pub(super) struct State {
    plot_cache: canvas::Cache,
    y_axis_cache: canvas::Cache,
    x_axis_cache: canvas::Cache,
    overlay_cache: canvas::Cache,
    is_panning: bool,
    last_cursor: Option<Point>,
    last_cache_rev: u64,
    // Track previous click for double-click detection
    previous_click: Option<iced_core::mouse::Click>,
    /// Cached scene to avoid recomputing in draw() and mouse_interaction()
    cached_scene: std::cell::RefCell<Option<(SceneCacheKey, Scene)>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            plot_cache: canvas::Cache::new(),
            y_axis_cache: canvas::Cache::new(),
            x_axis_cache: canvas::Cache::new(),
            overlay_cache: canvas::Cache::new(),
            is_panning: false,
            last_cursor: None,
            last_cache_rev: 0,
            previous_click: None,
            cached_scene: std::cell::RefCell::new(None),
        }
    }
}

impl State {
    fn clear_all_caches(&mut self) {
        self.plot_cache.clear();
        self.y_axis_cache.clear();
        self.x_axis_cache.clear();
        self.overlay_cache.clear();
        *self.cached_scene.borrow_mut() = None;
    }
}

pub struct LineComparison<'a, S> {
    pub(super) series: &'a [S],
    pub(super) stroke_width: f32,
    pub(super) zoom: Zoom,
    pub(super) pan: f32,
    pub(super) timeframe: Timeframe,
    pub(super) timezone: UserTimezone,
    pub(super) version: u64,
}

impl<'a, S> LineComparison<'a, S>
where
    S: SeriesLike,
{
    pub fn new(series: &'a [S], timeframe: Timeframe) -> Self {
        Self {
            series,
            stroke_width: 2.0,
            zoom: Zoom::points(DEFAULT_ZOOM_POINTS),
            timeframe,
            pan: 0.0,
            timezone: UserTimezone::Utc,
            version: 0,
        }
    }

    pub fn with_zoom(mut self, zoom: Zoom) -> Self {
        self.zoom = zoom;
        self
    }

    pub fn with_pan(mut self, pan: f32) -> Self {
        self.pan = pan;
        self
    }

    pub fn with_timezone(mut self, tz: UserTimezone) -> Self {
        self.timezone = tz;
        self
    }

    pub fn version(mut self, rev: u64) -> Self {
        self.version = rev;
        self
    }

    pub(super) fn align_floor(ts: u64, dt: u64) -> u64 {
        if dt == 0 {
            return ts;
        }
        (ts / dt) * dt
    }

    pub(super) fn align_ceil(ts: u64, dt: u64) -> u64 {
        if dt == 0 {
            return ts;
        }
        let f = (ts / dt) * dt;
        if f == ts { ts } else { f.saturating_add(dt) }
    }

    fn max_points_available(&self) -> usize {
        self.series
            .iter()
            .map(|s| s.points().len())
            .max()
            .unwrap_or(0)
    }

    fn normalize_zoom(&self, z: Zoom) -> Zoom {
        if z.is_all() {
            return Zoom::all();
        }
        let n = z.0.clamp(MIN_ZOOM_POINTS, MAX_ZOOM_POINTS);
        Zoom::points(n)
    }

    fn step_zoom_percent(&self, current: Zoom, zoom_in: bool) -> Zoom {
        let len = self.max_points_available().max(MIN_ZOOM_POINTS);
        let base_n = if current.is_all() {
            len
        } else {
            current.0.clamp(MIN_ZOOM_POINTS, MAX_ZOOM_POINTS)
        };

        let step = ((base_n as f32) * ZOOM_STEP_PCT).ceil().max(1.0) as usize;

        let new_n = if zoom_in {
            base_n.saturating_sub(step).max(MIN_ZOOM_POINTS)
        } else {
            base_n.saturating_add(step).min(MAX_ZOOM_POINTS)
        };

        Zoom::points(new_n)
    }

    pub(super) fn current_x_span(&self) -> f32 {
        let mut any = false;
        let mut data_min_x = u64::MAX;
        let mut data_max_x = u64::MIN;
        for s in self.series {
            for (x, _) in s.points() {
                any = true;
                if *x < data_min_x {
                    data_min_x = *x;
                }
                if *x > data_max_x {
                    data_max_x = *x;
                }
            }
        }
        if !any {
            return 1.0;
        }
        if self.zoom.is_all() {
            ((data_max_x - data_min_x) as f32).max(1.0)
        } else {
            let n = self.zoom.0.clamp(MIN_ZOOM_POINTS, MAX_ZOOM_POINTS);
            let dt = (self.dt_ms_est() as f32).max(1e-6);
            ((n.saturating_sub(1)) as f32 * dt).max(1.0)
        }
    }

    pub(super) fn dt_ms_est(&self) -> u64 {
        self.timeframe.to_milliseconds()
    }

    pub(super) fn format_crosshair_time(ts_ms: u64, tz: UserTimezone) -> String {
        let ts_i64 = ts_ms as i64;
        match tz {
            UserTimezone::Utc => {
                if let Some(dt) = chrono::Utc.timestamp_millis_opt(ts_i64).single() {
                    dt.format("%a %b %-d %H:%M").to_string()
                } else {
                    ts_ms.to_string()
                }
            }
            UserTimezone::Local => {
                if let Some(dt) = chrono::Local.timestamp_millis_opt(ts_i64).single() {
                    dt.format("%a %b %-d %H:%M").to_string()
                } else {
                    ts_ms.to_string()
                }
            }
        }
    }

    pub(super) fn to_tz_ms(ts_ms: u64, tz: UserTimezone) -> u64 {
        match tz {
            UserTimezone::Utc => ts_ms,
            UserTimezone::Local => {
                if let Some(dt) = chrono::Local.timestamp_millis_opt(ts_ms as i64).single() {
                    let off_ms = (dt.offset().local_minus_utc() as i64) * 1000;
                    if off_ms >= 0 {
                        ts_ms.saturating_add(off_ms as u64)
                    } else {
                        ts_ms.saturating_sub((-off_ms) as u64)
                    }
                } else {
                    ts_ms
                }
            }
        }
    }

    /// Returns a cached `Scene`, recomputing only when layout bounds, cursor position,
    /// or data version have changed. This prevents the double computation that occurs
    /// when both `draw()` and `mouse_interaction()` call `compute_scene()` in the same
    /// frame.
    pub(super) fn get_or_compute_scene(
        &self,
        state: &State,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) -> Option<Scene> {
        let key = SceneCacheKey::new(layout.bounds(), cursor, self.version);
        {
            let cache = state.cached_scene.borrow();
            if let Some((cached_key, cached_scene)) = cache.as_ref()
                && cached_key == &key
            {
                return Some(cached_scene.clone());
            }
        }
        let scene = self.compute_scene(layout, cursor)?;
        *state.cached_scene.borrow_mut() = Some((key, scene.clone()));
        Some(scene)
    }
}

impl<'a, S, M> Widget<M, Theme, Renderer> for LineComparison<'a, S>
where
    S: SeriesLike,
    M: Clone + 'static + From<LineComparisonEvent>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        // Column: [ Row(plot, y_axis) , x_axis ]
        let gutter_w = Y_AXIS_GUTTER;
        let x_axis_h = X_AXIS_HEIGHT;

        // First row: plot + y-axis
        let row_node = layout::next_to_each_other(
            &limits.shrink(Size::new(0.0, x_axis_h)),
            0.0,
            |l| {
                layout::atomic(
                    &l.shrink(Size::new(gutter_w, 0.0)),
                    Length::Fill,
                    Length::Fill,
                )
            },
            |l| layout::atomic(l, gutter_w, Length::Fill),
        );

        // X axis full width at bottom
        let x_axis_node = layout::atomic(limits, Length::Fill, x_axis_h);

        let row_node_height = row_node.size().height;

        let total_w = row_node.size().width;
        let total_h = row_node_height + x_axis_h;

        layout::Node::with_children(
            Size::new(total_w, total_h),
            vec![
                row_node.move_to(Point::new(0.0, 0.0)),
                x_axis_node.move_to(Point::new(0.0, row_node_height)),
            ],
        )
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        _viewport: &Rectangle,
    ) {
        if shell.is_event_captured() {
            return;
        }

        match event {
            iced::Event::Mouse(mouse_event) => {
                let state = tree.state.downcast_mut::<State>();
                let bounds = layout.bounds();
                let regions = Regions::from_layout(layout);

                let Some(cursor_pos) = cursor.position_in(bounds) else {
                    if state.is_panning {
                        state.is_panning = false;
                        state.last_cursor = None;
                    }
                    return;
                };

                let zone = regions.hit_test(cursor_pos);

                match mouse_event {
                    mouse::Event::WheelScrolled {
                        delta: mouse::ScrollDelta::Lines { y, .. },
                    } => {
                        if !matches!(zone, HitZone::Plot) {
                            return;
                        }

                        let zoom_in = *y > 0.0;
                        let new_zoom = self.step_zoom_percent(self.zoom, zoom_in);

                        if new_zoom != self.zoom {
                            shell.publish(M::from(LineComparisonEvent::ZoomChanged(
                                self.normalize_zoom(new_zoom),
                            )));
                            state.clear_all_caches();
                        }
                    }
                    mouse::Event::ButtonPressed(mouse::Button::Left) => {
                        if let Some(global_pos) = cursor.position() {
                            let new_click = iced_core::mouse::Click::new(
                                global_pos,
                                mouse::Button::Left,
                                state.previous_click,
                            );

                            if matches!(zone, HitZone::XAxis)
                                && new_click.kind() == iced_core::mouse::click::Kind::Double
                            {
                                shell.publish(M::from(LineComparisonEvent::XAxisDoubleClick));
                                state.clear_all_caches();
                                state.previous_click = Some(new_click);
                                return;
                            }

                            state.previous_click = Some(new_click);
                        } else {
                            state.previous_click = None;
                        }

                        if matches!(zone, HitZone::XAxis) {
                            return;
                        }

                        if let Some(scene) = self.compute_scene(layout, cursor)
                            && let Some(legend) = scene.legend.as_ref()
                        {
                            for row in &legend.rows {
                                if row.cog.contains(cursor_pos) {
                                    shell.publish(M::from(LineComparisonEvent::SeriesCog(
                                        row.ticker,
                                    )));
                                    state.clear_all_caches();
                                    return;
                                }
                                if row.has_close && row.close.contains(cursor_pos) {
                                    shell.publish(M::from(LineComparisonEvent::SeriesRemove(
                                        row.ticker,
                                    )));
                                    state.clear_all_caches();
                                    return;
                                }
                            }
                        }

                        if matches!(zone, HitZone::Plot) {
                            state.is_panning = true;
                            state.last_cursor = Some(cursor_pos);
                        }
                    }
                    mouse::Event::ButtonReleased(mouse::Button::Left) => {
                        state.is_panning = false;
                        state.last_cursor = None;
                    }
                    mouse::Event::CursorMoved { .. } => {
                        if state.is_panning {
                            let prev = state.last_cursor.unwrap_or(cursor_pos);
                            let dx_px = cursor_pos.x - prev.x;

                            if dx_px.abs() > 0.0 {
                                let x_span = self.current_x_span();
                                let plot_w = regions.plot.width.max(1.0);
                                let dx_ms = -(dx_px) * (x_span / plot_w);
                                let dt = self.dt_ms_est().max(1) as f32;
                                let dx_pts = dx_ms / dt;

                                let event = LineComparisonEvent::PanChanged(self.pan + dx_pts);

                                shell.publish(M::from(event));
                                state.clear_all_caches();
                            }
                            state.last_cursor = Some(cursor_pos);
                        } else if matches!(zone, HitZone::Plot) {
                            state.overlay_cache.clear();
                        }
                    }
                    _ => {}
                }
            }
            iced::Event::Window(window::Event::RedrawRequested(_)) => {
                let state = tree.state.downcast_mut::<State>();

                if state.last_cache_rev != self.version {
                    state.clear_all_caches();
                    state.last_cache_rev = self.version;
                }
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        use advanced::Renderer as _;

        let state = tree.state.downcast_ref::<State>();
        let Some(scene) = self.get_or_compute_scene(state, layout, cursor) else {
            return;
        };

        let bounds = layout.bounds();
        let palette = theme.extended_palette();

        renderer.with_translation(Vector::new(bounds.x, bounds.y), |r| {
            let plot_rect = scene.ctx.plot_rect();

            let plot_geom = state.plot_cache.draw(r, plot_rect.size(), |frame| {
                self.fill_main_geometry(frame, &scene.ctx);
            });

            let splitter_color = palette.background.strong.color.scale_alpha(0.25);
            r.fill_quad(
                Quad {
                    bounds: Rectangle {
                        x: plot_rect.x,
                        y: plot_rect.y + plot_rect.height,
                        width: plot_rect.width + scene.ctx.regions.y_axis.width,
                        height: 1.0,
                    },
                    snap: true,
                    ..Default::default()
                },
                splitter_color,
            );
            r.fill_quad(
                Quad {
                    bounds: Rectangle {
                        x: plot_rect.x + plot_rect.width,
                        y: plot_rect.y,
                        width: 1.0,
                        height: plot_rect.height,
                    },
                    snap: true,
                    ..Default::default()
                },
                splitter_color,
            );

            let y_rect = scene.ctx.regions.y_axis;
            let y_geom = state.y_axis_cache.draw(r, y_rect.size(), |frame| {
                self.fill_y_axis_labels(
                    frame,
                    &scene.ctx,
                    &scene.y_ticks,
                    &scene.y_labels,
                    palette,
                );
            });

            let x_rect = scene.ctx.regions.x_axis;
            let x_geom = state.x_axis_cache.draw(r, x_rect.size(), |frame| {
                self.fill_x_axis_labels(frame, &scene.ctx, palette);
            });

            let overlay_geom = state.overlay_cache.draw(r, bounds.size(), |frame| {
                self.fill_overlay_y_labels(
                    frame,
                    &scene.end_labels,
                    scene.ctx.regions.y_axis.x,
                    scene.ctx.gutter_width(),
                    scene.reserved_y.as_ref(),
                );
                self.fill_top_left_legend(
                    frame,
                    &scene.ctx,
                    if scene.hovering_legend {
                        None
                    } else {
                        scene.cursor.map(|c| c.x_domain)
                    },
                    palette,
                    scene.y_step,
                    scene.legend.as_ref(),
                    scene.hovering_legend,
                    scene.hovered_icon,
                    scene.hovered_row,
                );
                if !(scene.hovering_legend && scene.hovered_row.is_some()) {
                    self.fill_crosshair(frame, &scene, palette);
                }
            });

            r.with_translation(Vector::new(plot_rect.x, plot_rect.y), |r| {
                use iced::advanced::graphics::geometry::Renderer as _;
                r.draw_geometry(plot_geom);
            });
            r.with_translation(Vector::new(y_rect.x, y_rect.y), |r| {
                use iced::advanced::graphics::geometry::Renderer as _;
                r.draw_geometry(y_geom);
            });
            r.with_translation(Vector::new(x_rect.x, x_rect.y), |r| {
                use iced::advanced::graphics::geometry::Renderer as _;
                r.draw_geometry(x_geom);
            });

            r.with_layer(
                Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: bounds.width,
                    height: bounds.height,
                },
                |r| {
                    use iced::advanced::graphics::geometry::Renderer as _;
                    r.draw_geometry(overlay_geom);
                },
            );
        });
    }

    fn mouse_interaction(
        &self,
        _state: &Tree,
        layout: Layout<'_>,
        cursor: advanced::mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> advanced::mouse::Interaction {
        if let Some(cursor_in_layout) = cursor.position_in(layout.bounds()) {
            let state = _state.state.downcast_ref::<State>();
            if let Some(scene) = self.get_or_compute_scene(state, layout, cursor) {
                if let Some(legend) = scene.legend.as_ref() {
                    for row in &legend.rows {
                        if row.cog.contains(cursor_in_layout)
                            || (row.has_close && row.close.contains(cursor_in_layout))
                        {
                            return advanced::mouse::Interaction::Pointer;
                        }
                    }
                }

                if scene.hovering_legend && scene.hovered_row.is_some() {
                    return advanced::mouse::Interaction::default();
                }

                if state.is_panning {
                    return advanced::mouse::Interaction::Grabbing;
                }

                match scene.ctx.regions.hit_test(cursor_in_layout) {
                    HitZone::Plot => advanced::mouse::Interaction::Crosshair,
                    _ => advanced::mouse::Interaction::default(),
                }
            } else {
                advanced::mouse::Interaction::default()
            }
        } else {
            advanced::mouse::Interaction::default()
        }
    }
}

impl<'a, S, M> From<LineComparison<'a, S>> for Element<'a, M, Theme, Renderer>
where
    S: SeriesLike,
    M: Clone + 'a + 'static + From<LineComparisonEvent>,
{
    fn from(chart: LineComparison<'a, S>) -> Self {
        Element::new(chart)
    }
}
