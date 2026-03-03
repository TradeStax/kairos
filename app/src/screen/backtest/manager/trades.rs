//! Trades Tab — card-style summary bar with statistics and a sortable
//! trade table with zebra striping for the backtest management modal.

use super::{BacktestManager, ManagerMessage, TradeListSortColumn};
use crate::app::backtest_history::BacktestHistory;
use crate::components::primitives::icons::AZERET_MONO;
use crate::config::UserTimezone;
use crate::style::{self, palette, tokens};
use iced::widget::{button, column, container, row, rule, scrollable, text};
use iced::{Background, Color, Element, Length};
use tokens::backtest::{SELECTED_FILL, TABLE_HEADER_BG, TABLE_ROW_ALT};

// ── Public entry point ──────────────────────────────────────────────

pub fn view<'a>(
    manager: &'a BacktestManager,
    history: &'a BacktestHistory,
    timezone: UserTimezone,
) -> Element<'a, ManagerMessage> {
    view_trades(manager, history, timezone)
}

pub fn view_trades<'a>(
    manager: &'a BacktestManager,
    history: &'a BacktestHistory,
    timezone: UserTimezone,
) -> Element<'a, ManagerMessage> {
    let entry = manager.selected_id.and_then(|id| history.get(id));

    let result = entry.and_then(|e| e.result.as_ref());

    let Some(result) = result else {
        return empty_state();
    };

    let trades = &result.trades;
    if trades.is_empty() {
        return empty_state_msg("No trades recorded");
    }

    // ── Summary statistics ──────────────────────────────────────
    let (wins, losses, breakeven) = count_outcomes(trades);
    let (avg_win, avg_loss) = avg_win_loss(trades, wins, losses);
    let best = trades
        .iter()
        .map(|t| t.pnl_net_usd)
        .fold(f64::NEG_INFINITY, f64::max);
    let worst = trades
        .iter()
        .map(|t| t.pnl_net_usd)
        .fold(f64::INFINITY, f64::min);

    let summary_bar = build_summary_bar(
        trades.len(),
        wins,
        losses,
        breakeven,
        avg_win,
        avg_loss,
        best,
        worst,
    );

    // ── Detect multi-day span ───────────────────────────────────
    let is_multi_day = trades
        .first()
        .and_then(|first| {
            trades.last().map(|last| {
                let first_day = timezone.date_components((first.entry_time.0 / 1000) as i64);
                let last_day = timezone.date_components((last.exit_time.0 / 1000) as i64);
                first_day != last_day
            })
        })
        .unwrap_or(false);

    // ── Table header ────────────────────────────────────────────
    let header = build_table_header(manager);

    // ── Trade rows ──────────────────────────────────────────────
    let mut trade_rows: Vec<Element<'a, ManagerMessage>> =
        Vec::with_capacity(manager.sorted_indices.len());

    for (row_idx, &sorted_idx) in manager.sorted_indices.iter().enumerate() {
        let Some(trade) = trades.get(sorted_idx) else {
            continue;
        };

        let is_selected = manager.selected_trade == Some(sorted_idx);
        let is_odd = row_idx % 2 == 1;

        let row_bg = if is_selected {
            SELECTED_FILL
        } else if is_odd {
            TABLE_ROW_ALT
        } else {
            Color::TRANSPARENT
        };

        let entry_str = format_timestamp(trade.entry_time.0, is_multi_day, timezone);
        let exit_str = format_timestamp(trade.exit_time.0, is_multi_day, timezone);
        let side_str = if trade.side.is_buy() { "Long" } else { "Short" };
        let entry_price = format!("{:.2}", trade.entry_price.to_f64());
        let exit_price = format!("{:.2}", trade.exit_price.to_f64());
        let pnl_str = format!("{:+.2}", trade.pnl_net_usd);
        let ticks_str = format!("{:+}", trade.pnl_ticks);
        let rr_str = format!("{:.2}", trade.rr_ratio);
        let reason_str = trade.exit_reason.to_string();
        let idx_str = trade.index.to_string();

        let pnl_text_style: fn(&iced::Theme) -> iced::widget::text::Style =
            if trade.pnl_net_usd >= 0.0 {
                palette::success_text
            } else {
                palette::error_text
            };

        let ticks_text_style: fn(&iced::Theme) -> iced::widget::text::Style =
            if trade.pnl_ticks >= 0 {
                palette::success_text
            } else {
                palette::error_text
            };

        let row_content = row![
            mono_cell_fixed(idx_str, 35.0),
            trade_cell(entry_str),
            trade_cell(exit_str),
            trade_cell_fixed(side_str, 45.0),
            mono_cell(entry_price),
            mono_cell(exit_price),
            text(pnl_str)
                .size(tokens::text::SMALL)
                .font(AZERET_MONO)
                .width(Length::FillPortion(2))
                .style(pnl_text_style),
            text(ticks_str)
                .size(tokens::text::SMALL)
                .font(AZERET_MONO)
                .width(Length::Fixed(55.0))
                .style(ticks_text_style),
            mono_cell_fixed(rr_str, 45.0),
            trade_cell(reason_str),
        ]
        .spacing(1);

        let row_widget = container(
            button(row_content)
                .on_press(ManagerMessage::SelectTrade(Some(sorted_idx)))
                .padding([tokens::spacing::XS, tokens::spacing::SM])
                .width(Length::Fill)
                .style(|theme: &iced::Theme, status| {
                    style::button::transparent(theme, status, false)
                }),
        )
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(row_bg)),
            ..Default::default()
        })
        .width(Length::Fill);

        trade_rows.push(row_widget.into());
    }

    let trade_list =
        scrollable(column![header, rule::horizontal(1), column(trade_rows).spacing(0),].spacing(0))
            .height(Length::Fill);

    column![summary_bar, rule::horizontal(1), trade_list,]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// ── Summary Bar ─────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn build_summary_bar(
    total: usize,
    wins: usize,
    losses: usize,
    breakeven: usize,
    avg_win: f64,
    avg_loss: f64,
    best: f64,
    worst: f64,
) -> Element<'static, ManagerMessage> {
    let row1 = row![
        metric_card("TOTAL", &total.to_string(), None),
        metric_card("WINS", &wins.to_string(), Some(palette::success_text),),
        metric_card("LOSSES", &losses.to_string(), Some(palette::error_text),),
        metric_card("BREAKEVEN", &breakeven.to_string(), None,),
    ]
    .spacing(tokens::spacing::SM);

    let row2 = row![
        metric_card(
            "AVG WIN",
            &format!("${:.2}", avg_win),
            Some(palette::success_text),
        ),
        metric_card(
            "AVG LOSS",
            &format!("${:.2}", avg_loss.abs()),
            Some(palette::error_text),
        ),
        metric_card(
            "BEST TRADE",
            &format!("${:.2}", best),
            Some(palette::success_text),
        ),
        metric_card(
            "WORST TRADE",
            &format!("${:.2}", worst),
            Some(palette::error_text),
        ),
    ]
    .spacing(tokens::spacing::SM);

    container(
        column![row1, row2]
            .spacing(tokens::spacing::SM)
            .padding([tokens::spacing::MD, tokens::spacing::MD]),
    )
    .width(Length::Fill)
    .into()
}

