//! Chart data formatting for AI context.

use crate::context::market;
use crate::tools::ToolContext;

/// Build a compact chart overview from the tool context.
///
/// Includes: instrument info, recent candles, active studies,
/// and notable events.
pub fn format_chart_overview(ctx: &ToolContext) -> String {
    let mut out = String::with_capacity(2048);

    // Header
    out.push_str("## Chart Overview\n");
    out.push_str(&format!(
        "- Instrument: {}\n",
        ctx.ticker_info.ticker.as_str(),
    ));
    out.push_str(&format!(
        "- Basis: {}\n",
        ctx.chart_config.basis,
    ));
    out.push_str(&format!(
        "- Tick size: {}\n",
        ctx.ticker_info.tick_size,
    ));
    out.push_str(&format!(
        "- Candles loaded: {}\n",
        ctx.candles.len(),
    ));

    if let Some(trades) = &ctx.trades {
        out.push_str(&format!(
            "- Trades loaded: {}\n",
            trades.len(),
        ));
    }
    if let Some(depth) = &ctx.depth_snapshots {
        out.push_str(&format!(
            "- Depth snapshots: {}\n",
            depth.len(),
        ));
    }

    // Recent candles (last 20)
    if !ctx.candles.is_empty() {
        let tick_size = ctx.ticker_info.tick_size;
        out.push_str("\n### Recent Candles (newest last)\n");
        out.push_str(
            &format_candles_compact(
                &ctx.candles, tick_size,
            ),
        );
    }

    // Study summaries
    if !ctx.study_outputs.is_empty() {
        out.push_str("\n### Active Studies\n");
        out.push_str(&format_study_summary(
            &ctx.study_outputs,
        ));
    }

    // Notable events
    if !ctx.candles.is_empty() {
        let events = detect_notable_events(&ctx.candles);
        if !events.is_empty() {
            out.push_str("\n### Notable Events\n");
            for event in &events {
                out.push_str(&format!("- {event}\n"));
            }
        }
    }

    // Drawing count
    if !ctx.drawings.is_empty() {
        out.push_str(&format!(
            "\n### Drawings: {} active\n",
            ctx.drawings.len(),
        ));
    }

    out
}

/// Format candles as a compact table (last N candles).
pub fn format_candles_compact(
    candles: &[data::Candle],
    tick_size: f32,
) -> String {
    let tick_price = data::Price::from_f32(tick_size);
    let n = candles.len().min(20);
    let recent = &candles[candles.len() - n..];

    let mut out = String::with_capacity(n * 80);
    out.push_str(
        "Time | O | H | L | C | Vol | Delta\n",
    );
    out.push_str(
        "--- | --- | --- | --- | --- | --- | ---\n",
    );

    for c in recent {
        let ts = market::format_timestamp(
            c.time.to_millis(),
        );
        let o =
            market::format_price(c.open, tick_price);
        let h =
            market::format_price(c.high, tick_price);
        let l =
            market::format_price(c.low, tick_price);
        let cl =
            market::format_price(c.close, tick_price);
        let vol = market::format_volume(
            c.total_volume().value() as u64,
        );
        let delta = c.volume_delta() as i64;

        out.push_str(&format!(
            "{ts} | {o} | {h} | {l} | {cl} | {vol} | \
             {delta}\n",
        ));
    }
    out
}

/// Summarize active study outputs.
pub fn format_study_summary(
    studies: &[(String, study::StudyOutput)],
) -> String {
    let mut out = String::new();
    for (name, output) in studies {
        let desc = match output {
            study::StudyOutput::Lines(lines) => {
                if let Some(last) = lines
                    .first()
                    .and_then(|l| l.values.last())
                {
                    format!(
                        "{name}: latest = {:.4}",
                        last.value
                    )
                } else {
                    format!("{name}: (no data)")
                }
            }
            study::StudyOutput::Band {
                upper, lower, ..
            } => {
                let u = upper
                    .values
                    .last()
                    .map(|v| v.value)
                    .unwrap_or(0.0);
                let l = lower
                    .values
                    .last()
                    .map(|v| v.value)
                    .unwrap_or(0.0);
                format!(
                    "{name}: upper={u:.4}, lower={l:.4}"
                )
            }
            study::StudyOutput::Bars(series) => {
                let total: f64 = series
                    .iter()
                    .flat_map(|s| s.values.last())
                    .map(|b| b.value)
                    .sum();
                format!(
                    "{name}: latest bar = {total:.0}"
                )
            }
            study::StudyOutput::Histogram(bars) => {
                let last = bars
                    .last()
                    .map(|b| b.value)
                    .unwrap_or(0.0);
                format!(
                    "{name}: latest = {last:.4}"
                )
            }
            study::StudyOutput::Levels(levels) => {
                format!(
                    "{name}: {} levels",
                    levels.len()
                )
            }
            study::StudyOutput::Profile(profiles, _) => {
                format!(
                    "{name}: {} profile(s)",
                    profiles.len()
                )
            }
            study::StudyOutput::Footprint(fp) => {
                format!(
                    "{name}: {} footprint candles",
                    fp.candles.len()
                )
            }
            study::StudyOutput::Markers(m) => {
                format!(
                    "{name}: {} markers",
                    m.markers.len()
                )
            }
            study::StudyOutput::Composite(parts) => {
                format!(
                    "{name}: composite ({} parts)",
                    parts.len()
                )
            }
            study::StudyOutput::Empty => {
                format!("{name}: (empty)")
            }
        };
        out.push_str(&format!("- {desc}\n"));
    }
    out
}

