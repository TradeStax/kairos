//! Analytics tab for the backtest management modal.
//!
//! Five sections: prop firm simulation, Monte Carlo, risk & position
//! sizing, P&L distribution + MAE/MFE scatter, and P&L by hour.

use super::charts::{
    BarChart, HistogramChart, MonteCarloChart, PropFirmEquityChart, PropFirmMonteCarloChart,
    ScatterChart,
};
use super::computed::{ComputedAnalytics, PropFirmResult, PropFirmStatus};
use super::{BacktestManager, ManagerMessage, PropFirmDetailView};
use crate::app::backtest_history::BacktestHistory;
use crate::components::primitives::icon_button::icon_button;
use crate::components::primitives::icons::{AZERET_MONO, Icon};
use crate::config::UserTimezone;
use crate::style::{palette, tokens};
use iced::widget::{button, canvas, column, container, row, rule, scrollable, text};
use iced::{Background, Color, Element, Length};

// ── Public Entry Point ──────────────────────────────────────────────

/// Render the Analytics tab content.
pub fn view<'a>(
    manager: &'a BacktestManager,
    history: &'a BacktestHistory,
    _timezone: UserTimezone,
) -> Element<'a, ManagerMessage> {
    let entry = manager.selected_id.and_then(|id| history.get(id));

    let Some(entry) = entry else {
        return empty_state("Select a completed backtest");
    };

    let Some(ref result) = entry.result else {
        return empty_state("Select a completed backtest");
    };

    let Some(ref analytics) = manager.analytics else {
        return empty_state("Select a completed backtest");
    };

    // ── Full-page prop firm detail view ─────────────────────────
    if let Some(ref detail) = manager.prop_firm_detail
        && let Some(pf) = analytics.prop_firm_results.get(detail.account_index)
    {
        return prop_firm_detail_page(pf, detail);
    }

    // ── Section 1: Prop Firm Simulation ────────────────────────
    let prop_firm_section = {
        let header = section_header("PROP FIRM SIMULATION");

        let mut cards_row = row![].spacing(tokens::spacing::SM);
        for (i, pf) in analytics.prop_firm_results.iter().enumerate() {
            cards_row = cards_row.push(prop_firm_card(pf, i));
        }

        column![header, cards_row].spacing(tokens::spacing::SM)
    };

    // ── Section 2: Monte Carlo Simulation ──────────────────────
    let monte_carlo_section = {
        let header = section_header("MONTE CARLO SIMULATION (100 paths)");

        let original_equity: Vec<f64> = result
            .equity_curve
            .points
            .iter()
            .map(|p| p.total_equity_usd)
            .collect();

        let chart = canvas(MonteCarloChart {
            paths: analytics.monte_carlo_paths.clone(),
            p5: analytics.monte_carlo_p5.clone(),
            p50: analytics.monte_carlo_p50.clone(),
            p95: analytics.monte_carlo_p95.clone(),
            original_equity,
            cache: &manager.monte_carlo_cache,
        })
        .width(Length::Fill)
        .height(Length::Fixed(250.0));

        column![header, chart].spacing(tokens::spacing::SM)
    };

    // ── Section 3: Risk & Position Sizing ──────────────────────
    let risk_row = row![
        risk_expectancy_card(analytics),
        position_sizing_card(analytics),
    ]
    .spacing(tokens::spacing::MD);

    // ── Section 4: P&L Distribution + MAE/MFE Scatter ──────────
    let histogram_col = {
        let header = section_header("P&L DISTRIBUTION");

        let chart = canvas(HistogramChart {
            bins: analytics.pnl_histogram.clone(),
            cache: &manager.histogram_cache,
        })
        .width(Length::Fill)
        .height(Length::Fixed(200.0));

        column![header, chart]
            .spacing(tokens::spacing::SM)
            .width(Length::FillPortion(1))
    };

    let scatter_col = {
        let header = section_header("MAE vs MFE");

        let chart = canvas(ScatterChart {
            points: analytics.mae_mfe_scatter.clone(),
            cache: &manager.scatter_cache,
        })
        .width(Length::Fill)
        .height(Length::Fixed(200.0));

        column![header, chart]
            .spacing(tokens::spacing::SM)
            .width(Length::FillPortion(1))
    };

    let charts_row = row![histogram_col, scatter_col].spacing(tokens::spacing::MD);

    // ── Section 5: Performance by Hour ─────────────────────────
    let hour_section = {
        let header = section_header("PERFORMANCE BY HOUR");

        let bars: Vec<(String, f64)> = analytics
            .pnl_by_hour
            .iter()
            .map(|(h, pnl)| (format!("{:02}", h), *pnl))
            .collect();

        let chart = canvas(BarChart {
            bars,
            cache: &manager.bar_chart_cache,
        })
        .width(Length::Fill)
        .height(Length::Fixed(180.0));

        column![header, chart].spacing(tokens::spacing::SM)
    };

    // ── Assemble ─────────────────────────────────────────────────
    let content = column![
        prop_firm_section,
        rule::horizontal(1),
        monte_carlo_section,
        rule::horizontal(1),
        risk_row,
        rule::horizontal(1),
        charts_row,
        rule::horizontal(1),
        hour_section,
    ]
    .spacing(tokens::spacing::LG)
    .padding(tokens::spacing::MD);

    scrollable(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// ── Section Header ─────────────────────────────────────────────────

fn section_header(title: &str) -> Element<'static, ManagerMessage> {
    text(title.to_string())
        .size(tokens::text::LABEL)
        .style(palette::neutral_text)
        .into()
}

