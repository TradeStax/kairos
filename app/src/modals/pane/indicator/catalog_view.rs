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
    widget::{
        button, column, container, row, rule, scrollable, space, text, toggler,
    },
};

use super::helpers::category_badge;

impl IndicatorManagerModal {
    // ── View ─────────────────────────────────────────────────────────

    pub fn view(&self) -> Element<'_, Message> {
        let header = ModalHeaderBuilder::new("Indicators")
            .on_close(Message::Close);
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

    pub(super) fn view_search_and_filter(
        &self,
    ) -> Element<'_, Message> {
        let search = SearchFieldBuilder::new(
            "Search indicators...",
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
        let filter_tabs =
            ButtonGroupBuilder::new(filter_items, selected_idx)
                .tab_style();

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
                .padding(
                    iced::padding::left(tokens::spacing::LG)
                        .right(tokens::spacing::LG),
                ),
        ]
        .spacing(tokens::spacing::MD)
        .height(Length::Fill)
        .into()
    }

    // ── Left Panel ───────────────────────────────────────────────────

    pub(super) fn view_left_panel(&self) -> Element<'_, Message> {
        let registry = crate::app::init::services::create_unified_registry();
        let all_studies = registry.list();

        // Filter studies by search and category
        let filtered_studies: Vec<&study::StudyInfo> = all_studies
            .iter()
            .filter(|info| {
                self.category_filter.matches(info.category)
            })
            .filter(|info| {
                if self.search_query.is_empty() {
                    return true;
                }
                let q = self.search_query.to_lowercase();
                info.name.to_lowercase().contains(&q)
                    || info.id.to_lowercase().contains(&q)
                    || info.category.to_string().to_lowercase().contains(&q)
            })
            .collect();

        // Split into active and available
        let active_study_rows: Vec<Element<_>> = self
            .active_study_ids
            .iter()
            .filter_map(|sid| {
                filtered_studies
                    .iter()
                    .find(|info| info.id == *sid)
                    .map(|info| self.study_list_row(info, true))
            })
            .collect();

        let available_studies: Vec<Element<_>> = filtered_studies
            .iter()
            .filter(|info| !self.active_study_ids.contains(&info.id))
            .map(|info| self.study_list_row(info, false))
            .collect();

        let has_active = !active_study_rows.is_empty();
        let has_available = !available_studies.is_empty();

        let mut content = column![].spacing(tokens::spacing::MD);

        if has_active {
            content = content.push(
                text("Active")
                    .size(tokens::text::TINY)
                    .style(|theme: &iced::Theme| {
                        text::Style {
                            color: Some(
                                theme
                                    .extended_palette()
                                    .background
                                    .weak
                                    .text,
                            ),
                        }
                    }),
            );

            let mut active_col =
                column![].spacing(tokens::spacing::XXS);
            for row in active_study_rows {
                active_col = active_col.push(row);
            }
            content = content.push(active_col);
        }

        if has_available {
            content = content.push(
                text("Available")
                    .size(tokens::text::TINY)
                    .style(|theme: &iced::Theme| {
                        text::Style {
                            color: Some(
                                theme
                                    .extended_palette()
                                    .background
                                    .weak
                                    .text,
                            ),
                        }
                    }),
            );

            let mut avail_col =
                column![].spacing(tokens::spacing::XXS);
            for row in available_studies {
                avail_col = avail_col.push(row);
            }
            content = content.push(avail_col);
        }

        if !has_active && !has_available {
            let empty = container(
                EmptyStateBuilder::new(
                    "No indicators match your search",
                )
                .icon(Icon::Search),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill);

            return empty.into();
        }

        scrollable(content.width(Length::Fill))
            .style(style::scroll_bar)
            .height(Length::Fill)
            .into()
    }

    pub(super) fn study_list_row(
        &self,
        info: &study::StudyInfo,
        is_active: bool,
    ) -> Element<'_, Message> {
        let study_id = info.id.clone();
        let name = info.name.clone();
        let category = info.category;
        let is_selected = self.selected
            == Some(SelectedIndicator::Study(info.id.clone()));

        let cat_badge = category_badge(category);

        let name_text = text(name).size(tokens::text::BODY);
        let toggle = toggler(is_active)
            .size(tokens::layout::TOGGLER_SIZE)
            .on_toggle({
                let sid = study_id.clone();
                move |_| Message::ToggleStudy(sid.clone())
            });

        let content_row = row![
            name_text,
            space::horizontal(),
            cat_badge,
            toggle,
        ]
        .spacing(tokens::spacing::SM)
        .align_y(Alignment::Center)
        .width(Length::Fill);

        button(content_row)
            .on_press(Message::SelectIndicator(
                SelectedIndicator::Study(study_id),
            ))
            .width(Length::Fill)
            .padding([tokens::spacing::SM, tokens::spacing::MD])
            .style(move |theme, status| {
                style::button::modifier(theme, status, is_selected)
            })
            .into()
    }
}
