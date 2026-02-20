//! Bridge: converts collected JS PlotCommands into StudyOutput variants.

use crate::runtime::plot::PlotCommand;
use data::Candle;
use std::collections::HashMap;
use study::output::{
    BarPoint, BarSeries, HistogramBar, LineSeries, PriceLevel, StudyOutput, TradeMarker,
};

/// Convert collected plot commands from a script execution into StudyOutput variants.
///
/// Returns a Vec because a single script may produce multiple output types
/// (e.g., MACD produces Lines + Histogram).
pub fn convert_plot_commands(
    commands: &[PlotCommand],
    candles: &[Candle],
) -> Vec<StudyOutput> {
    let mut lines_by_id: HashMap<usize, LineSeries> = HashMap::new();
    let mut bar_series_list: Vec<BarSeries> = Vec::new();
    let mut histogram_bars: Vec<HistogramBar> = Vec::new();
    let mut markers: Vec<TradeMarker> = Vec::new();
    let mut levels: Vec<PriceLevel> = Vec::new();
    let mut fills: Vec<(usize, usize, f32)> = Vec::new();

    for cmd in commands {
        match cmd {
            PlotCommand::Line {
                id,
                name,
                points,
                color,
                width,
                style,
            } => {
                let series = build_line_series(name, points, *color, *width, *style, candles);
                lines_by_id.insert(*id, series);
            }
            PlotCommand::Bar { name, points } => {
                let series = build_bar_series(name, points, candles);
                bar_series_list.push(series);
            }
            PlotCommand::Histogram { name, points } => {
                let bars = build_histogram(name, points, candles);
                histogram_bars.extend(bars);
            }
            PlotCommand::Marker {
                time,
                price,
                size,
                color,
                label,
                is_buy,
            } => {
                markers.push(TradeMarker {
                    time: *time as u64,
                    price: (*price * 100_000_000.0).round() as i64,
                    contracts: *size,
                    is_buy: *is_buy,
                    color: *color,
                    label: label.clone(),
                    debug: None,
                });
            }
            PlotCommand::HLine {
                price,
                name,
                color,
                style,
                opacity,
            } => {
                levels.push(PriceLevel {
                    price: *price,
                    label: name.clone(),
                    color: *color,
                    style: *style,
                    opacity: *opacity,
                    show_label: true,
                    fill_above: None,
                    fill_below: None,
                });
            }
            PlotCommand::Fill {
                plot_id_a,
                plot_id_b,
                opacity,
                ..
            } => {
                fills.push((*plot_id_a, *plot_id_b, *opacity));
            }
        }
    }

    let mut outputs = Vec::new();

    // Process fills: merge two line plots into a Band
    for (id_a, id_b, opacity) in &fills {
        let upper = lines_by_id.remove(id_a);
        let lower = lines_by_id.remove(id_b);
        if let (Some(upper), Some(lower)) = (upper, lower) {
            outputs.push(StudyOutput::Band {
                upper,
                middle: None,
                lower,
                fill_opacity: *opacity,
            });
        }
    }

    // Remaining lines
    if !lines_by_id.is_empty() {
        let mut lines: Vec<(usize, LineSeries)> = lines_by_id.into_iter().collect();
        lines.sort_by_key(|(id, _)| *id);
        let series: Vec<LineSeries> = lines.into_iter().map(|(_, s)| s).collect();
        outputs.push(StudyOutput::Lines(series));
    }

    if !bar_series_list.is_empty() {
        outputs.push(StudyOutput::Bars(bar_series_list));
    }

    if !histogram_bars.is_empty() {
        outputs.push(StudyOutput::Histogram(histogram_bars));
    }

    if !markers.is_empty() {
        outputs.push(StudyOutput::Markers(markers));
    }

    if !levels.is_empty() {
        outputs.push(StudyOutput::Levels(levels));
    }

    outputs
}

fn build_line_series(
    name: &str,
    points: &[f64],
    color: data::SerializableColor,
    width: f32,
    style: study::config::LineStyleValue,
    candles: &[Candle],
) -> LineSeries {
    let pts: Vec<(u64, f32)> = points
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.is_nan())
        .filter_map(|(i, v)| {
            candles.get(i).map(|c| (c.time.0, *v as f32))
        })
        .collect();

    LineSeries {
        label: name.to_string(),
        color,
        width,
        style,
        points: pts,
    }
}

fn build_bar_series(
    name: &str,
    points: &[(f64, data::SerializableColor)],
    candles: &[Candle],
) -> BarSeries {
    let pts: Vec<BarPoint> = points
        .iter()
        .enumerate()
        .filter(|(_, (v, _))| !v.is_nan())
        .filter_map(|(i, (v, color))| {
            candles.get(i).map(|c| BarPoint {
                x: c.time.0,
                value: *v as f32,
                color: *color,
                overlay: None,
            })
        })
        .collect();

    BarSeries {
        label: name.to_string(),
        points: pts,
    }
}

fn build_histogram(
    _name: &str,
    points: &[(f64, data::SerializableColor)],
    candles: &[Candle],
) -> Vec<HistogramBar> {
    points
        .iter()
        .enumerate()
        .filter(|(_, (v, _))| !v.is_nan())
        .filter_map(|(i, (v, color))| {
            candles.get(i).map(|c| HistogramBar {
                x: c.time.0,
                value: *v as f32,
                color: *color,
            })
        })
        .collect()
}