fn metric_card(
    label: &str,
    value: &str,
    style: Option<fn(&iced::Theme) -> iced::widget::text::Style>,
) -> Element<'static, ManagerMessage> {
    let lbl = text(label.to_string())
        .size(tokens::text::TINY)
        .style(palette::neutral_text);

    let val = if let Some(s) = style {
        text(value.to_string())
            .size(tokens::text::BODY)
            .font(AZERET_MONO)
            .style(s)
    } else {
        text(value.to_string())
            .size(tokens::text::BODY)
            .font(AZERET_MONO)
    };

    container(column![lbl, val].spacing(tokens::spacing::XXS))
        .padding([tokens::spacing::SM, tokens::spacing::MD])
        .width(Length::FillPortion(1))
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(TABLE_HEADER_BG)),
            border: iced::Border {
                radius: tokens::radius::MD.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

// ── Table Header ────────────────────────────────────────────────────

fn build_table_header(manager: &BacktestManager) -> Element<'_, ManagerMessage> {
    let sort_col = manager.sort_column;
    let ascending = manager.sort_ascending;

    container(
        row![
            col_header_narrow("#", TradeListSortColumn::Index, 35.0, sort_col, ascending,),
            col_header("ENTRY", TradeListSortColumn::EntryTime, sort_col, ascending,),
            col_header("EXIT", TradeListSortColumn::ExitTime, sort_col, ascending,),
            col_header_narrow("SIDE", TradeListSortColumn::Side, 45.0, sort_col, ascending,),
            col_header(
                "ENTRY $",
                TradeListSortColumn::EntryTime,
                sort_col,
                ascending,
            ),
            col_header("EXIT $", TradeListSortColumn::ExitTime, sort_col, ascending,),
            col_header("P&L $", TradeListSortColumn::PnlUsd, sort_col, ascending,),
            col_header_narrow(
                "TICKS",
                TradeListSortColumn::PnlTicks,
                55.0,
                sort_col,
                ascending,
            ),
            col_header_narrow(
                "R:R",
                TradeListSortColumn::RrRatio,
                45.0,
                sort_col,
                ascending,
            ),
            col_header(
                "REASON",
                TradeListSortColumn::ExitReason,
                sort_col,
                ascending,
            ),
        ]
        .spacing(1)
        .padding([0.0, tokens::spacing::SM]),
    )
    .style(|_theme: &iced::Theme| container::Style {
        background: Some(Background::Color(TABLE_HEADER_BG)),
        border: iced::Border {
            radius: tokens::radius::SM.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .width(Length::Fill)
    .into()
}

fn col_header(
    label: &'static str,
    col: TradeListSortColumn,
    current: TradeListSortColumn,
    ascending: bool,
) -> Element<'static, ManagerMessage> {
    let indicator = sort_indicator(col, current, ascending);
    button(
        text(format!("{}{}", label, indicator))
            .size(tokens::text::TINY)
            .style(palette::neutral_text),
    )
    .on_press(ManagerMessage::SortTrades(col))
    .padding([tokens::spacing::XXS, tokens::spacing::XS])
    .width(Length::FillPortion(2))
    .style(|theme: &iced::Theme, status| style::button::transparent(theme, status, false))
    .into()
}

fn col_header_narrow(
    label: &'static str,
    col: TradeListSortColumn,
    width: f32,
    current: TradeListSortColumn,
    ascending: bool,
) -> Element<'static, ManagerMessage> {
    let indicator = sort_indicator(col, current, ascending);
    button(
        text(format!("{}{}", label, indicator))
            .size(tokens::text::TINY)
            .style(palette::neutral_text),
    )
    .on_press(ManagerMessage::SortTrades(col))
    .padding([tokens::spacing::XXS, tokens::spacing::XS])
    .width(Length::Fixed(width))
    .style(|theme: &iced::Theme, status| style::button::transparent(theme, status, false))
    .into()
}

fn sort_indicator(
    col: TradeListSortColumn,
    current: TradeListSortColumn,
    ascending: bool,
) -> &'static str {
    if col == current {
        if ascending { " \u{2191}" } else { " \u{2193}" }
    } else {
        ""
    }
}

