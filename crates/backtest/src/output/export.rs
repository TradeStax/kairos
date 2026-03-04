//! Human-readable JSON export format for backtest results.
//!
//! [`BacktestExport`] converts internal types (fixed-point prices,
//! millisecond timestamps) into a portable, shareable JSON document
//! with readable values.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::output::result::BacktestResult;
use crate::output::snapshot::ContextValue;

/// Top-level export document.
#[derive(Serialize)]
pub struct BacktestExport {
    pub export_version: u32,
    pub exported_at: String,
    pub run: ExportRunInfo,
    pub config: serde_json::Value,
    pub metrics: serde_json::Value,
    pub equity_curve: Vec<ExportEquityPoint>,
    pub daily_snapshots: Vec<ExportDailySnapshot>,
    pub trades: Vec<ExportTrade>,
    pub warnings: Vec<String>,
}

/// High-level run metadata.
#[derive(Serialize)]
pub struct ExportRunInfo {
    pub id: String,
    pub strategy_name: String,
    pub run_started_at: String,
    pub run_duration_ms: u64,
    pub total_data_trades: usize,
    pub sessions_processed: usize,
    pub sessions_skipped: usize,
}

/// A single equity curve sample.
#[derive(Serialize)]
pub struct ExportEquityPoint {
    pub timestamp: String,
    pub realized_equity_usd: f64,
    pub unrealized_pnl_usd: f64,
    pub total_equity_usd: f64,
}

/// Daily equity snapshot.
#[derive(Serialize)]
pub struct ExportDailySnapshot {
    pub date: String,
    pub equity: f64,
    pub realized_pnl: f64,
    pub positions_count: usize,
}

/// A single trade with all details in human-readable form.
#[derive(Serialize)]
pub struct ExportTrade {
    pub index: usize,
    pub entry_time: String,
    pub exit_time: String,
    pub side: String,
    pub quantity: f64,
    pub entry_price: f64,
    pub exit_price: f64,
    pub initial_stop_loss: f64,
    pub initial_take_profit: Option<f64>,
    pub pnl_ticks: i64,
    pub pnl_gross_usd: f64,
    pub commission_usd: f64,
    pub pnl_net_usd: f64,
    pub rr_ratio: f64,
    pub mae_ticks: i64,
    pub mfe_ticks: i64,
    pub exit_reason: String,
    pub label: Option<String>,
    pub instrument: Option<String>,
    pub duration_ms: Option<u64>,
    pub duration_human: Option<String>,
    pub strategy_context: BTreeMap<String, serde_json::Value>,
}

impl BacktestExport {
    /// Convert a [`BacktestResult`] into a portable export document.
    #[must_use]
    pub fn from_result(result: &BacktestResult) -> Self {
        let now = now_iso8601();

        let run = ExportRunInfo {
            id: result.id.to_string(),
            strategy_name: result.strategy_name.clone(),
            run_started_at: ms_to_iso8601(result.run_started_at_ms),
            run_duration_ms: result.run_duration_ms,
            total_data_trades: result.total_data_trades,
            sessions_processed: result.sessions_processed,
            sessions_skipped: result.sessions_skipped,
        };

        // Serialize config and metrics as generic JSON values so
        // the export captures every field without a parallel struct.
        let config = serde_json::to_value(&result.config).unwrap_or_default();
        let metrics = serde_json::to_value(&result.metrics).unwrap_or_default();

        let equity_curve = result
            .equity_curve
            .points
            .iter()
            .map(|p| ExportEquityPoint {
                timestamp: ms_to_iso8601(p.timestamp.0),
                realized_equity_usd: p.realized_equity_usd,
                unrealized_pnl_usd: p.unrealized_pnl_usd,
                total_equity_usd: p.total_equity_usd,
            })
            .collect();

        let daily_snapshots = result
            .daily_snapshots
            .iter()
            .map(|s| ExportDailySnapshot {
                date: ms_to_iso8601(s.date_ms),
                equity: s.equity,
                realized_pnl: s.realized_pnl,
                positions_count: s.positions_count,
            })
            .collect();

        let trades = result.trades.iter().map(export_trade).collect();

        Self {
            export_version: 1,
            exported_at: now,
            run,
            config,
            metrics,
            equity_curve,
            daily_snapshots,
            trades,
            warnings: result.warnings.clone(),
        }
    }
}

