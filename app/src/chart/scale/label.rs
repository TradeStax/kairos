use crate::chart::TEXT_SIZE;
use crate::components::primitives::AZERET_MONO;

use iced::{
    Alignment, Color, Point, Rectangle, Size,
    theme::palette::Extended,
    widget::canvas::{self, Frame},
};

pub const REGULAR_LABEL_WIDTH: f32 = TEXT_SIZE * 6.0;

/// calculates `Rectangle` from given content, clamps it within bounds if needed
pub fn calc_label_rect(
    y_pos: f32,
    content_amt: i16,
    text_size: f32,
    bounds: Rectangle,
) -> Rectangle {
    let content_amt = content_amt.max(1);
    let label_height = text_size + (f32::from(content_amt) * (text_size / 2.0) + 4.0);

    let rect = Rectangle {
        x: 1.0,
        y: y_pos - label_height / 2.0,
        width: bounds.width - 1.0,
        height: label_height,
    };

    // clamp when label is partially visible within bounds
    if rect.y < bounds.height && rect.y + label_height > 0.0 {
        Rectangle {
            y: rect.y.clamp(0.0, (bounds.height - label_height).max(0.0)),
            ..rect
        }
    } else {
        rect
    }
}

#[derive(Debug, Clone)]
pub struct LabelContent {
    pub content: String,
    pub background_color: Option<Color>,
    pub text_color: Color,
    pub text_size: f32,
}

#[derive(Debug, Clone)]
pub enum AxisLabel {
    X {
        bounds: Rectangle,
        label: LabelContent,
    },
    Y {
        bounds: Rectangle,
        value_label: LabelContent,
        timer_label: Option<LabelContent>,
    },
}

impl AxisLabel {
    pub fn new_x(
        center_x_position: f32,
        text_content: String,
        axis_bounds: Rectangle,
        is_crosshair: bool,
        palette: &Extended,
    ) -> Self {
        let content_width = text_content.len() as f32 * (TEXT_SIZE / 2.6);

        let rect = Rectangle {
            x: center_x_position - content_width,
            y: 4.0,
            width: 2.0 * content_width,
            height: axis_bounds.height - 8.0,
        };

        let label = LabelContent {
            content: text_content,
            background_color: if is_crosshair {
                Some(palette.secondary.base.color)
            } else {
                None
            },
            text_color: if is_crosshair {
                palette.secondary.base.text
            } else {
                palette.background.base.text
            },
            text_size: TEXT_SIZE,
        };

        AxisLabel::X {
            bounds: rect,
            label,
        }
    }

    fn intersects(&self, other: &AxisLabel) -> bool {
        match (self, other) {
            (
                AxisLabel::Y {
                    bounds: self_rect, ..
                },
                AxisLabel::Y {
                    bounds: other_rect, ..
                },
            )
            | (
                AxisLabel::X {
                    bounds: self_rect, ..
                },
                AxisLabel::X {
                    bounds: other_rect, ..
                },
            ) => self_rect.intersects(other_rect),
            _ => false,
        }
    }

    pub fn filter_and_draw(labels: &[AxisLabel], frame: &mut Frame) {
        for i in (0..labels.len()).rev() {
            let should_draw = labels[i + 1..]
                .iter()
                .all(|existing| !existing.intersects(&labels[i]));

            if should_draw {
                labels[i].draw(frame);
            }
        }
    }

    fn draw(&self, frame: &mut Frame) {
        match self {
            AxisLabel::X { bounds, label } => {
                let frame_bounds = frame.size();
                if bounds.x + bounds.width < 0.0 || bounds.x > frame_bounds.width {
                    return;
                }

                if let Some(bg) = label.background_color {
                    frame.fill_rectangle(
                        Point::new(bounds.x, bounds.y),
                        Size::new(bounds.width, bounds.height),
                        bg,
                    );
                }

                frame.fill_text(canvas::Text {
                    content: label.content.clone(),
                    position: bounds.center(),
                    size: label.text_size.into(),
                    color: label.text_color,
                    align_y: Alignment::Center.into(),
                    align_x: Alignment::Center.into(),
                    font: AZERET_MONO,
                    ..canvas::Text::default()
                });
            }
            AxisLabel::Y {
                bounds,
                value_label,
                timer_label,
            } => {
                if let Some(background_color) = value_label.background_color {
                    frame.fill_rectangle(
                        Point::new(bounds.x, bounds.y),
                        Size::new(bounds.width, bounds.height),
                        background_color,
                    );
                }

                if let Some(timer_label) = timer_label {
                    let value_label = canvas::Text {
                        content: value_label.content.clone(),
                        position: Point::new(bounds.x + 4.0, bounds.y + 2.0),
                        color: value_label.text_color,
                        size: value_label.text_size.into(),
                        font: AZERET_MONO,
                        ..canvas::Text::default()
                    };

                    frame.fill_text(value_label);

                    let timer_label = canvas::Text {
                        content: timer_label.content.clone(),
                        position: Point::new(bounds.x + 4.0, bounds.y + 15.0),
                        color: timer_label.text_color,
                        size: timer_label.text_size.into(),
                        font: AZERET_MONO,
                        ..canvas::Text::default()
                    };

                    frame.fill_text(timer_label);
                } else {
                    let value_label = canvas::Text {
                        content: value_label.content.clone(),
                        position: Point::new(bounds.x + 4.0, bounds.y + 4.0),
                        color: value_label.text_color,
                        size: value_label.text_size.into(),
                        font: AZERET_MONO,
                        ..canvas::Text::default()
                    };

                    frame.fill_text(value_label);
                }
            }
        }
    }
}