// ── Cell helpers ────────────────────────────────────────────────────

fn trade_cell(value: impl Into<String>) -> Element<'static, ManagerMessage> {
    text(value.into())
        .size(tokens::text::SMALL)
        .width(Length::FillPortion(2))
        .into()
}

fn trade_cell_fixed(value: impl Into<String>, width: f32) -> Element<'static, ManagerMessage> {
    text(value.into())
        .size(tokens::text::SMALL)
        .width(Length::Fixed(width))
        .into()
}

fn mono_cell(value: impl Into<String>) -> Element<'static, ManagerMessage> {
    text(value.into())
        .size(tokens::text::SMALL)
        .font(AZERET_MONO)
        .width(Length::FillPortion(2))
        .into()
}

fn mono_cell_fixed(value: impl Into<String>, width: f32) -> Element<'static, ManagerMessage> {
    text(value.into())
        .size(tokens::text::SMALL)
        .font(AZERET_MONO)
        .width(Length::Fixed(width))
        .into()
}

// ── Timestamp formatting ────────────────────────────────────────────

fn format_timestamp(ms: u64, multi_day: bool, tz: UserTimezone) -> String {
    let Some(dt_utc) = chrono::DateTime::from_timestamp_millis(ms as i64) else {
        return "?".to_string();
    };
    match tz {
        UserTimezone::Local => {
            let dt = dt_utc.with_timezone(&chrono::Local);
            if multi_day {
                dt.format("%m/%d %H:%M:%S").to_string()
            } else {
                dt.format("%H:%M:%S").to_string()
            }
        }
        UserTimezone::Utc => {
            if multi_day {
                dt_utc.format("%m/%d %H:%M:%S").to_string()
            } else {
                dt_utc.format("%H:%M:%S").to_string()
            }
        }
    }
}

// ── Statistics helpers ──────────────────────────────────────────────

fn count_outcomes(trades: &[backtest::TradeRecord]) -> (usize, usize, usize) {
    let mut wins = 0usize;
    let mut losses = 0usize;
    let mut breakeven = 0usize;
    for t in trades {
        if t.pnl_net_usd > 0.0 {
            wins += 1;
        } else if t.pnl_net_usd < 0.0 {
            losses += 1;
        } else {
            breakeven += 1;
        }
    }
    (wins, losses, breakeven)
}

fn avg_win_loss(trades: &[backtest::TradeRecord], wins: usize, losses: usize) -> (f64, f64) {
    let avg_win = if wins > 0 {
        trades
            .iter()
            .filter(|t| t.pnl_net_usd > 0.0)
            .map(|t| t.pnl_net_usd)
            .sum::<f64>()
            / wins as f64
    } else {
        0.0
    };
    let avg_loss = if losses > 0 {
        trades
            .iter()
            .filter(|t| t.pnl_net_usd < 0.0)
            .map(|t| t.pnl_net_usd)
            .sum::<f64>()
            / losses as f64
    } else {
        0.0
    };
    (avg_win, avg_loss)
}

// ── Empty states ────────────────────────────────────────────────────

fn empty_state<'a>() -> Element<'a, ManagerMessage> {
    empty_state_msg("Select a backtest")
}

fn empty_state_msg(msg: &str) -> Element<'static, ManagerMessage> {
    let label = text(msg.to_string())
        .size(tokens::text::LABEL)
        .color(Color::from_rgba(1.0, 1.0, 1.0, 0.4));

    container(label)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::Alignment::Center)
        .align_y(iced::Alignment::Center)
        .into()
}
