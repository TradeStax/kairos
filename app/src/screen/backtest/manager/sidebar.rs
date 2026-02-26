//! Left sidebar for the backtest management modal.
//!
//! Shows a "New Backtest" button at the top, followed by a scrollable
//! list of all backtest history entries with status badges. Each entry
//! is a clickable row that selects the backtest for viewing.

use super::{BacktestManager, ManagerMessage};
use crate::app::backtest_history::{BacktestHistory, BacktestStatus};
use crate::components::primitives::{Icon, icon_text};
use crate::style::{self, palette, tokens};
use iced::widget::{button, column, container, row, rule, scrollable, text};
use iced::{Background, Color, Element, Length};

/// Build the sidebar panel for the management modal.
pub fn view_sidebar<'a>(
    manager: &'a BacktestManager,
    history: &'a BacktestHistory,
) -> Element<'a, ManagerMessage> {
    // ── New Backtest button ──────────────────────────────────────
    let new_btn_content = row![
        icon_text(Icon::ChartOutline, 12),
        text("New Backtest").size(tokens::text::BODY),
    ]
    .spacing(tokens::spacing::SM)
    .align_y(iced::Alignment::Center);

    let new_btn = button(new_btn_content)
        .on_press(ManagerMessage::NewBacktest)
        .width(Length::Fill)
        .padding([tokens::spacing::SM, tokens::spacing::MD])
        .style(|theme, status| style::button::transparent(theme, status, false));

    // ── History list ─────────────────────────────────────────────
    let entries = history.all_sorted();

    let list: Element<'a, ManagerMessage> = if entries.is_empty() {
        let hint = text("No backtests yet")
            .size(tokens::text::SMALL)
            .color(Color::from_rgba(1.0, 1.0, 1.0, 0.4));
        container(hint)
            .width(Length::Fill)
            .padding(tokens::spacing::LG)
            .align_x(iced::Alignment::Center)
            .into()
    } else {
        let mut col = column![].spacing(tokens::spacing::XXS);

        for entry in &entries {
            let is_selected = manager.selected_id == Some(entry.id);
            col = col.push(entry_row(entry, is_selected));
        }

        scrollable(col.width(Length::Fill))
            .style(style::scroll_bar)
            .height(Length::Fill)
            .into()
    };

    // ── Compose sidebar ──────────────────────────────────────────
    column![
        container(new_btn).padding([tokens::spacing::SM, tokens::spacing::SM,]),
        rule::horizontal(1),
        list,
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

// ── Entry row ────────────────────────────────────────────────────────

fn entry_row<'a>(
    entry: &crate::app::backtest_history::BacktestHistoryEntry,
    is_selected: bool,
) -> Element<'a, ManagerMessage> {
    // Line 1: "Strategy | Ticker"
    let header_label = format!("{} | {}", entry.strategy_name, entry.ticker,);
    let line1 = text(header_label).size(tokens::text::BODY);

    // Line 2: status-dependent
    let line2: Element<'a, ManagerMessage> = match entry.status {
        BacktestStatus::Running => {
            let pct = (entry.progress * 100.0) as u32;
            text(format!("Running {}%", pct))
                .size(tokens::text::SMALL)
                .style(palette::warning_text)
                .into()
        }
        BacktestStatus::Completed => view_completed_line(entry),
        BacktestStatus::Failed => {
            let snippet = entry.error.as_deref().unwrap_or("Unknown error");
            let truncated = if snippet.len() > 28 {
                format!("{}...", &snippet[..25])
            } else {
                snippet.to_string()
            };
            text(format!("FAILED {}", truncated))
                .size(tokens::text::SMALL)
                .style(palette::error_text)
                .into()
        }
    };

    let content = column![line1, line2]
        .spacing(tokens::spacing::XXS)
        .width(Length::Fill);

    let id = entry.id;
    button(content)
        .on_press(ManagerMessage::SelectBacktest(id))
        .width(Length::Fill)
        .padding([tokens::spacing::SM, tokens::spacing::MD])
        .style(move |theme, status| sidebar_entry_style(theme, status, is_selected))
        .into()
}

/// Completed backtest line: P&L colored green/red + short date.
fn view_completed_line<'a>(
    entry: &crate::app::backtest_history::BacktestHistoryEntry,
) -> Element<'a, ManagerMessage> {
    let date_str = format_date_short(entry.started_at_ms);

    if let Some(ref result) = entry.result {
        let pnl = result.metrics.net_pnl_usd;
        let pnl_str = format_pnl(pnl);

        let pnl_text = if pnl >= 0.0 {
            text(pnl_str)
                .size(tokens::text::SMALL)
                .style(palette::success_text)
        } else {
            text(pnl_str)
                .size(tokens::text::SMALL)
                .style(palette::error_text)
        };

        let date_text = text(date_str)
            .size(tokens::text::SMALL)
            .color(Color::from_rgba(1.0, 1.0, 1.0, 0.4));

        row![pnl_text, date_text]
            .spacing(tokens::spacing::SM)
            .into()
    } else {
        text(date_str)
            .size(tokens::text::SMALL)
            .color(Color::from_rgba(1.0, 1.0, 1.0, 0.4))
            .into()
    }
}

// ── Formatting helpers ───────────────────────────────────────────────

fn format_date_short(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|dt| dt.format("%b %d").to_string())
        .unwrap_or_default()
}

fn format_pnl(pnl: f64) -> String {
    let abs = pnl.abs();
    if pnl >= 0.0 {
        format!("+${}", format_usd(abs))
    } else {
        format!("-${}", format_usd(abs))
    }
}

fn format_usd(value: f64) -> String {
    let rounded = value.round() as i64;
    let s = rounded.abs().to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

// ── Button style ─────────────────────────────────────────────────────

fn sidebar_entry_style(
    theme: &iced::Theme,
    status: iced::widget::button::Status,
    is_selected: bool,
) -> iced::widget::button::Style {
    use iced::Border;
    use iced::widget::button::Status;

    let palette = theme.extended_palette();
    let accent = palette.primary.base.color;

    let selected_bg = Background::Color(Color {
        r: accent.r,
        g: accent.g,
        b: accent.b,
        a: 0.1,
    });

    iced::widget::button::Style {
        text_color: palette.background.base.text,
        background: match status {
            Status::Hovered => Some(palette.background.weak.color.into()),
            Status::Pressed => Some(palette.background.strong.color.into()),
            Status::Active | Status::Disabled => {
                if is_selected {
                    Some(selected_bg)
                } else {
                    None
                }
            }
        },
        border: Border {
            radius: tokens::radius::MD.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
