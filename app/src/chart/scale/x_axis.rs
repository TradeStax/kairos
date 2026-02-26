use super::label::{AxisLabel, REGULAR_LABEL_WIDTH};
use super::{Interaction, Message, timeseries};
use crate::chart::core::x_to_interval as x_to_interval_fn;
use data::Autoscale;
use data::ChartBasis;
use iced::{
    Point, Rectangle, Renderer, Size, Theme, mouse,
    widget::canvas::{self, Cache, Geometry},
};

// X-AXIS LABELS
pub struct AxisLabelsX<'a> {
    pub labels_cache: &'a Cache,
    pub max: u64,
    pub scaling: f32,
    pub translation_x: f32,
    pub basis: ChartBasis,
    pub cell_width: f32,
    pub timezone: crate::config::UserTimezone,
    pub chart_bounds: Rectangle,
    pub interval_keys: Option<Vec<u64>>,
    pub autoscaling: Option<Autoscale>,
    /// Remote crosshair interval from a linked pane
    pub remote_crosshair: Option<u64>,
    /// Current crosshair interval (from main chart or study panel)
    pub crosshair_interval: Option<u64>,
}

impl AxisLabelsX<'_> {
    fn generate_remote_crosshair_label(
        &self,
        interval: u64,
        region: Rectangle,
        bounds: Rectangle,
        palette: &iced::theme::palette::Extended,
    ) -> Option<AxisLabel> {
        match self.basis {
            ChartBasis::Tick(agg) => {
                let interval_keys = self.interval_keys.as_ref()?;
                let agg_val = u64::from(agg);
                if agg_val == 0 {
                    return None;
                }
                let cell_index = -(interval as f32 / agg_val as f32);
                let chart_x = cell_index * self.cell_width;

                let x_min = region.x;
                let x_max = region.x + region.width;
                let range = x_max - x_min;
                if range.abs() < f32::EPSILON {
                    return None;
                }

                let snap_x = ((chart_x - x_min) / range) * bounds.width;
                if snap_x < 0.0 || snap_x > bounds.width {
                    return None;
                }

                let last_index = interval_keys.len().checked_sub(1)?;
                let offset = (-cell_index).round() as usize;
                if offset > last_index {
                    return None;
                }
                let array_index = last_index - offset;
                let timestamp = interval_keys.get(array_index)?;

                let label_text = self
                    .timezone
                    .format_crosshair_label(*timestamp as i64, agg.into());
                Some(AxisLabel::new_x(snap_x, label_text, bounds, true, palette))
            }
            ChartBasis::Time(timeframe) => {
                let x_min = self.x_to_interval(region.x) as f64;
                let x_max = self.x_to_interval(region.x + region.width) as f64;
                let range = x_max - x_min;
                if range.abs() < f64::EPSILON {
                    return None;
                }

                let snap_ratio = (interval as f64 - x_min) / range;
                let snap_x = snap_ratio * f64::from(bounds.width);
                if snap_x.is_nan() || snap_x < 0.0 || snap_x > f64::from(bounds.width) {
                    return None;
                }

                let tf_ms = timeframe.to_milliseconds();
                let label_text = self.timezone.format_crosshair_label(interval as i64, tf_ms);
                Some(AxisLabel::new_x(
                    snap_x as f32,
                    label_text,
                    bounds,
                    true,
                    palette,
                ))
            }
        }
    }

    fn calc_crosshair_pos(&self, cursor_pos: Point, region: Rectangle) -> (f32, f32, i32) {
        let crosshair_ratio = f64::from(cursor_pos.x) / f64::from(self.chart_bounds.width);
        let chart_x_min = region.x;
        let crosshair_pos = chart_x_min + crosshair_ratio as f32 * region.width;
        let cell_index = (crosshair_pos / self.cell_width).round();

        (crosshair_pos, crosshair_ratio as f32, cell_index as i32)
    }

    fn generate_crosshair(
        &self,
        cursor_pos: Point,
        region: Rectangle,
        bounds: Rectangle,
        palette: &iced::theme::palette::Extended,
    ) -> Option<AxisLabel> {
        match self.basis {
            ChartBasis::Tick(interval) => {
                let Some(interval_keys) = &self.interval_keys else {
                    return None;
                };

                let (crosshair_pos, _, cell_index) = self.calc_crosshair_pos(cursor_pos, region);

                let chart_x_min = region.x;
                let chart_x_max = region.x + region.width;

                let snapped_position = (crosshair_pos / self.cell_width).round() * self.cell_width;
                let snap_ratio = (snapped_position - chart_x_min) / (chart_x_max - chart_x_min);
                let snap_x = snap_ratio * bounds.width;

                if snap_x.is_nan() || snap_x < 0.0 || snap_x > bounds.width {
                    return None;
                }

                let last_index = interval_keys.len() - 1;
                let offset = i64::from(-cell_index) as usize;
                if offset > last_index {
                    return None;
                }

                let array_index = last_index - offset;

                if let Some(timestamp) = interval_keys.get(array_index) {
                    let label_text = self
                        .timezone
                        .format_crosshair_label(*timestamp as i64, interval.into());

                    return Some(AxisLabel::new_x(snap_x, label_text, bounds, true, palette));
                }
            }
            ChartBasis::Time(timeframe) => {
                let (_, crosshair_ratio, _) = self.calc_crosshair_pos(cursor_pos, region);

                let x_min = self.x_to_interval(region.x);
                let x_max = self.x_to_interval(region.x + region.width);

                let crosshair_millis =
                    x_min as f64 + f64::from(crosshair_ratio) * (x_max as f64 - x_min as f64);

                let interval = timeframe.to_milliseconds();

                let crosshair_time =
                    chrono::DateTime::from_timestamp_millis(crosshair_millis as i64)?;
                let rounded_timestamp =
                    (crosshair_time.timestamp_millis() as f64 / (interval as f64)).round() as u64
                        * interval;

                let snap_ratio =
                    (rounded_timestamp as f64 - x_min as f64) / (x_max as f64 - x_min as f64);

                let snap_x = snap_ratio * f64::from(bounds.width);
                if snap_x.is_nan() || snap_x < 0.0 || snap_x > f64::from(bounds.width) {
                    return None;
                }

                let label_text = self
                    .timezone
                    .format_crosshair_label(rounded_timestamp as i64, interval);

                return Some(AxisLabel::new_x(
                    snap_x as f32,
                    label_text,
                    bounds,
                    true,
                    palette,
                ));
            }
        }

        None
    }

    fn visible_region(&self, size: Size) -> Rectangle {
        let width = size.width / self.scaling;
        let height = size.height / self.scaling;

        Rectangle {
            x: -self.translation_x - width / 2.0,
            y: 0.0,
            width,
            height,
        }
    }

    fn x_to_interval(&self, x: f32) -> u64 {
        x_to_interval_fn(x, self.max as f64, self.cell_width, &self.basis)
    }
}

