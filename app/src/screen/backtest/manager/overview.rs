//! Overview tab for the backtest management modal.
//!
//! Shows KPI metrics cards, equity curve canvas, drawdown chart,
//! monthly returns grid, and P&L histogram.

use super::charts::{DrawdownChart, EquityChart, HistogramChart, ReturnsGrid};
use super::computed::ComputedAnalytics;
use super::{BacktestManager, ManagerMessage};
use crate::app::backtest_history::{BacktestHistory, BacktestStatus};
use crate::config::UserTimezone;
use crate::style::{palette, tokens};
use iced::widget::{canvas, center, column, container, row, rule, scrollable, text};
use iced::{Background, Color, Element, Length};
use std::sync::Arc;

/// Render the Overview tab content.
pub fn view_overview<'a>(
    manager: &'a BacktestManager,
    history: &'a BacktestHistory,
    timezone: UserTimezone,
) -> Element<'a, ManagerMessage> {
    // Resolve selected backtest entry
    let entry = manager.selected_id.and_then(|id| history.get(id));

    let Some(entry) = entry else {
        return empty_state("Select a backtest to view results");
    };

    if entry.status != BacktestStatus::Completed {
        return empty_state("Backtest is not yet completed");
    }

    let Some(ref result) = entry.result else {
        return empty_state("No result data available");
    };

    let metrics = &result.metrics;
    let analytics = manager.analytics.as_ref();

    // ── Section 1: KPI Grid ────────────────────────────────────────
    let pnl_value = format_usd(metrics.net_pnl_usd);
    let pnl_positive = metrics.net_pnl_usd >= 0.0;

    let kpi_row_1 = row![
        metric_card_colored("Net P&L", pnl_value, Some(pnl_positive),),
        metric_card("Trades", metrics.total_trades.to_string(),),
        metric_card("Win Rate", format!("{:.1}%", metrics.win_rate * 100.0),),
        metric_card("Profit Factor", format_ratio(metrics.profit_factor),),
    ]
    .spacing(tokens::spacing::MD);

    let kpi_row_2 = row![
        metric_card("Sharpe", format_ratio(metrics.sharpe_ratio),),
        metric_card("Max DD", format!("{:.1}%", metrics.max_drawdown_pct),),
        metric_card("Sortino", format_ratio(metrics.sortino_ratio),),
        metric_card("Return %", format!("{:.2}%", metrics.total_return_pct),),
    ]
    .spacing(tokens::spacing::MD);

    let kpi_section = column![kpi_row_1, kpi_row_2].spacing(tokens::spacing::MD);

    // ── Section 2: Equity Curve ────────────────────────────────────
    let equity_chart = canvas(EquityChart {
        result: Arc::clone(result),
        selected_trade_idx: manager.selected_trade,
        cache: &manager.equity_cache,
        timezone,
    })
    .width(Length::Fill)
    .height(Length::Fixed(220.0));

    // ── Section 3: Drawdown Chart ──────────────────────────────────
    let drawdown_chart = canvas(DrawdownChart {
        result: Arc::clone(result),
        cache: &manager.drawdown_cache,
        timezone,
    })
    .width(Length::Fill)
    .height(Length::Fixed(120.0));

    // ── Section 4: Monthly Returns + P&L Histogram ─────────────────
    let bottom_row = build_bottom_row(manager, analytics);

    // ── Assemble ───────────────────────────────────────────────────
    let content = column![
        kpi_section,
        rule::horizontal(1),
        equity_chart,
        rule::horizontal(1),
        drawdown_chart,
        rule::horizontal(1),
        bottom_row,
    ]
    .spacing(tokens::spacing::LG)
    .padding(tokens::spacing::MD);

    scrollable(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// ── Bottom Row ──────────────────────────────────────────────────────

fn build_bottom_row<'a>(
    manager: &'a BacktestManager,
    analytics: Option<&'a ComputedAnalytics>,
) -> Element<'a, ManagerMessage> {
    let monthly_col = {
        let header = text("Monthly Returns").size(tokens::text::LABEL);

        let grid: Element<'a, ManagerMessage> = match analytics {
            Some(a) => canvas(ReturnsGrid {
                monthly_returns: a.monthly_returns.clone(),
                cache: &manager.returns_cache,
            })
            .width(Length::Fill)
            .height(Length::Fixed(150.0))
            .into(),
            None => empty_chart_placeholder(),
        };

        column![header, grid]
            .spacing(tokens::spacing::SM)
            .width(Length::FillPortion(1))
    };

    let histogram_col = {
        let header = text("P&L Distribution").size(tokens::text::LABEL);

        let chart: Element<'a, ManagerMessage> = match analytics {
            Some(a) => canvas(HistogramChart {
                bins: a.pnl_histogram.clone(),
                cache: &manager.histogram_cache,
            })
            .width(Length::Fill)
            .height(Length::Fixed(150.0))
            .into(),
            None => empty_chart_placeholder(),
        };

        column![header, chart]
            .spacing(tokens::spacing::SM)
            .width(Length::FillPortion(1))
    };

    row![monthly_col, histogram_col]
        .spacing(tokens::spacing::LG)
        .into()
}

