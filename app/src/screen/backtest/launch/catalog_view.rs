//! Catalog view — left panel, search/filter, and main view layout.

use super::*;

use crate::components::display::empty_state::EmptyStateBuilder;
use crate::components::input::search_field::SearchFieldBuilder;
use crate::components::layout::button_group::ButtonGroupBuilder;
use crate::components::overlay::modal_header::ModalHeaderBuilder;
use crate::components::primitives::icons::Icon;
use crate::style::{self, tokens};

use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, row, rule, scrollable, space, text},
};

use crate::components::primitives::badge::{BadgeKind, badge};

pub(super) fn category_badge(
    category: backtest::StrategyCategory,
) -> iced::Element<'static, Message> {
    let (label, kind) = match category {
        backtest::StrategyCategory::BreakoutMomentum => ("Breakout", BadgeKind::Warning),
        backtest::StrategyCategory::MeanReversion => ("Mean Rev.", BadgeKind::Info),
        backtest::StrategyCategory::TrendFollowing => ("Trend", BadgeKind::Success),
        backtest::StrategyCategory::OrderFlow => ("Order Flow", BadgeKind::Default),
        backtest::StrategyCategory::Custom => ("Custom", BadgeKind::Default),
    };
    badge(label, kind)
}

impl BacktestLaunchModal {
    // ── View ─────────────────────────────────────────────────────────

    pub fn view(&self) -> Element<'_, Message> {
        let header = ModalHeaderBuilder::new("Backtest Strategy").on_close(Message::Close);
        let search_and_filter = self.view_search_and_filter();
        let body = self.view_body();

        let inner = column![
            header,
            container(
                column![search_and_filter, body]
                    .spacing(tokens::spacing::MD)
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .padding(iced::Padding {
                top: tokens::spacing::MD,
                right: 0.0,
                bottom: tokens::spacing::XL,
                left: tokens::spacing::XL,
            }),
        ]
        .width(Length::Fill)
        .height(Length::Fill);

        container(inner)
            .max_width(tokens::layout::MODAL_WIDTH_XL)
            .max_height(600.0)
            .style(style::dashboard_modal)
            .into()
    }

    pub(super) fn view_search_and_filter(&self) -> Element<'_, Message> {
        let search = SearchFieldBuilder::new(
            "Search strategies...",
            &self.search_query,
            Message::SearchChanged,
        )
        .on_clear(Message::SearchChanged(String::new()))
        .width(Length::Fill);

        let filter_items: Vec<(String, Message)> = CategoryFilter::ALL
            .iter()
            .map(|cat| (cat.to_string(), Message::CategorySelected(*cat)))
            .collect();
        let selected_idx = CategoryFilter::ALL
            .iter()
            .position(|c| c == &self.category_filter)
            .unwrap_or(0);
        let filter_tabs = ButtonGroupBuilder::new(filter_items, selected_idx).tab_style();

        column![search, filter_tabs]
            .spacing(tokens::spacing::MD)
            .width(Length::Fill)
            .into()
    }

    pub(super) fn view_body(&self) -> Element<'_, Message> {
        let left_panel = self.view_left_panel();
        let right_panel = self.view_right_panel();

        row![
            container(left_panel)
                .width(Length::FillPortion(4))
                .height(Length::Fill),
            rule::vertical(1).style(style::split_ruler),
            container(right_panel)
                .width(Length::FillPortion(6))
                .height(Length::Fill)
                .padding(iced::padding::left(tokens::spacing::LG).right(tokens::spacing::LG),),
        ]
        .spacing(tokens::spacing::MD)
        .height(Length::Fill)
        .into()
    }

    // ── Left Panel ───────────────────────────────────────────────────

    pub(super) fn view_left_panel(&self) -> Element<'_, Message> {
        let filtered: Vec<&(String, Box<dyn backtest::Strategy>)> = self
            .strategy_snapshots
            .iter()
            .filter(|(_, s)| self.category_filter.matches(s.metadata().category))
            .filter(|(_, s)| {
                if self.search_query.is_empty() {
                    return true;
                }
                let q = self.search_query.to_lowercase();
                s.metadata().name.to_lowercase().contains(&q)
                    || s.id().to_lowercase().contains(&q)
                    || s.metadata()
                        .category
                        .to_string()
                        .to_lowercase()
                        .contains(&q)
            })
            .collect();

        if filtered.is_empty() {
            let empty = container(
                EmptyStateBuilder::new("No strategies match your search").icon(Icon::Search),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill);

            return empty.into();
        }

        let mut content = column![].spacing(tokens::spacing::XXS);

        for (id, strategy) in &filtered {
            let is_selected = self.selected_strategy_id.as_deref() == Some(id.as_str());
            content = content.push(self.strategy_list_row(id, strategy.as_ref(), is_selected));
        }

        scrollable(content.width(Length::Fill))
            .style(style::scroll_bar)
            .height(Length::Fill)
            .into()
    }

    fn strategy_list_row(
        &self,
        id: &str,
        strategy: &dyn backtest::Strategy,
        is_selected: bool,
    ) -> Element<'_, Message> {
        let meta = strategy.metadata();
        let cat_badge = category_badge(meta.category);

        let name_text = text(meta.name.clone()).size(tokens::text::BODY);

        let content_row = row![name_text, space::horizontal(), cat_badge,]
            .spacing(tokens::spacing::SM)
            .align_y(Alignment::Center)
            .width(Length::Fill);

        let sid = id.to_string();
        button(content_row)
            .on_press(Message::SelectStrategy(sid))
            .width(Length::Fill)
            .padding([tokens::spacing::SM, tokens::spacing::MD])
            .style(move |theme, status| style::button::modifier(theme, status, is_selected))
            .into()
    }
}
