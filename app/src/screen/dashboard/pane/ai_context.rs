use super::types::{AiContextBubble, AiContextSummary};
use super::State;
use crate::drawing::{DrawingId, DrawingTool};
use iced::Point;

/// Format a number with comma thousands separators.
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(c);
    }
    result
}

impl State {
    /// Show the AI context bubble for a completed AiContext drawing.
    ///
    /// Extracts chart context from the drawing's time range and builds
    /// a summary for the floating input panel.
    pub(in crate::screen::dashboard::pane) fn show_ai_context_bubble(
        &mut self,
        id: DrawingId,
    ) {
        let chart = match self.content.drawing_chart() {
            Some(c) => c,
            None => return,
        };
        let drawing = match chart.drawings().get(id) {
            Some(d) if d.tool == DrawingTool::AiContext => d,
            _ => return,
        };
        if drawing.points.len() < 2 {
            return;
        }

        // Compute screen anchor (bottom-center of the drawing rectangle)
        let view_state = chart.view_state();
        let bounds_size = iced::Size::new(
            view_state.bounds.width,
            view_state.bounds.height,
        );
        let screen_p1 =
            drawing.points[0].as_screen_point(view_state, bounds_size);
        let screen_p2 =
            drawing.points[1].as_screen_point(view_state, bounds_size);
        let anchor_x = (screen_p1.x + screen_p2.x) / 2.0;
        let anchor_y = screen_p1.y.max(screen_p2.y);

        let t1 = drawing.points[0].time;
        let t2 = drawing.points[1].time;
        let (time_start, time_end) = (t1.min(t2), t1.max(t2));

        let p1 = drawing.points[0].price.to_f64();
        let p2 = drawing.points[1].price.to_f64();
        let (price_lo, price_hi) = if p1 < p2 { (p1, p2) } else { (p2, p1) };

        // Ticker + timeframe
        let ticker = self
            .ticker_info
            .map(|t| t.ticker.as_str().to_string())
            .unwrap_or_else(|| "?".into());
        let timeframe = self
            .settings
            .selected_basis
            .map(|b| format!("{}", b))
            .unwrap_or_else(|| "?".into());

        // Tick decimals for price formatting
        let tick_decimals = self
            .ticker_info
            .map(|t| {
                let ts = t.tick_size;
                if ts <= 0.0 {
                    2
                } else {
                    (-(ts as f64).log10()).ceil() as usize
                }
            })
            .unwrap_or(2);

        // Filter candles in range
        let candles: Vec<_> = self
            .chart_data
            .as_ref()
            .map(|cd| {
                cd.candles
                    .iter()
                    .filter(|c| c.time.0 >= time_start && c.time.0 <= time_end)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if candles.is_empty() {
            // No candles — delete drawing and show toast
            if let Some(chart) = self.content.drawing_chart_mut() {
                chart.drawings_mut().delete(id);
                chart.invalidate_all_drawing_caches();
            }
            self.notifications
                .push(crate::components::display::toast::Toast::warn(
                    "No candles in selected range".to_string(),
                ));
            return;
        }

        // Aggregate stats
        let candle_count = candles.len();
        let total_volume: u64 = candles.iter().map(|c| c.volume() as u64).sum();
        let net_delta: i64 = candles
            .iter()
            .map(|c| c.buy_volume.0 as i64 - c.sell_volume.0 as i64)
            .sum();

        // Format timestamps
        let fmt_ts = |ms: u64| -> String {
            chrono::DateTime::from_timestamp_millis(ms as i64)
                .map(|dt| dt.format("%m/%d %H:%M").to_string())
                .unwrap_or_else(|| "?".into())
        };

        // Pre-format OHLCV lines (cap at 50)
        let max_lines = 50;
        let candle_ohlcv_lines: Vec<String> = candles
            .iter()
            .take(max_lines)
            .map(|c| {
                let ts = chrono::DateTime::from_timestamp_millis(c.time.0 as i64)
                    .map(|dt| dt.format("%H:%M").to_string())
                    .unwrap_or_else(|| "?".into());
                let delta = c.buy_volume.0 as i64 - c.sell_volume.0 as i64;
                let sign = if delta >= 0 { "+" } else { "" };
                format!(
                    "{} O={:.prec$} H={:.prec$} L={:.prec$} C={:.prec$} \
                     V={:.0} D={sign}{}",
                    ts,
                    c.open.to_f64(),
                    c.high.to_f64(),
                    c.low.to_f64(),
                    c.close.to_f64(),
                    c.volume(),
                    delta,
                    prec = tick_decimals,
                )
            })
            .collect();

        let summary = AiContextSummary {
            ticker: ticker.clone(),
            timeframe: timeframe.clone(),
            time_start_fmt: fmt_ts(time_start),
            time_end_fmt: fmt_ts(time_end),
            price_high: format!("{:.prec$}", price_hi, prec = tick_decimals),
            price_low: format!("{:.prec$}", price_lo, prec = tick_decimals),
            candle_count,
            total_volume: format_number(total_volume),
            net_delta: {
                let sign = if net_delta >= 0 { "+" } else { "" };
                format!("{sign}{}", format_number(net_delta.unsigned_abs()))
            },
            candle_ohlcv_lines,
        };

        self.ai_context_bubble = Some(AiContextBubble {
            drawing_id: id,
            input_text: String::new(),
            range_summary: summary,
            anchor: Point::new(anchor_x, anchor_y),
        });
    }
}
