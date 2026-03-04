//! Compact info bar for the trade detail view.
//!
//! Two-line summary replacing the previous 4-card grid layout.

use super::super::ManagerMessage;
use super::strategy_context;
use crate::components::primitives::icons::AZERET_MONO;
use crate::config::UserTimezone;
use crate::style::{palette, tokens};
use backtest::BacktestConfig;
use iced::widget::{column, container, row, text};
use iced::{Background, Color, Element, Length};

pub fn view_info_bar<'a>(
    trade: &'a backtest::TradeRecord,
    _config: &'a BacktestConfig,
    timezone: UserTimezone,
) -> Element<'a, ManagerMessage> {
    let side_str = if trade.side.is_buy() { "Long" } else { "Short" };
    let entry_price = format!("{:.2}", trade.entry_price.to_f64());
    let exit_price = format!("{:.2}", trade.exit_price.to_f64());
    let pnl_str = format!("${:+.2}", trade.pnl_net_usd);
    let ticks_str = format!("{:+} ticks", trade.pnl_ticks);
    let is_win = trade.pnl_net_usd >= 0.0;

    let entry_ts = super::super::trades::format_timestamp(trade.entry_time.0, true, timezone);
    let exit_ts = super::super::trades::format_timestamp(trade.exit_time.0, true, timezone);
    let duration = format_duration(trade.duration_ms.unwrap_or(0));
    let exit_reason = trade.exit_reason.to_string();

    // Line 1: Direction · Entry → Exit · P&L
    let pnl_style: fn(&iced::Theme) -> iced::widget::text::Style = if is_win {
        palette::success_text
    } else {
        palette::error_text
    };

    let line1 = row![
        text(format!(
            "{} \u{00B7} {} \u{2192} {}",
            side_str, entry_price, exit_price
        ))
        .size(tokens::text::SMALL)
        .font(AZERET_MONO),
        text(format!(" \u{00B7} {} ({})", pnl_str, ticks_str))
            .size(tokens::text::SMALL)
            .font(AZERET_MONO)
            .style(pnl_style),
    ]
    .align_y(iced::Alignment::Center);

    // Line 2: Time range · Duration · Exit Reason · Strategy context
    let mut line2_str = format!(
        "{} \u{2192} {} \u{00B7} {} \u{00B7} {}",
        entry_ts, exit_ts, duration, exit_reason
    );

    // Append strategy context summary if available
    if let Some(snapshot) = &trade.snapshot
        && let Some(ctx_summary) = strategy_context::strategy_context_summary(&snapshot.context)
    {
        line2_str.push_str(&format!(" \u{00B7} {}", ctx_summary));
    }

    let line2 = text(line2_str)
        .size(tokens::text::TINY)
        .style(palette::neutral_text);

    let content = column![line1, line2].spacing(tokens::spacing::XXS);

    container(content)
        .padding([tokens::spacing::SM, tokens::spacing::MD])
        .width(Length::Fill)
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.02))),
            border: iced::Border {
                radius: tokens::radius::SM.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn format_duration(ms: u64) -> String {
    let total_secs = ms / 1000;
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins.max(1))
    }
}
