//! Side Panel Canvas
//!
//! Renders side-panel-placement studies (VBP cumulative profile, etc.)
//! in a separate vertical canvas to the right of the main chart.
//! The Y axis is shared with the main chart (same price scale), so
//! volume bars are drawn as horizontal bars whose heights align with
//! the price levels shown on the shared Y-axis labels.

use super::coord;
use crate::chart::core::SidePanelStudyInfo;
use crate::chart::{Message, ViewState};
use iced::widget::canvas::{self, Cache, Event, Frame, Geometry, Path, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, mouse};
use study::output::{ProfileOutput, ProfileRenderConfig, VbpGroupingMode, VbpResolvedCache};

use data::Price;

/// Canvas program that renders side-panel studies (horizontal VBP bars)
/// sharing the main chart's price Y-axis.
pub struct SidePanelCanvas<'a> {
    pub studies: Vec<SidePanelStudyInfo<'a>>,
    pub state: &'a ViewState,
    pub cache: &'a Cache,
    pub crosshair_cache: &'a Cache,
}

impl<'a> canvas::Program<Message> for SidePanelCanvas<'a> {
    type State = ();

    fn update(
        &self,
        _state: &mut (),
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let y = cursor.position_in(bounds).map(|p| p.y);
                Some(canvas::Action::publish(Message::SidePanelCrosshairMoved(y)))
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let content = self.cache.draw(renderer, bounds.size(), |frame| {
            draw_side_panel_content(frame, &self.studies, self.state, bounds.size());
        });

        let crosshair = self.crosshair_cache.draw(renderer, bounds.size(), |frame| {
            if let Some(y) = self.state.crosshair.y.get() {
                draw_crosshair(frame, y, bounds.size());
            }
        });

        vec![content, crosshair]
    }
}

// ── Content rendering ─────────────────────────────────────────────────

