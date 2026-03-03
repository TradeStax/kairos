//! Historical dataset preview rendering
//!
//! Contains the price line chart canvas, preview data types, and the
//! historical panel view method.

use crate::components;
use crate::style;
use crate::style::{palette, tokens};
use data::{Connection, HistoricalDatasetInfo};
use iced::{
    Alignment, Color, Element, Length,
    widget::{canvas, column, container, row, scrollable, space, text_input},
};

use super::{DataFeedsMessage, DataFeedsModal};

// ── Preview data for historical datasets ──────────────────────────────

/// Preview data loaded for a historical dataset
#[derive(Debug, Clone)]
pub struct PreviewData {
    pub feed_id: data::FeedId,
    pub price_line: Vec<(u64, f64)>,
    pub trades: Vec<TradePreviewRow>,
    pub total_trades: usize,
}

#[derive(Debug, Clone)]
pub struct TradePreviewRow {
    pub time: String,
    pub price: String,
    pub size: String,
    pub side: String,
}

// ── Price line chart (canvas) ─────────────────────────────────────────

pub(super) struct PriceLineChart {
    pub(super) points: Vec<(u64, f64)>,
}

impl<Message> canvas::Program<Message> for PriceLineChart {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        if self.points.len() < 2 {
            return vec![];
        }

        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let palette = theme.extended_palette();
        let line_color = palette.primary.base.color;

        let (min_t, max_t) = self
            .points
            .iter()
            .fold((u64::MAX, u64::MIN), |(lo, hi), (t, _)| {
                (lo.min(*t), hi.max(*t))
            });
        let (min_p, max_p) = self
            .points
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), (_, p)| {
                (lo.min(*p), hi.max(*p))
            });

        let t_range = (max_t - min_t).max(1) as f64;
        let p_range = (max_p - min_p).max(0.01);
        let w = bounds.width;
        let h = bounds.height;
        let pad = 4.0;

        let to_point = |t: u64, p: f64| -> iced::Point {
            let x = pad + ((t - min_t) as f64 / t_range) as f32 * (w - 2.0 * pad);
            let y = pad + (1.0 - ((p - min_p) / p_range) as f32) * (h - 2.0 * pad);
            iced::Point::new(x, y)
        };

        // Build line path
        let mut builder = canvas::path::Builder::new();
        let first = self.points[0];
        builder.move_to(to_point(first.0, first.1));
        for &(t, p) in &self.points[1..] {
            builder.line_to(to_point(t, p));
        }
        let line_path = builder.build();

        frame.stroke(
            &line_path,
            canvas::Stroke::default()
                .with_color(line_color)
                .with_width(1.5),
        );

        // Fill area under line
        let mut fill_builder = canvas::path::Builder::new();
        let first_pt = to_point(first.0, first.1);
        fill_builder.move_to(iced::Point::new(first_pt.x, h));
        fill_builder.line_to(first_pt);
        for &(t, p) in &self.points[1..] {
            fill_builder.line_to(to_point(t, p));
        }
        let Some(last) = self.points.last() else {
            return vec![frame.into_geometry()];
        };
        let last_pt = to_point(last.0, last.1);
        fill_builder.line_to(iced::Point::new(last_pt.x, h));
        fill_builder.close();

        frame.fill(
            &fill_builder.build(),
            Color {
                a: 0.1,
                ..line_color
            },
        );

        vec![frame.into_geometry()]
    }
}

// ── Historical panel view ─────────────────────────────────────────────