impl canvas::Program<Message> for AxisLabelsX<'_> {
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
                        let difference_x = last_position.x - cursor_position.x;

                        if difference_x.abs() > 1.0 {
                            *last_position = cursor_position;

                            let delta = if self.autoscaling == Some(Autoscale::FitAll) {
                                difference_x * 0.02
                            } else {
                                difference_x * 0.08
                            };

                            let message = Message::XScaling(delta, 0.0, false);

                            return Some(canvas::Action::publish(message).and_capture());
                        }
                    }
                }
                mouse::Event::WheelScrolled { delta } => match delta {
                    mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {
                        let message = Message::XScaling(
                            *y,
                            {
                                if let Some(cursor_to_center) =
                                    cursor.position_from(bounds.center())
                                {
                                    cursor_to_center.x
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
        let palette = theme.extended_palette();

        // The time axis canvas may be wider than the main chart canvas when a
        // side panel is active.  All label X positions must align with the
        // candles above, so we use chart_bounds.width as the X reference —
        // the same width the main chart uses for its frame transforms.
        let chart_w = if self.chart_bounds.width > 0.0 {
            self.chart_bounds.width
        } else {
            bounds.width
        };
        // Axis bounds rectangle that carries chart_w as its width.
        // Used for label positioning so coordinates match the chart above.
        let chart_axis_bounds = Rectangle {
            width: chart_w,
            ..bounds
        };

        let labels = self.labels_cache.draw(renderer, bounds.size(), |frame| {
            // Compute visible region from the main chart width so the time
            // range exactly matches what the candles show.
            let region = self.visible_region(Size::new(chart_w, frame.size().height));

            let target_spacing = REGULAR_LABEL_WIDTH * 2.0;
            let target_count = (chart_w / target_spacing).floor() as usize;

            let label_count = target_count.max(2);

            let mut labels: Vec<AxisLabel> = Vec::with_capacity(label_count + 1); // +1 for crosshair

            match self.basis {
                ChartBasis::Tick(_) => {
                    if let Some(interval_keys) = &self.interval_keys {
                        let last_idx = interval_keys.len() - 1;
                        let mut last_x: Option<f32> = None;
                        let mut prev_date: Option<(i32, u32, u32)> = None;
                        let mut date_labels: Vec<AxisLabel> = Vec::new();

                        for (i, timestamp) in interval_keys.iter().enumerate() {
                            let cell_index = -(last_idx as i32) + i as i32;
                            let x_position = cell_index as f32 * self.cell_width;

                            let x_min_region = region.x;
                            let x_max_region = region.x + region.width;
                            let snap_ratio = if (x_max_region - x_min_region).abs() < f32::EPSILON {
                                0.5
                            } else {
                                (x_position - x_min_region) / (x_max_region - x_min_region)
                            };
                            let snap_x = snap_ratio * chart_w;

                            if last_x.is_none_or(|lx| (snap_x - lx).abs() >= target_spacing) {
                                let ts_secs = (*timestamp / 1000) as i64;
                                let current_date = self.timezone.date_components(ts_secs);

                                let is_date_label = match (current_date, prev_date) {
                                    (Some(_), None) => true,
                                    (Some(cur), Some(prev)) => cur != prev,
                                    _ => false,
                                };

                                if let Some(d) = current_date {
                                    prev_date = Some(d);
                                }

                                if is_date_label {
                                    let label_text =
                                        self.timezone.format_date_boundary(ts_secs);
                                    date_labels.push(AxisLabel::new_x(
                                        snap_x,
                                        label_text,
                                        chart_axis_bounds,
                                        false,
                                        palette,
                                    ));
                                } else {
                                    let label_text = self.timezone.format_timestamp(
                                        ts_secs,
                                        data::Timeframe::M1s,
                                    );
                                    labels.push(AxisLabel::new_x(
                                        snap_x,
                                        label_text,
                                        chart_axis_bounds,
                                        false,
                                        palette,
                                    ));
                                }

                                last_x = Some(snap_x);
                            }
                        }

                        // Date labels after time labels for collision priority
                        labels.extend(date_labels);
                    }
                }
                ChartBasis::Time(timeframe) => {
                    let x_min_region = self.x_to_interval(region.x);
                    let x_max_region = self.x_to_interval(region.x + region.width);

                    let generated_labels = timeseries::generate_time_labels(
                        timeframe,
                        self.timezone,
                        chart_axis_bounds, // use chart_w for pixel positions
                        x_min_region,
                        x_max_region,
                        label_count as i32,
                        palette,
                    );

                    labels.extend(generated_labels);
                }
            }

            if let Some(cursor_pos) = cursor.position_in(self.chart_bounds)
                && let Some(label) =
                    self.generate_crosshair(cursor_pos, region, chart_axis_bounds, palette)
            {
                labels.push(label);
            } else if let Some(interval) = self.crosshair_interval.or(self.remote_crosshair) {
                // Show time label for crosshair from study panel or linked pane
                if let Some(label) = self.generate_remote_crosshair_label(
                    interval,
                    region,
                    chart_axis_bounds,
                    palette,
                ) {
                    labels.push(label);
                }
            }

            AxisLabel::filter_and_draw(&labels, frame);
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
            Interaction::Panning { .. } => mouse::Interaction::None,
            Interaction::Zoomin { .. } => mouse::Interaction::ResizingHorizontally,
            Interaction::None if cursor.is_over(bounds) => mouse::Interaction::ResizingHorizontally,
            _ => mouse::Interaction::default(),
        }
    }
}