fn draw_side_panel_content(
    frame: &mut Frame,
    studies: &[SidePanelStudyInfo<'_>],
    state: &ViewState,
    bounds: Size,
) {
    use study::StudyOutput;

    for info in studies {
        if let StudyOutput::Profile(profiles, config) = info.output {
            for profile in profiles {
                render_side_panel_bars(frame, profile, config, state, bounds);
            }
        }
    }
}

/// Render horizontal VBP bars for one profile in the side panel.
///
/// Y coordinates match the main chart transform:
/// `screen_y = (price_to_y(price) + state.translation.y) * state.scaling + bounds.height / 2.0`
///
/// Max volume is computed from the **resolved** (quantum-merged) levels so bars
/// never overflow the canvas bounds. Using the original unmerged max would cause
/// merged levels (which can have higher combined volume) to produce bar_width > 1.0.
pub fn render_side_panel_bars(
    frame: &mut Frame,
    profile: &ProfileOutput,
    config: &ProfileRenderConfig,
    state: &ViewState,
    bounds: Size,
) {
    if profile.levels.is_empty() {
        return;
    }

    // Populate the resolved (quantum-merged) cache first
    ensure_resolved_cache(profile, config, state);
    let cache_guard = profile
        .resolved_cache
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let Some(resolved) = cache_guard.as_ref() else {
        return;
    };

    if resolved.levels.is_empty() {
        return;
    }

    // Compute max from the RESOLVED levels — merged levels may have higher
    // combined volume than the original per-level max.
    let max_volume = resolved
        .levels
        .iter()
        .map(|l| l.buy_volume + l.sell_volume)
        .fold(0.0_f32, f32::max);

    if max_volume <= 0.0 {
        return;
    }

    let tick_units = state.tick_size.units.max(1) as f32;

    for (idx, level) in resolved.levels.iter().enumerate() {
        let vol = level.buy_volume + level.sell_volume;
        if vol <= 0.0 {
            continue;
        }

        // Convert price → screen Y (matching main chart transform exactly)
        let price = Price::from_units(level.price_units);
        let chart_y = state.price_to_y(price);
        let screen_y = (chart_y + state.translation.y) * state.scaling + bounds.height / 2.0;

        // Row height in screen pixels — matches one tick level height on the main canvas
        let row_h =
            (state.cell_height * state.scaling * (resolved.quantum as f32 / tick_units)).max(1.0);

        let top = screen_y - row_h / 2.0;

        // Cull off-screen levels
        if top + row_h <= 0.0 || top >= bounds.height {
            continue;
        }

        // Bars grow left→right. Reserve a 2px gap so bars never touch the y-axis rule.
        let usable = (bounds.width - 2.0).max(0.0);
        let bar_width = (vol / max_volume) * usable;
        if bar_width < 0.5 {
            continue;
        }

        let color = bar_color(level, idx, resolved.value_area, config);
        frame.fill_rectangle(Point::new(0.0, top), Size::new(bar_width, row_h), color);
    }

    // POC line
    if config.poc_config.show_poc
        && let Some(poc_idx) = resolved.poc
        && let Some(level) = resolved.levels.get(poc_idx)
    {
        let price = Price::from_units(level.price_units);
        let chart_y = state.price_to_y(price);
        let screen_y = (chart_y + state.translation.y) * state.scaling + bounds.height / 2.0;
        let color = coord::to_iced_color(config.poc_config.poc_color, 1.0);
        let width = coord::effective_line_width(config.poc_config.poc_line_width, state.scaling);
        let line = Path::line(
            Point::new(0.0, screen_y),
            Point::new(bounds.width, screen_y),
        );
        frame.stroke(&line, Stroke::default().with_color(color).with_width(width));
    }

    // VAH / VAL lines
    if config.va_config.show_value_area
        && let Some((vah_idx, val_idx)) = resolved.value_area
    {
        for (idx, color_field, width_field) in [
            (
                vah_idx,
                config.va_config.vah_color,
                config.va_config.vah_line_width,
            ),
            (
                val_idx,
                config.va_config.val_color,
                config.va_config.val_line_width,
            ),
        ] {
            if let Some(level) = resolved.levels.get(idx) {
                let price = Price::from_units(level.price_units);
                let chart_y = state.price_to_y(price);
                let screen_y =
                    (chart_y + state.translation.y) * state.scaling + bounds.height / 2.0;
                let color = coord::to_iced_color(color_field, 1.0);
                let width = coord::effective_line_width(width_field, state.scaling);
                let line = Path::line(
                    Point::new(0.0, screen_y),
                    Point::new(bounds.width, screen_y),
                );
                frame.stroke(&line, Stroke::default().with_color(color).with_width(width));
            }
        }
    }
}

/// Choose the bar color based on VBP type and value area membership.
fn bar_color(
    level: &study::output::ProfileLevel,
    idx: usize,
    value_area: Option<(usize, usize)>,
    config: &ProfileRenderConfig,
) -> Color {
    use study::output::VbpType;

    let va_factor = if config.va_config.show_va_highlight {
        if let Some((vah, val)) = value_area {
            if idx >= val && idx <= vah { 1.0 } else { 0.4 }
        } else {
            1.0
        }
    } else {
        1.0
    };

    match config.vbp_type {
        VbpType::Volume | VbpType::DeltaAndTotalVolume => {
            coord::to_iced_color(config.volume_color, config.opacity * va_factor)
        }
        VbpType::BidAskVolume => {
            // Use bid color for bars (simplified for side panel)
            coord::to_iced_color(config.bid_color, config.opacity * va_factor)
        }
        VbpType::Delta | VbpType::DeltaPercentage => {
            let delta = level.buy_volume - level.sell_volume;
            if delta >= 0.0 {
                coord::to_iced_color(config.bid_color, config.opacity * va_factor)
            } else {
                coord::to_iced_color(config.ask_color, config.opacity * va_factor)
            }
        }
    }
}

/// Populate the resolved cache if not yet computed or stale.
fn ensure_resolved_cache(output: &ProfileOutput, config: &ProfileRenderConfig, state: &ViewState) {
    let tick_units = state.tick_size.units.max(1);

    let target_quantum = match output.grouping_mode {
        VbpGroupingMode::Automatic { factor } => {
            let dq = coord::compute_dynamic_quantum(
                state, 4.0, // MIN_ROW_PX
                factor, tick_units,
            );
            if dq > output.quantum {
                dq
            } else {
                output.quantum
            }
        }
        VbpGroupingMode::Manual => output.quantum,
    };

    {
        let cache = output
            .resolved_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(ref c) = *cache
            && c.quantum == target_quantum
        {
            return;
        }
    }

    use study::studies::orderflow::vbp::profile_core;

    let (levels, poc, value_area) = if target_quantum > output.quantum {
        let merged = merge_levels_to_quantum(&output.levels, target_quantum);
        let poc = profile_core::find_poc_index(&merged);
        let value_area = if config.va_config.show_value_area {
            poc.and_then(|idx| {
                profile_core::calculate_value_area(
                    &merged,
                    idx,
                    config.va_config.value_area_pct as f64,
                )
            })
        } else {
            None
        };
        (merged, poc, value_area)
    } else {
        (output.levels.clone(), output.poc, output.value_area)
    };

    *output
        .resolved_cache
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(VbpResolvedCache {
        quantum: target_quantum,
        levels,
        poc,
        value_area,
    });
}

fn merge_levels_to_quantum(
    levels: &[study::output::ProfileLevel],
    target_quantum: i64,
) -> Vec<study::output::ProfileLevel> {
    if levels.is_empty() {
        return Vec::new();
    }

    let mut merged = Vec::with_capacity(levels.len() / 2 + 1);
    let mut cur_bucket = (levels[0].price_units / target_quantum) * target_quantum;
    let mut buy_acc: f64 = 0.0;
    let mut sell_acc: f64 = 0.0;

    for level in levels {
        let bucket = (level.price_units / target_quantum) * target_quantum;
        if bucket != cur_bucket {
            merged.push(study::output::ProfileLevel {
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
    merged.push(study::output::ProfileLevel {
        price: Price::from_units(cur_bucket).to_f64(),
        price_units: cur_bucket,
        buy_volume: buy_acc as f32,
        sell_volume: sell_acc as f32,
    });

    merged
}

// ── Crosshair ─────────────────────────────────────────────────────────

fn draw_crosshair(frame: &mut Frame, y: f32, bounds: Size) {
    if y < 0.0 || y > bounds.height {
        return;
    }
    let line = Path::line(Point::new(0.0, y), Point::new(bounds.width, y));
    frame.stroke(
        &line,
        Stroke::default()
            .with_color(Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.35,
            })
            .with_width(1.0),
    );
}