// ── Metric Cards ────────────────────────────────────────────────────

fn metric_card<'a>(label: &'static str, value: String) -> Element<'a, ManagerMessage> {
    container(
        column![
            text(label)
                .size(tokens::text::SMALL)
                .style(palette::neutral_text),
            text(value).size(tokens::text::LABEL),
        ]
        .spacing(tokens::spacing::XXS)
        .padding([tokens::spacing::XS, tokens::spacing::MD]),
    )
    .style(card_background)
    .width(Length::Fill)
    .into()
}

fn metric_card_colored<'a>(
    label: &'static str,
    value: String,
    positive: Option<bool>,
) -> Element<'a, ManagerMessage> {
    let value_text = match positive {
        Some(true) => text(value)
            .size(tokens::text::LABEL)
            .style(palette::success_text),
        Some(false) => text(value)
            .size(tokens::text::LABEL)
            .style(palette::error_text),
        None => text(value).size(tokens::text::LABEL),
    };

    container(
        column![
            text(label)
                .size(tokens::text::SMALL)
                .style(palette::neutral_text),
            value_text,
        ]
        .spacing(tokens::spacing::XXS)
        .padding([tokens::spacing::XS, tokens::spacing::MD]),
    )
    .style(card_background)
    .width(Length::Fill)
    .into()
}

// ── Shared Styles ───────────────────────────────────────────────────

fn card_background(theme: &iced::Theme) -> container::Style {
    let p = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(Color {
            a: 0.04,
            ..p.background.weak.color
        })),
        ..Default::default()
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

fn empty_state<'a>(msg: &'static str) -> Element<'a, ManagerMessage> {
    center(
        text(msg)
            .size(tokens::text::BODY)
            .style(palette::neutral_text),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn empty_chart_placeholder<'a>() -> Element<'a, ManagerMessage> {
    container(
        text("No data")
            .size(tokens::text::SMALL)
            .style(palette::neutral_text),
    )
    .width(Length::Fill)
    .height(Length::Fixed(150.0))
    .center_x(Length::Fill)
    .center_y(Length::Fixed(150.0))
    .into()
}

fn format_usd(val: f64) -> String {
    let abs = val.abs();
    let sign = if val < 0.0 { "-" } else { "" };
    if abs >= 1_000_000.0 {
        format!("{}${:.1}M", sign, abs / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{}${:.0}", sign, abs)
    } else {
        format!("{}${:.2}", sign, abs)
    }
}

fn format_ratio(val: f64) -> String {
    if val.is_infinite() {
        "Inf".to_string()
    } else if val.is_nan() {
        "N/A".to_string()
    } else {
        format!("{:.2}", val)
    }
}