/// Convert a single [`TradeRecord`] to an [`ExportTrade`].
fn export_trade(t: &crate::output::trade_record::TradeRecord) -> ExportTrade {
    let side = if t.side == kairos_data::Side::Buy {
        "Long"
    } else {
        "Short"
    };

    let strategy_context = t
        .snapshot
        .as_ref()
        .map(|snap| {
            snap.context
                .iter()
                .map(|(k, v)| (k.clone(), context_value_to_json(v)))
                .collect()
        })
        .unwrap_or_default();

    ExportTrade {
        index: t.index,
        entry_time: ms_to_iso8601(t.entry_time.0),
        exit_time: ms_to_iso8601(t.exit_time.0),
        side: side.to_string(),
        quantity: t.quantity,
        entry_price: t.entry_price.to_f64(),
        exit_price: t.exit_price.to_f64(),
        initial_stop_loss: t.initial_stop_loss.to_f64(),
        initial_take_profit: t.initial_take_profit.map(|p| p.to_f64()),
        pnl_ticks: t.pnl_ticks,
        pnl_gross_usd: t.pnl_gross_usd,
        commission_usd: t.commission_usd,
        pnl_net_usd: t.pnl_net_usd,
        rr_ratio: t.rr_ratio,
        mae_ticks: t.mae_ticks,
        mfe_ticks: t.mfe_ticks,
        exit_reason: t.exit_reason.to_string(),
        label: t.label.clone(),
        instrument: t.instrument.map(|i| i.as_str().to_string()),
        duration_ms: t.duration_ms,
        duration_human: t.duration_ms.map(format_duration),
        strategy_context,
    }
}

/// Convert a [`ContextValue`] to a JSON value with readable
/// formatting.
fn context_value_to_json(cv: &ContextValue) -> serde_json::Value {
    match cv {
        ContextValue::Price(p) => serde_json::json!(p.to_f64()),
        ContextValue::Float(f) => serde_json::json!(f),
        ContextValue::Integer(i) => serde_json::json!(i),
        ContextValue::Bool(b) => serde_json::json!(b),
        ContextValue::Text(s) => serde_json::json!(s),
        ContextValue::Timestamp(ts) => {
            serde_json::json!(ms_to_iso8601(ts.0))
        }
    }
}

/// Format milliseconds as ISO 8601 UTC string.
fn ms_to_iso8601(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    let nanos = ((ms % 1000) * 1_000_000) as u32;
    let dt = chrono::DateTime::from_timestamp(secs, nanos);
    match dt {
        Some(dt) => dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        None => format!("{}ms", ms),
    }
}

/// Current time as ISO 8601 UTC string.
fn now_iso8601() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

/// Format a duration in milliseconds as a human-readable string.
///
/// Examples: `"5m 23s"`, `"1h 2m 30s"`, `"45s"`, `"0s"`.
fn format_duration(ms: u64) -> String {
    let total_secs = ms / 1000;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ms_to_iso8601() {
        // 2025-01-15 09:30:00 UTC
        let ms = 1_736_933_400_000_u64;
        let iso = ms_to_iso8601(ms);
        assert!(iso.starts_with("2025-01-15T09:30:00"));
        assert!(iso.ends_with('Z'));
    }

    #[test]
    fn test_format_duration_seconds() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(45_000), "45s");
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(323_000), "5m 23s");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(3_750_000), "1h 2m 30s");
    }

    #[test]
    fn test_context_value_to_json_price() {
        let cv = ContextValue::Price(kairos_data::Price::from_f64(5025.75));
        let json = context_value_to_json(&cv);
        let val = json.as_f64().unwrap();
        assert!((val - 5025.75).abs() < 1e-6);
    }

    #[test]
    fn test_context_value_to_json_timestamp() {
        let cv = ContextValue::Timestamp(kairos_data::Timestamp(1_736_933_400_000));
        let json = context_value_to_json(&cv);
        let s = json.as_str().unwrap();
        assert!(s.starts_with("2025-01-15T09:30:00"));
    }
}
