//! Custom canvas-based seek bar with volume histogram overlay.

use crate::config::UserTimezone;
use crate::services::VolumeBucket;
use data::TimeRange;
use iced::widget::canvas::{self, Frame, Geometry, LineDash, Path, Stroke, Text as CanvasText};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, mouse};

/// Space reserved above the trackbar for the hover tooltip.
const TOOLTIP_RESERVE: f32 = 24.0;
/// Height of the trackbar area itself.
const TRACKBAR_H: f32 = 32.0;

/// Interaction state for the trackbar.
#[derive(Debug, Clone, Copy, Default)]
pub enum Interaction {
    #[default]
    Idle,
    Dragging,
}

/// Canvas program that draws volume bars and a playhead.
struct VolumeTrackbarProgram<'a, Message> {
    buckets: &'a [VolumeBucket],
    progress: f32,
    time_range: Option<&'a TimeRange>,
    timezone: UserTimezone,
    on_seek: Box<dyn Fn(f32) -> Message + 'a>,
}

impl<Message> canvas::Program<Message> for VolumeTrackbarProgram<'_, Message> {
    type State = Interaction;

    fn update(
        &self,
        state: &mut Interaction,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds)
                    && pos.y >= TOOLTIP_RESERVE
                {
                    *state = Interaction::Dragging;
                    let progress = (pos.x / bounds.width).clamp(0.0, 1.0);
                    return Some(canvas::Action::publish((self.on_seek)(progress)));
                }
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if matches!(state, Interaction::Dragging)
                    && let Some(pos) = cursor.position_in(bounds)
                {
                    let progress = (pos.x / bounds.width).clamp(0.0, 1.0);
                    return Some(canvas::Action::publish((self.on_seek)(progress)));
                }
                if cursor.is_over(bounds) {
                    return Some(canvas::Action::request_redraw());
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if matches!(state, Interaction::Dragging) {
                    *state = Interaction::Idle;
                    return Some(canvas::Action::request_redraw());
                }
            }
            _ => {}
        }
        None
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let palette = theme.extended_palette();
        let primary = palette.primary.base.color;
        let bg_strong = palette.background.strong.color;
        let text_color = palette.background.base.text;

        let mut frame = Frame::new(renderer, bounds.size());
        let w = bounds.width;

        // The trackbar area starts below the tooltip reserve
        let top = TOOLTIP_RESERVE;
        let h = TRACKBAR_H;

        if self.buckets.is_empty() || w < 1.0 {
            return vec![frame.into_geometry()];
        }

        // Find max total volume for normalization
        let max_vol = self
            .buckets
            .iter()
            .map(|b| b.buy_volume + b.sell_volume)
            .fold(0.0_f32, f32::max);

        let num = self.buckets.len();
        let bar_w = (w / num as f32).max(1.0);
        let playhead_x = self.progress * w;
        let usable_h = h - 4.0;

        // Track line at vertical center of trackbar area
        let center_y = top + h / 2.0;
        frame.stroke(
            &Path::line(Point::new(0.0, center_y), Point::new(w, center_y)),
            Stroke::default().with_width(1.0).with_color(Color {
                a: 0.2,
                ..bg_strong
            }),
        );

        // Draw volume bars
        if max_vol > 0.0 {
            for (i, bucket) in self.buckets.iter().enumerate() {
                let total = bucket.buy_volume + bucket.sell_volume;
                let bar_h = (total / max_vol) * usable_h;
                let x = i as f32 * bar_w;
                let y = top + h - 2.0 - bar_h;

                let is_played = x + bar_w <= playhead_x;
                let is_partial = x < playhead_x && x + bar_w > playhead_x;

                if is_played {
                    frame.fill_rectangle(
                        Point::new(x, y),
                        Size::new(bar_w - 0.5, bar_h),
                        Color { a: 0.5, ..primary },
                    );
                } else if is_partial {
                    let played_w = playhead_x - x;
                    let unplayed_w = bar_w - 0.5 - played_w;
                    frame.fill_rectangle(
                        Point::new(x, y),
                        Size::new(played_w, bar_h),
                        Color { a: 0.5, ..primary },
                    );
                    if unplayed_w > 0.0 {
                        frame.fill_rectangle(
                            Point::new(x + played_w, y),
                            Size::new(unplayed_w, bar_h),
                            Color {
                                a: 0.3,
                                ..bg_strong
                            },
                        );
                    }
                } else {
                    frame.fill_rectangle(
                        Point::new(x, y),
                        Size::new(bar_w - 0.5, bar_h),
                        Color {
                            a: 0.3,
                            ..bg_strong
                        },
                    );
                }
            }
        }

        // Playhead: 2px vertical line
        frame.fill_rectangle(
            Point::new(playhead_x - 1.0, top),
            Size::new(2.0, h),
            primary,
        );

        // Hover cursor line + timestamp tooltip
        if let Some(pos) = cursor.position_in(bounds) {
            let hover_x = pos.x.clamp(0.0, w);

            // Dashed vertical cursor line (trackbar area only)
            let dash_line = Path::line(Point::new(hover_x, top), Point::new(hover_x, top + h));
            frame.stroke(
                &dash_line,
                Stroke {
                    width: 1.0,
                    line_dash: LineDash {
                        segments: &[3.0, 3.0],
                        offset: 0,
                    },
                    ..Stroke::default().with_color(Color {
                        a: 0.6,
                        ..text_color
                    })
                },
            );

            // Timestamp tooltip
            if let Some(range) = self.time_range {
                let hover_progress = hover_x / w;
                let start_ms = range.start.to_millis();
                let end_ms = range.end.to_millis();
                let hover_ms = start_ms + ((end_ms - start_ms) as f32 * hover_progress) as u64;

                let label = self.timezone.format_replay_tooltip(hover_ms as i64);
                if !label.is_empty() {
                    let font_size = 10.0_f32;
                    let pad_x = 6.0_f32;
                    let pad_y = 4.0_f32;
                    let radius = 3.0_f32;
                    let tip_w = label.len() as f32 * 6.0 + pad_x * 2.0;
                    let tip_h = font_size + pad_y * 2.0;
                    let gap = 4.0_f32;

                    // Center on hover_x, clamped within canvas width
                    let tip_x = (hover_x - tip_w / 2.0).clamp(0.0, w - tip_w);
                    // Right above the trackbar area
                    let tip_y = top - tip_h - gap;

                    // Rounded rect path
                    let tip_rect = Path::new(|b| {
                        b.move_to(Point::new(tip_x + radius, tip_y));
                        b.line_to(Point::new(tip_x + tip_w - radius, tip_y));
                        b.arc_to(
                            Point::new(tip_x + tip_w, tip_y),
                            Point::new(tip_x + tip_w, tip_y + radius),
                            radius,
                        );
                        b.line_to(Point::new(tip_x + tip_w, tip_y + tip_h - radius));
                        b.arc_to(
                            Point::new(tip_x + tip_w, tip_y + tip_h),
                            Point::new(tip_x + tip_w - radius, tip_y + tip_h),
                            radius,
                        );
                        b.line_to(Point::new(tip_x + radius, tip_y + tip_h));
                        b.arc_to(
                            Point::new(tip_x, tip_y + tip_h),
                            Point::new(tip_x, tip_y + tip_h - radius),
                            radius,
                        );
                        b.line_to(Point::new(tip_x, tip_y + radius));
                        b.arc_to(
                            Point::new(tip_x, tip_y),
                            Point::new(tip_x + radius, tip_y),
                            radius,
                        );
                    });

                    // Fill + outline
                    frame.fill(
                        &tip_rect,
                        Color {
                            a: 0.9,
                            ..bg_strong
                        },
                    );
                    frame.stroke(
                        &tip_rect,
                        Stroke::default().with_width(1.0).with_color(Color {
                            a: 0.6,
                            ..bg_strong
                        }),
                    );

                    // Text centered in the pill
                    frame.fill_text(CanvasText {
                        content: label,
                        position: Point::new(tip_x + pad_x, tip_y + pad_y),
                        size: iced::Pixels(font_size),
                        color: text_color,
                        ..CanvasText::default()
                    });
                }
            }
        }

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Interaction,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if matches!(state, Interaction::Dragging) {
            return mouse::Interaction::Grabbing;
        }
        if cursor.is_over(bounds) {
            return mouse::Interaction::Pointer;
        }
        mouse::Interaction::default()
    }
}

/// Create a volume trackbar element.
pub fn volume_trackbar<'a, Message: 'a>(
    buckets: &'a [VolumeBucket],
    progress: f32,
    time_range: Option<&'a TimeRange>,
    timezone: UserTimezone,
    on_seek: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    canvas(VolumeTrackbarProgram {
        buckets,
        progress,
        time_range,
        timezone,
        on_seek: Box::new(on_seek),
    })
    .height(Length::Fixed(TOOLTIP_RESERVE + TRACKBAR_H))
    .width(Length::Fill)
    .into()
}

fn canvas<'a, P, Message>(program: P) -> iced::widget::Canvas<P, Message, Theme, Renderer>
where
    P: canvas::Program<Message> + 'a,
{
    iced::widget::Canvas::new(program)
}
