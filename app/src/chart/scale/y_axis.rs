use super::label::{AxisLabel, LabelContent, calc_label_rect};
use super::{Interaction, Message, linear};
use data::ChartBasis;
use data::util::round_to_tick;
use iced::{
    Color, Rectangle, Renderer, Size, Theme, mouse,
    widget::canvas::{self, Cache, Geometry},
};

// Y-AXIS LABELS
pub struct AxisLabelsY<'a> {
    pub labels_cache: &'a Cache,
    pub translation_y: f32,
    pub scaling: f32,
    pub min: f32,
    pub last_price: Option<linear::PriceInfoLabel>,
    pub tick_size: f32,
    pub decimals: usize,
    pub cell_height: f32,
    pub basis: ChartBasis,
    pub chart_bounds: Rectangle,
    /// Y position (in side-panel canvas coords) from a side-panel hover.
    /// When set, draw a price label on the Y-axis aligned to that position.
    pub crosshair_y: Option<f32>,
}

impl AxisLabelsY<'_> {
    fn visible_region(&self, size: Size) -> Rectangle {
        let width = size.width / self.scaling;
        let height = size.height / self.scaling;

        Rectangle {
            x: 0.0,
            y: -self.translation_y - height / 2.0,
            width,
            height,
        }
    }

    fn y_to_price(&self, y: f32) -> f32 {
        self.min - (y / self.cell_height) * self.tick_size
    }
}

impl canvas::Program<Message> for AxisLabelsY<'_> {
    type State = Interaction;

    fn update(
        &self,
        interaction: &mut Interaction,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if let iced::Event::Mouse(mouse::Event::ButtonReleased(_)) = event {
            *interaction = Interaction::None;
        }

        let cursor_position = cursor.position_in(bounds)?;

        if let iced::Event::Mouse(mouse_event) = event {
            match mouse_event {
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    *interaction = Interaction::Zoomin {
                        last_position: cursor_position,
                    };
                }
                mouse::Event::CursorMoved { .. } => {
                    if let Interaction::Zoomin {
                        ref mut last_position,
                    } = *interaction
                    {
                        let difference_y = last_position.y - cursor_position.y;

                        if difference_y.abs() > 1.0 {
                            *last_position = cursor_position;

                            let message = Message::YScaling(difference_y * 0.15, 0.0, false);

                            return Some(canvas::Action::publish(message).and_capture());
                        }
                    }
                }
                mouse::Event::WheelScrolled { delta } => match delta {
                    mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {
                        let message = Message::YScaling(
                            *y,
                            {
                                if let Some(cursor_to_center) =
                                    cursor.position_from(bounds.center())
                                {
                                    cursor_to_center.y
                                } else {
                                    0.0
                                }
                            },
                            true,
                        );

                        return Some(canvas::Action::publish(message).and_capture());
                    }
                },
                _ => {}
            }
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
        let text_size = 12.0;
        let palette = theme.extended_palette();

        let labels = self.labels_cache.draw(renderer, bounds.size(), |frame| {
            let region = self.visible_region(frame.size());

            let highest = self.y_to_price(region.y);
            let lowest = self.y_to_price(region.y + region.height);

            let range = highest - lowest;

            let mut all_labels = linear::generate_labels(
                bounds,
                lowest,
                highest,
                text_size,
                palette.background.base.text,
                Some(self.decimals),
            );

            // Last price (priority 2)
            if let Some(label) = self.last_price {
                let candle_close_label = match self.basis {
                    ChartBasis::Time(timeframe) => {
                        let interval = timeframe.to_milliseconds();

                        let current_time = chrono::Utc::now().timestamp_millis() as u64;
                        let next_kline_open = (current_time / interval + 1) * interval;

                        let remaining_seconds = (next_kline_open - current_time) / 1000;

                        if remaining_seconds > 0 {
                            let hours = remaining_seconds / 3600;
                            let minutes = (remaining_seconds % 3600) / 60;
                            let seconds = remaining_seconds % 60;

                            let time_format = if hours > 0 {
                                format!("{hours:02}:{minutes:02}:{seconds:02}")
                            } else {
                                format!("{minutes:02}:{seconds:02}")
                            };

                            Some(LabelContent {
                                content: time_format,
                                background_color: Some(palette.background.strong.color),
                                text_color: if palette.is_dark {
                                    Color::BLACK.scale_alpha(0.8)
                                } else {
                                    Color::WHITE.scale_alpha(0.8)
                                },
                                text_size: 11.0,
                            })
                        } else {
                            None
                        }
                    }
                    ChartBasis::Tick(_) => None,
                };

                let (price, color) = label.get_with_color(palette);
                let price = price.to_f32();

                let price_label = LabelContent {
                    content: format!("{:.*}", self.decimals, price),
                    background_color: Some(color),
                    text_color: {
                        if candle_close_label.is_some() {
                            if palette.is_dark {
                                Color::BLACK
                            } else {
                                Color::WHITE
                            }
                        } else {
                            palette.primary.strong.text
                        }
                    },
                    text_size: 12.0,
                };

                let y_pos = bounds.height - ((price - lowest) / range * bounds.height);
                let content_amt = if candle_close_label.is_some() { 2 } else { 1 };

                all_labels.push(AxisLabel::Y {
                    bounds: calc_label_rect(y_pos, content_amt, text_size, bounds),
                    value_label: price_label,
                    timer_label: candle_close_label,
                });
            }

            // Crosshair price label
            if let Some(crosshair_pos) = cursor.position_in(self.chart_bounds) {
                let rounded_price = round_to_tick(
                    lowest + (range * (bounds.height - crosshair_pos.y) / bounds.height),
                    self.tick_size,
                );
                let y_position = bounds.height - ((rounded_price - lowest) / range * bounds.height);

                let label = LabelContent {
                    content: format!("{:.*}", self.decimals, rounded_price),
                    background_color: Some(palette.secondary.base.color),
                    text_color: palette.secondary.base.text,
                    text_size: 12.0,
                };

                all_labels.push(AxisLabel::Y {
                    bounds: calc_label_rect(y_position, 1, text_size, bounds),
                    value_label: label,
                    timer_label: None,
                });
            } else if let Some(side_y) = self.crosshair_y {
                // Cursor is in the side panel — mirror price label on the Y-axis
                let rounded_price = round_to_tick(
                    lowest + (range * (bounds.height - side_y) / bounds.height),
                    self.tick_size,
                );
                let y_position = bounds.height - ((rounded_price - lowest) / range * bounds.height);

                let label = LabelContent {
                    content: format!("{:.*}", self.decimals, rounded_price),
                    background_color: Some(palette.secondary.base.color),
                    text_color: palette.secondary.base.text,
                    text_size: 12.0,
                };

                all_labels.push(AxisLabel::Y {
                    bounds: calc_label_rect(y_position, 1, text_size, bounds),
                    value_label: label,
                    timer_label: None,
                });
            }

            AxisLabel::filter_and_draw(&all_labels, frame);
        });

        vec![labels]
    }

    fn mouse_interaction(
        &self,
        interaction: &Interaction,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match interaction {
            Interaction::Zoomin { .. } => mouse::Interaction::ResizingVertically,
            Interaction::Panning { .. } => mouse::Interaction::None,
            Interaction::None if cursor.is_over(bounds) => mouse::Interaction::ResizingVertically,
            _ => mouse::Interaction::default(),
        }
    }
}
