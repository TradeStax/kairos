//! Big Trades debug modal — shows detected markers in a scrollable table.

use crate::components::layout::scrollable_content::scrollable_content;
use crate::components::primitives::label::title;
use crate::screen::dashboard::pane::Message;
use crate::style;
use crate::style::tokens;

use iced::{
    Element, Length, padding,
    widget::{column, container, row, text},
};
use study::output::{StudyOutput, TradeMarker};

use tokens::debug_table::*;

fn hdr(label: &str, w: f32) -> iced::widget::Text<'_> {
    text(label).size(11).width(Length::Fixed(w))
}

fn cell(value: String, w: f32) -> iced::widget::Text<'static> {
    text(value).size(11).width(Length::Fixed(w))
}

/// Render the Big Trades debug modal content.
pub fn big_trades_debug_view<'a>(
    output: &StudyOutput,
    tick_size: f32,
) -> Element<'a, Message> {
    let markers: &[TradeMarker] = match output {
        StudyOutput::Markers(data) => &data.markers,
        _ => &[],
    };

    let decimals = count_decimals(tick_size);

    let summary = text(format!("{} markers detected", markers.len()))
        .size(13);

    let header = row![
        hdr("Time", COL_TIME),
        hdr("Side", COL_SIDE),
        hdr("Qty", COL_QTY),
        hdr("VWAP", COL_VWAP),
        hdr("Fills", COL_FILLS),
        hdr("Window", COL_WINDOW),
        hdr("Range", COL_RANGE),
    ]
    .spacing(tokens::spacing::SM)
    .padding(padding::bottom(tokens::spacing::XS));

    let mut rows = column![].spacing(2);
    for marker in markers.iter().rev() {
        let side_str = if marker.is_buy { "BUY" } else { "SELL" };
        let vwap = data::Price::from_units(marker.price).to_f64();

        let (fills, window_ms, range_str) =
            if let Some(ref debug) = marker.debug {
                let window = debug
                    .last_fill_time
                    .saturating_sub(debug.first_fill_time);
                let min_price =
                    data::Price::from_units(debug.price_min_units).to_f64();
                let max_price =
                    data::Price::from_units(debug.price_max_units).to_f64();
                (
                    format!("{}", debug.fill_count),
                    format!("{}ms", window),
                    format!(
                        "{:.prec$}-{:.prec$}",
                        min_price, max_price, prec = decimals
                    ),
                )
            } else {
                ("-".into(), "-".into(), "-".into())
            };

        let r = row![
            cell(format_timestamp(marker.time), COL_TIME),
            cell(side_str.into(), COL_SIDE),
            cell(format!("{:.0}", marker.contracts), COL_QTY),
            cell(format!("{:.prec$}", vwap, prec = decimals), COL_VWAP),
            cell(fills, COL_FILLS),
            cell(window_ms, COL_WINDOW),
            cell(range_str, COL_RANGE),
        ]
        .spacing(tokens::spacing::SM);

        rows = rows.push(r);
    }

    let content = column![
        title("Big Trades Debug"),
        summary,
        header,
        scrollable_content(rows),
    ]
    .spacing(tokens::spacing::SM)
    .width(Length::Fill);

    container(content)
        .width(620)
        .padding(28)
        .max_height(500)
        .style(style::chart_modal)
        .into()
}

fn format_timestamp(time: u64) -> String {
    use chrono::{TimeZone, Utc};
    if let Some(dt) = Utc.timestamp_millis_opt(time as i64).single() {
        dt.format("%H:%M:%S%.3f").to_string()
    } else {
        format!("{}", time)
    }
}

fn count_decimals(tick_size: f32) -> usize {
    if tick_size <= 0.0 {
        return 2;
    }
    let s = format!("{}", tick_size);
    if let Some(dot_pos) = s.find('.') {
        s.len() - dot_pos - 1
    } else {
        0
    }
}
