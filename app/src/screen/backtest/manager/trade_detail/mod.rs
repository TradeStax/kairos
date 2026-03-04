//! Trade Detail View — chart-dominant breakdown of a single trade.
//!
//! Renders a full session candlestick chart with entry/exit markers,
//! strategy overlays, and a compact info bar at the bottom.

mod chart;
mod sections;
mod strategy_context;

use super::{ManagerMessage, TradeDetailView};
use crate::config::UserTimezone;
use crate::style::{self, palette, tokens};
use backtest::{BacktestConfig, BacktestResult};
use iced::widget::{button, canvas, column, container, row, text};
use iced::{Element, Length};

/// Render the trade detail view inline in the trades tab.
pub fn view<'a>(
    detail: &'a TradeDetailView,
    result: &'a BacktestResult,
    config: &'a BacktestConfig,
    timezone: UserTimezone,
) -> Element<'a, ManagerMessage> {
    let Some(trade) = result.trades.get(detail.trade_index) else {
        return text("Trade not found").size(tokens::text::BODY).into();
    };

    let ticker_str = trade
        .instrument
        .map(|i| format!("{i:?}"))
        .unwrap_or_else(|| format!("{:?}", config.ticker));
    let side_str = if trade.side.is_buy() { "Long" } else { "Short" };
    let is_win = trade.pnl_net_usd >= 0.0;

    // ── Header ──────────────────────────────────────────────────
    let back_btn = button(text("\u{2190} Back").size(tokens::text::SMALL))
        .on_press(ManagerMessage::CloseTradeDetail)
        .padding([tokens::spacing::XXS, tokens::spacing::SM])
        .style(|theme: &iced::Theme, status| style::button::transparent(theme, status, false));

    let pnl_str = format!("{:+.2}", trade.pnl_net_usd);
    let pnl_style: fn(&iced::Theme) -> iced::widget::text::Style = if is_win {
        palette::success_text
    } else {
        palette::error_text
    };

    let trade_summary = row![
        text(format!(
            "Trade #{} \u{00B7} {} \u{00B7} {} \u{00B7} ",
            trade.index, ticker_str, side_str
        ))
        .size(tokens::text::SMALL)
        .style(palette::neutral_text),
        text(format!("${}", pnl_str))
            .size(tokens::text::SMALL)
            .style(pnl_style),
    ];

    let header = row![
        back_btn,
        iced::widget::Space::new().width(Length::Fill),
        trade_summary,
    ]
    .align_y(iced::Alignment::Center)
    .spacing(tokens::spacing::SM);

    // ── Chart ───────────────────────────────────────────────────
    let tick_size = backtest::config::InstrumentSpec::from_ticker(config.ticker)
        .tick_size
        .to_f64();

    let chart_canvas = canvas(chart::MiniTradeChart {
        trade,
        snapshot: trade.snapshot.as_ref(),
        tick_size,
        cache: &detail.chart_cache,
        strategy_id: &config.strategy_id,
    })
    .height(Length::Fill)
    .width(Length::Fill);

    // ── Info bar ─────────────────────────────────────────────────
    let info_bar = sections::view_info_bar(trade, config, timezone);

    // ── Layout ──────────────────────────────────────────────────
    container(
        column![header, chart_canvas, info_bar]
            .spacing(tokens::spacing::XS)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .padding(tokens::spacing::SM)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
