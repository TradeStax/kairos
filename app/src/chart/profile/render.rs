use super::ProfileChart;
use crate::chart::drawing;
use crate::chart::{Chart, ChartState, Interaction, Message, TEXT_SIZE, base_mouse_interaction};
use crate::components::primitives::AZERET_MONO;
use crate::style::tokens::{spacing, text};
use iced::theme::palette::Extended;
use iced::widget::canvas::{self, Event, Frame, Geometry, Text};
use iced::{Point, Rectangle, Renderer, Size, Theme, Vector, mouse};

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

        // Extract the computed profiles + render config.
        let (profiles, render_config) = match self.profiles_and_config() {
            Some((ps, c)) => (ps, c),
            _ => return vec![],
        };
        if bounds.width == 0.0 || profiles.is_empty() {
            return vec![];
        }

        // Tooltip uses an iterator over all profile levels — no Vec needed.
        // The actual iteration happens in draw_profile_tooltip below.

        let bounds_size = bounds.size();
        let palette = theme.extended_palette();

        // ── Main cache layer ─────────────────────────────────────────
        let main_layer = chart.cache.main.draw(renderer, bounds_size, |frame| {
            let center = Vector::new(bounds.width / 2.0, bounds.height / 2.0);

            frame.translate(center);
            frame.scale(chart.scaling);
            frame.translate(chart.translation);

            // Render profiles using time-based positioning
            crate::chart::study_renderer::vbp::render_vbp_multi(
                frame,
                profiles,
                render_config,
                chart,
                bounds_size,
            );

            // ── Overlay studies ───────────────────────────────────
            for s in &self.studies {
                let output = s.output();
                let placement = s.placement();
                if !matches!(output, study::StudyOutput::Empty)
                    && matches!(
                        placement,
                        study::StudyPlacement::Overlay | study::StudyPlacement::Background
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
        let drawings_layer = chart.cache.drawings.draw(renderer, bounds_size, |frame| {
            drawing::render::draw_completed_drawings(
                frame,
                chart,
                &self.drawings,
                bounds_size,
                palette,
            );
        });

        // ── Crosshair cache layer ────────────────────────────────────
        let crosshair = chart.cache.crosshair.draw(renderer, bounds_size, |frame| {
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
                let result = crate::chart::overlay::draw_crosshair(
                    chart,
                    frame,
                    theme,
                    bounds_size,
                    cursor_position,
                    interaction,
                );

                chart.crosshair.interval.set(Some(result.interval));

                // Profile tooltip
                draw_profile_tooltip(
                    profiles,
                    &self.ticker_info,
                    frame,
                    palette,
                    chart,
                    cursor_position,
                    bounds_size,
                );
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
                // Remote crosshair from linked pane
                crate::chart::overlay::draw_remote_crosshair(
                    chart,
                    frame,
                    theme,
                    bounds_size,
                    interval,
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
        if let Some(i) = base_mouse_interaction(&state.interaction, bounds, cursor) {
            return i;
        }
        if cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

/// Draw profile tooltip showing volume at cursor price.
fn draw_profile_tooltip(
    profiles: &[study::output::ProfileOutput],
    ticker_info: &data::FuturesTickerInfo,
    frame: &mut Frame,
    palette: &Extended,
    chart: &crate::chart::ViewState,
    cursor_position: Point,
    bounds: Size,
) {
    // Convert cursor Y to chart-space Y, then to price
    let chart_y = (cursor_position.y - bounds.height / 2.0) / chart.scaling - chart.translation.y;
    let price = chart.y_to_price(chart_y);
    let price_units = price.units();

    // Find nearest level across all profiles (no Vec allocation)
    let Some(nearest) = profiles
        .iter()
        .flat_map(|p| p.levels.iter())
        .min_by_key(|l| (l.price_units - price_units).abs())
    else {
        return;
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

    let position = Point::new(spacing::MD, spacing::MD);

    let tooltip_rect = Rectangle {
        x: position.x,
        y: position.y,
        width: total_width,
        height: spacing::XL,
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
            size: iced::Pixels(text::BODY),
            color: seg_color,
            font: AZERET_MONO,
            ..Text::default()
        });
        x += text.len() as f32 * spacing::MD;
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
        format!("-{formatted}")
    } else {
        formatted
    }
}