// ── Prop Firm Card ─────────────────────────────────────────────────

fn prop_firm_card(pf: &PropFirmResult, idx: usize) -> Element<'static, ManagerMessage> {
    let name = text(pf.account.name.to_string()).size(tokens::text::LABEL);

    let (badge_text, badge_style): (&str, fn(&iced::Theme) -> iced::widget::text::Style) =
        match pf.status {
            PropFirmStatus::Passed => ("PASSED", palette::success_text),
            PropFirmStatus::Failed => ("FAILED", palette::error_text),
            PropFirmStatus::Active => ("ACTIVE", palette::info_text),
        };
    let badge = text(badge_text.to_string())
        .size(tokens::text::SMALL)
        .style(badge_style);

    // MC pass rate badge
    let pass_rate_pct = pf.monte_carlo.pass_rate * 100.0;
    let pr_style: fn(&iced::Theme) -> iced::widget::text::Style = if pass_rate_pct >= 50.0 {
        palette::success_text
    } else {
        palette::error_text
    };
    let pass_rate_label = text(format!("MC: {:.0}%", pass_rate_pct))
        .size(tokens::text::SMALL)
        .style(pr_style);

    let pnl_style: fn(&iced::Theme) -> iced::widget::text::Style = if pf.final_pnl >= 0.0 {
        palette::success_text
    } else {
        palette::error_text
    };
    let pnl_row = kv_row("P&L", &format_pnl(pf.final_pnl), Some(pnl_style));

    let dd_pct = if pf.account.size > 0.0 {
        pf.worst_drawdown / pf.account.size * 100.0
    } else {
        0.0
    };
    let dd_style: fn(&iced::Theme) -> iced::widget::text::Style = if pf.hit_drawdown_limit {
        palette::error_text
    } else {
        palette::neutral_text
    };
    let dd_row = kv_row("Worst DD", &format!("{:.1}%", dd_pct), Some(dd_style));

    let progress_pct = pf.progress_pct;
    let progress_bar = progress_bar_widget(progress_pct);

    let badge_row = row![badge, pass_rate_label].spacing(tokens::spacing::SM);

    let card_content =
        column![name, badge_row, pnl_row, dd_row, progress_bar].spacing(tokens::spacing::XS);

    let style_fn = move |theme: &iced::Theme, status: button::Status| {
        let p = theme.extended_palette();
        let bg_alpha = match status {
            button::Status::Hovered => 0.08,
            button::Status::Pressed => 0.10,
            _ => 0.04,
        };
        button::Style {
            background: Some(Background::Color(Color {
                a: bg_alpha,
                ..p.background.weak.color
            })),
            text_color: p.background.base.text,
            border: iced::Border {
                radius: tokens::radius::MD.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    };

    button(
        container(card_content)
            .padding(tokens::spacing::MD)
            .width(Length::Fill),
    )
    .width(Length::FillPortion(1))
    .padding(0)
    .on_press(ManagerMessage::SelectPropFirm(idx))
    .style(style_fn)
    .into()
}

// ── Progress Bar Widget ───────────────────────────────────────────

fn progress_bar_widget(pct: f64) -> Element<'static, ManagerMessage> {
    let clamped = pct.clamp(0.0, 100.0) as f32 / 100.0;
    let fill_color = if pct >= 100.0 {
        tokens::backtest::PROP_FIRM_PROGRESS_COMPLETE
    } else {
        tokens::backtest::PROP_FIRM_PROGRESS_FILL
    };

    // Track
    let track = container(
        container(text("").size(1.0))
            .width(Length::FillPortion((clamped * 1000.0) as u16))
            .height(Length::Fixed(3.0))
            .style(move |_theme: &iced::Theme| container::Style {
                background: Some(Background::Color(fill_color)),
                ..Default::default()
            }),
    )
    .width(Length::Fill)
    .height(Length::Fixed(3.0))
    .style(|_theme: &iced::Theme| container::Style {
        background: Some(Background::Color(
            tokens::backtest::PROP_FIRM_PROGRESS_TRACK,
        )),
        border: iced::Border {
            radius: 1.5.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    let label = text(format!("{:.0}%", pct))
        .size(tokens::text::TINY)
        .style(palette::neutral_text);

    row![track, label]
        .spacing(tokens::spacing::XS)
        .align_y(iced::Alignment::Center)
        .into()
}

// ── Full-Page Prop Firm Detail ────────────────────────────────────

fn prop_firm_detail_page<'a>(
    pf: &'a PropFirmResult,
    detail: &'a PropFirmDetailView,
) -> Element<'a, ManagerMessage> {
    let acct = &pf.account;

    // ── Header ───────────────────────────────────────────────
    let back_btn = icon_button(Icon::Return)
        .size(tokens::text::BODY)
        .tooltip("Back")
        .on_press(ManagerMessage::ClosePropFirmDetail)
        .style(crate::style::button::secondary);

    let (status_text, status_style): (&str, fn(&iced::Theme) -> iced::widget::text::Style) =
        match pf.status {
            PropFirmStatus::Passed => ("PASSED", palette::success_text),
            PropFirmStatus::Failed => ("FAILED", palette::error_text),
            PropFirmStatus::Active => ("ACTIVE", palette::info_text),
        };

    let header_row = row![
        back_btn,
        text(format!("{} Detail", acct.name)).size(tokens::text::LABEL),
        iced::widget::Space::new().width(Length::Fill),
        text(status_text.to_string())
            .size(tokens::text::LABEL)
            .style(status_style),
    ]
    .spacing(tokens::spacing::SM)
    .align_y(iced::Alignment::Center);

    // ── Deterministic Result Section ─────────────────────────
    let det_header = section_header("DETERMINISTIC RESULT");

    let dd_pct = if acct.size > 0.0 {
        pf.worst_drawdown / acct.size * 100.0
    } else {
        0.0
    };
    let daily_pct = if acct.size > 0.0 {
        pf.worst_daily_loss / acct.size * 100.0
    } else {
        0.0
    };

    let det_metrics = column![
        kv_row_mono("Account Size", &format_dollar(acct.size), None,),
        kv_row_mono("Profit Target", &format_dollar(acct.profit_target), None,),
        kv_row_mono("DD Limit", &format_dollar(acct.max_trailing_dd), None,),
        kv_row_mono("Daily Limit", &format_dollar(acct.daily_loss_limit), None,),
        rule::horizontal(1),
        kv_row_mono(
            "Final P&L",
            &format_pnl(pf.final_pnl),
            Some(if pf.final_pnl >= 0.0 {
                palette::success_text
            } else {
                palette::error_text
            }),
        ),
        kv_row_mono(
            "Worst DD",
            &format!("{} ({:.1}%)", format_dollar(pf.worst_drawdown), dd_pct),
            Some(if pf.hit_drawdown_limit {
                palette::error_text
            } else {
                palette::neutral_text
            }),
        ),
        kv_row_mono(
            "Worst Daily",
            &format!("{} ({:.1}%)", format_dollar(pf.worst_daily_loss), daily_pct),
            Some(if pf.hit_daily_limit {
                palette::warning_text
            } else {
                palette::neutral_text
            }),
        ),
        kv_row_mono("Progress", &format!("{:.1}%", pf.progress_pct), None,),
    ]
    .spacing(tokens::spacing::XS);

    let det_left = container(det_metrics).width(Length::Fixed(260.0));

    let det_chart = canvas(PropFirmEquityChart {
        equity_curve: &pf.equity_curve,
        account_size: acct.size,
        profit_target: acct.profit_target,
        max_trailing_dd: acct.max_trailing_dd,
        breach_trade_idx: pf.breach_trade_idx,
        cache: &detail.equity_chart_cache,
    })
    .width(Length::Fill)
    .height(Length::Fixed(220.0));

    let det_right = container(det_chart)
        .width(Length::Fill)
        .height(Length::Fixed(220.0));

    let det_body = row![det_left, det_right].spacing(tokens::spacing::MD);

    let det_section = container(column![det_header, det_body].spacing(tokens::spacing::SM))
        .padding(tokens::spacing::MD)
        .width(Length::Fill)
        .style(card_background);

    // ── Monte Carlo Section ──────────────────────────────────
    let mc = &pf.monte_carlo;
    let mc_header = section_header(&format!(
        "MONTE CARLO SIMULATION ({} iterations)",
        mc.num_iterations
    ));

    let mc_metrics = column![
        kv_row_mono(
            "Pass Rate",
            &format!("{:.1}%", mc.pass_rate * 100.0),
            Some(if mc.pass_rate >= 0.5 {
                palette::success_text
            } else {
                palette::error_text
            }),
        ),
        kv_row_mono(
            "Avg to Pass",
            &mc.avg_trades_to_pass
                .map(|v| format!("{:.0} trades", v))
                .unwrap_or_else(|| "N/A".to_string()),
            None,
        ),
        kv_row_mono(
            "Avg to Fail",
            &mc.avg_trades_to_fail
                .map(|v| format!("{:.0} trades", v))
                .unwrap_or_else(|| "N/A".to_string()),
            None,
        ),
        rule::horizontal(1),
        kv_row_mono(
            "Median P&L",
            &format_pnl(mc.median_final_pnl),
            Some(if mc.median_final_pnl >= 0.0 {
                palette::success_text
            } else {
                palette::error_text
            }),
        ),
        kv_row_mono("P5 P&L", &format_pnl(mc.p5_final_pnl), None,),
        kv_row_mono("P95 P&L", &format_pnl(mc.p95_final_pnl), None,),
    ]
    .spacing(tokens::spacing::XS);

    let mc_left = container(mc_metrics).width(Length::Fixed(260.0));

    let mc_chart = canvas(PropFirmMonteCarloChart {
        paths: &mc.sample_paths,
        account_size: acct.size,
        profit_target: acct.profit_target,
        dd_limit: acct.max_trailing_dd,
        cache: &detail.mc_chart_cache,
    })
    .width(Length::Fill)
    .height(Length::Fixed(220.0));

    let mc_right = container(mc_chart)
        .width(Length::Fill)
        .height(Length::Fixed(220.0));

    let mc_body = row![mc_left, mc_right].spacing(tokens::spacing::MD);

    let mc_section = container(column![mc_header, mc_body].spacing(tokens::spacing::SM))
        .padding(tokens::spacing::MD)
        .width(Length::Fill)
        .style(card_background);

    // ── Assemble ─────────────────────────────────────────────
    let content = column![header_row, rule::horizontal(1), det_section, mc_section,]
        .spacing(tokens::spacing::LG)
        .padding(tokens::spacing::MD);

    scrollable(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// ── Risk & Expectancy Card ─────────────────────────────────────────

fn risk_expectancy_card<'a>(analytics: &ComputedAnalytics) -> Element<'a, ManagerMessage> {
    let title = text("RISK & EXPECTANCY")
        .size(tokens::text::LABEL)
        .style(palette::neutral_text);

    let e_style: fn(&iced::Theme) -> iced::widget::text::Style =
        if analytics.expectancy_per_trade >= 0.0 {
            palette::success_text
        } else {
            palette::error_text
        };

    let var_style: fn(&iced::Theme) -> iced::widget::text::Style = if analytics.var_95 >= 0.0 {
        palette::success_text
    } else {
        palette::error_text
    };

    let cvar_style: fn(&iced::Theme) -> iced::widget::text::Style = if analytics.cvar_99 >= 0.0 {
        palette::success_text
    } else {
        palette::error_text
    };

    let rows = column![
        kv_row_mono(
            "E[trade]",
            &format!("${:.2}", analytics.expectancy_per_trade),
            Some(e_style),
        ),
        kv_row_mono(
            "Payoff Ratio",
            &format!("{:.2}", analytics.payoff_ratio),
            None,
        ),
        kv_row_mono(
            "VaR (95%)",
            &format!("${:.2}", analytics.var_95),
            Some(var_style),
        ),
        kv_row_mono(
            "CVaR (99%)",
            &format!("${:.2}", analytics.cvar_99),
            Some(cvar_style),
        ),
    ]
    .spacing(tokens::spacing::XS);

    container(column![title, rows].spacing(tokens::spacing::SM))
        .padding(tokens::spacing::MD)
        .width(Length::FillPortion(1))
        .style(card_background)
        .into()
}

// ── Position Sizing Card ───────────────────────────────────────────

fn position_sizing_card<'a>(analytics: &ComputedAnalytics) -> Element<'a, ManagerMessage> {
    let title = text("POSITION SIZING")
        .size(tokens::text::LABEL)
        .style(palette::neutral_text);

    let rows = column![
        kv_row_mono(
            "Kelly %",
            &format!("{:.1}%", analytics.kelly_criterion * 100.0),
            None,
        ),
        kv_row_mono("Optimal f", &format!("{:.2}", analytics.optimal_f), None,),
        kv_row_mono(
            "Risk of Ruin",
            &format!("{:.1}%", analytics.risk_of_ruin),
            None,
        ),
        kv_row_mono(
            "Max Consec Loss",
            &format!("{}", analytics.max_consecutive_losses),
            None,
        ),
    ]
    .spacing(tokens::spacing::XS);

    container(column![title, rows].spacing(tokens::spacing::SM))
        .padding(tokens::spacing::MD)
        .width(Length::FillPortion(1))
        .style(card_background)
        .into()
}

// ── Shared Helpers ─────────────────────────────────────────────────

fn kv_row(
    label: &str,
    value: &str,
    style: Option<fn(&iced::Theme) -> iced::widget::text::Style>,
) -> Element<'static, ManagerMessage> {
    let lbl = text(label.to_string())
        .size(tokens::text::SMALL)
        .style(palette::neutral_text)
        .width(Length::Fixed(80.0));

    let val = if let Some(s) = style {
        text(value.to_string()).size(tokens::text::SMALL).style(s)
    } else {
        text(value.to_string()).size(tokens::text::SMALL)
    };

    row![lbl, val].spacing(tokens::spacing::SM).into()
}

fn kv_row_mono(
    label: &str,
    value: &str,
    style: Option<fn(&iced::Theme) -> iced::widget::text::Style>,
) -> Element<'static, ManagerMessage> {
    let lbl = text(label.to_string())
        .size(tokens::text::SMALL)
        .style(palette::neutral_text)
        .width(Length::Fixed(120.0));

    let val = if let Some(s) = style {
        text(value.to_string())
            .size(tokens::text::SMALL)
            .font(AZERET_MONO)
            .style(s)
    } else {
        text(value.to_string())
            .size(tokens::text::SMALL)
            .font(AZERET_MONO)
    };

    row![lbl, val].spacing(tokens::spacing::SM).into()
}

// ── Currency Formatting ─────────────────────────────────────────────

fn format_dollar(value: f64) -> String {
    let abs = value.abs() as u64;
    if abs >= 1000 {
        format!("${},{:03}", abs / 1000, abs % 1000)
    } else {
        format!("${}", abs)
    }
}

fn format_pnl(value: f64) -> String {
    let sign = if value >= 0.0 { "+" } else { "-" };
    let abs = value.abs() as u64;
    if abs >= 1000 {
        format!("{}${},{:03}", sign, abs / 1000, abs % 1000)
    } else {
        format!("{}${}", sign, abs)
    }
}

// ── Shared Styles ───────────────────────────────────────────────────

fn card_background(theme: &iced::Theme) -> container::Style {
    let p = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(Color {
            a: 0.04,
            ..p.background.weak.color
        })),
        border: iced::Border {
            radius: tokens::radius::MD.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

// ── Empty State ─────────────────────────────────────────────────────

fn empty_state<'a>(msg: &'static str) -> Element<'a, ManagerMessage> {
    let label = text(msg)
        .size(tokens::text::BODY)
        .color(Color::from_rgba(1.0, 1.0, 1.0, 0.4));

    container(label)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::Alignment::Center)
        .align_y(iced::Alignment::Center)
        .into()
}