/// Detect notable events in the candle data.
///
/// Looks for: large volume spikes, big range candles, strong
/// delta, etc.
pub fn detect_notable_events(
    candles: &[data::Candle],
) -> Vec<String> {
    if candles.is_empty() {
        return Vec::new();
    }

    let mut events = Vec::new();

    // Compute average volume and range for reference
    let avg_vol: f64 = candles
        .iter()
        .map(|c| c.total_volume().value())
        .sum::<f64>()
        / candles.len() as f64;

    let avg_range: f64 = candles
        .iter()
        .map(|c| c.range().to_f64())
        .sum::<f64>()
        / candles.len() as f64;

    // Scan last 50 candles for notable events
    let scan_start =
        candles.len().saturating_sub(50);
    for c in &candles[scan_start..] {
        let vol = c.total_volume().value();
        let range = c.range().to_f64();
        let delta = c.volume_delta();

        // Volume spike (> 3x average)
        if avg_vol > 0.0 && vol > avg_vol * 3.0 {
            events.push(format!(
                "Volume spike at {} \
                 ({:.0} vs avg {:.0})",
                market::format_timestamp(
                    c.time.to_millis()
                ),
                vol,
                avg_vol,
            ));
        }

        // Large range candle (> 3x average)
        if avg_range > 0.0 && range > avg_range * 3.0 {
            events.push(format!(
                "Large range candle at {} \
                 ({:.4} vs avg {:.4})",
                market::format_timestamp(
                    c.time.to_millis()
                ),
                range,
                avg_range,
            ));
        }

        // Strong delta (> 2x volume in one direction)
        if vol > 0.0 && delta.abs() > vol * 0.7 {
            let direction = if delta > 0.0 {
                "buying"
            } else {
                "selling"
            };
            events.push(format!(
                "Strong {} at {} \
                 (delta {:.0}, vol {:.0})",
                direction,
                market::format_timestamp(
                    c.time.to_millis()
                ),
                delta,
                vol,
            ));
        }
    }

    // Limit to most recent 10 events
    if events.len() > 10 {
        events.drain(..events.len() - 10);
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::{
        Candle, Price, Timestamp, Volume,
    };

    fn make_candle(
        time_ms: u64,
        open: f32,
        high: f32,
        low: f32,
        close: f32,
        buy_vol: f64,
        sell_vol: f64,
    ) -> Candle {
        Candle {
            time: Timestamp::from_millis(time_ms),
            open: Price::from_f32(open),
            high: Price::from_f32(high),
            low: Price::from_f32(low),
            close: Price::from_f32(close),
            buy_volume: Volume(buy_vol),
            sell_volume: Volume(sell_vol),
        }
    }

    #[test]
    fn test_detect_notable_events_volume_spike() {
        let mut candles: Vec<Candle> = (0..20)
            .map(|i| {
                make_candle(
                    i * 60000,
                    100.0,
                    101.0,
                    99.0,
                    100.5,
                    50.0,
                    50.0,
                )
            })
            .collect();
        // Add a volume spike
        candles.push(make_candle(
            20 * 60000,
            100.0,
            101.0,
            99.0,
            100.5,
            500.0,
            500.0,
        ));

        let events = detect_notable_events(&candles);
        assert!(
            events.iter().any(|e| e.contains("Volume spike")),
            "Expected volume spike event, got: {:?}",
            events
        );
    }
}