impl DataFeedsModal {
    pub(super) fn view_historical_panel<'a>(
        &'a self,
        feed: &'a Connection,
        info: &'a HistoricalDatasetInfo,
    ) -> Element<'a, DataFeedsMessage> {
        // Editable name
        let name_field = column![
            components::primitives::body("Name"),
            text_input("Dataset name", &self.edit_form.name)
                .on_input(DataFeedsMessage::SetName)
                .size(tokens::text::LABEL),
        ]
        .spacing(tokens::spacing::XS);

        // Info row (read-only)
        let info_row = column![
            row![
                components::primitives::small("Provider:"),
                components::primitives::small(feed.provider.display_name()),
                space::horizontal().width(12),
                components::primitives::small("Ticker:"),
                components::primitives::small(&info.ticker),
            ]
            .spacing(tokens::spacing::XS),
            row![
                components::primitives::small("Range:"),
                components::primitives::small(format!(
                    "{} - {}",
                    info.date_range.start.format("%b %d, %Y"),
                    info.date_range.end.format("%b %d, %Y")
                )),
            ]
            .spacing(tokens::spacing::XS),
            row![
                components::primitives::small("Schema:"),
                components::primitives::small(&info.schema),
                if let Some(count) = info.trade_count {
                    Element::from(
                        row![
                            space::horizontal().width(12),
                            components::primitives::small("Trades:"),
                            components::primitives::small(format_count(count)),
                        ]
                        .spacing(tokens::spacing::XS),
                    )
                } else {
                    space::horizontal().width(0).into()
                },
            ]
            .spacing(tokens::spacing::XS),
        ]
        .spacing(tokens::spacing::XXS);

        // Price line chart
        let chart_section: Element<'_, DataFeedsMessage> =
            if let Some(ref preview) = self.preview_data {
                if !preview.price_line.is_empty() {
                    let chart = PriceLineChart {
                        points: preview.price_line.clone(),
                    };
                    container(canvas::Canvas::new(chart).width(Length::Fill).height(120))
                        .style(style::modal_container)
                        .into()
                } else {
                    container(components::primitives::small("No price data available"))
                        .height(60)
                        .width(Length::Fill)
                        .align_x(Alignment::Center)
                        .align_y(Alignment::Center)
                        .style(style::modal_container)
                        .into()
                }
            } else if self.preview_loading {
                container(components::primitives::small("Loading preview..."))
                    .height(60)
                    .width(Length::Fill)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .style(style::modal_container)
                    .into()
            } else {
                container(components::primitives::small("No preview available"))
                    .height(60)
                    .width(Length::Fill)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .style(style::modal_container)
                    .into()
            };

        // Trade table
        let trade_table: Element<'_, DataFeedsMessage> = if let Some(ref preview) =
            self.preview_data
        {
            if !preview.trades.is_empty() {
                let header = row![
                    components::primitives::tiny("Time").width(Length::FillPortion(3)),
                    components::primitives::tiny("Price").width(Length::FillPortion(2)),
                    components::primitives::tiny("Size").width(Length::FillPortion(1)),
                    components::primitives::tiny("Side").width(Length::FillPortion(1)),
                ]
                .spacing(tokens::spacing::XS)
                .padding([tokens::spacing::XXS, tokens::spacing::XS]);

                let mut rows = column![header].spacing(tokens::spacing::XXXS);
                for trade in preview.trades.iter().take(50) {
                    let is_buy = trade.side == "Buy";
                    let side_text_style = move |theme: &iced::Theme| iced::widget::text::Style {
                        color: Some(if is_buy {
                            palette::success_color(theme)
                        } else {
                            palette::error_color(theme)
                        }),
                    };

                    let trade_row = row![
                        components::primitives::tiny(&trade.time).width(Length::FillPortion(3)),
                        components::primitives::tiny(&trade.price).width(Length::FillPortion(2)),
                        components::primitives::tiny(&trade.size).width(Length::FillPortion(1)),
                        components::primitives::tiny(&trade.side)
                            .width(Length::FillPortion(1))
                            .style(side_text_style),
                    ]
                    .spacing(tokens::spacing::XS)
                    .padding([tokens::spacing::XXXS, tokens::spacing::XS]);

                    rows = rows.push(trade_row);
                }

                if preview.total_trades > 50 {
                    rows = rows.push(components::primitives::tiny(format!(
                        "... and {} more trades",
                        preview.total_trades - 50
                    )));
                }

                scrollable(rows).height(120).into()
            } else {
                space::vertical().height(0).into()
            }
        } else {
            space::vertical().height(0).into()
        };

        let auto_connect_toggle = components::input::toggle_switch::toggle_switch(
            "Connect on startup",
            self.edit_form.auto_connect,
            DataFeedsMessage::SetAutoConnect,
        );

        let form_content = column![
            name_field,
            auto_connect_toggle,
            info_row,
            chart_section,
            trade_table,
        ]
        .spacing(10)
        .padding([tokens::spacing::LG, tokens::spacing::XL]);

        scrollable(form_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

fn format_count(count: usize) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}
