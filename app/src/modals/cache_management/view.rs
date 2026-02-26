//! View rendering for the cache management modal.

use std::collections::BTreeMap;

use iced::widget::{button, checkbox, column, container, row, scrollable, space, text, text_input};
use iced::{Alignment, Element, Length};

use crate::components::primitives::{self, Icon, icon_text};
use crate::style;
use crate::style::tokens;

use super::{CacheManagementMessage, CacheManagementModal, DeleteTarget, format_size};

impl CacheManagementModal {
    /// Render the modal body content (inside ModalShell).
    pub fn view(&self) -> Element<'_, CacheManagementMessage> {
        if self.loading {
            return container(primitives::body("Scanning cache..."))
                .width(Length::Fill)
                .height(300)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into();
        }

        if self.entries.is_empty() && !self.loading {
            return container(primitives::body("No cached data found"))
                .width(Length::Fill)
                .height(300)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into();
        }

        let header = self.view_summary_bar();
        let tree = self.view_tree();
        let footer = self.view_footer();

        column![header, tree, footer]
            .spacing(tokens::spacing::MD)
            .width(Length::Fill)
            .into()
    }

    /// Summary bar: total size, file count, and search input.
    fn view_summary_bar(&self) -> Element<'_, CacheManagementMessage> {
        let summary = primitives::body(format!(
            "Cache: {} \u{00B7} {} files",
            format_size(self.total_size),
            self.total_files,
        ));

        let search = text_input("Search...", &self.search_query)
            .on_input(CacheManagementMessage::SearchChanged)
            .size(tokens::text::BODY)
            .width(180);

        row![summary, space::horizontal().width(Length::Fill), search]
            .align_y(Alignment::Center)
            .spacing(tokens::spacing::MD)
            .into()
    }

    /// The tree view of symbol > schema > date entries.
    fn view_tree(&self) -> Element<'_, CacheManagementMessage> {
        let filtered = self.filtered_entries();

        // Group: symbol -> schema -> Vec<(global_index, &entry)>
        let mut tree: BTreeMap<&str, BTreeMap<&str, Vec<(usize, _)>>> = BTreeMap::new();
        for &(idx, entry) in &filtered {
            tree.entry(entry.symbol.as_str())
                .or_default()
                .entry(entry.schema.as_str())
                .or_default()
                .push((idx, entry));
        }

        let mut col = column![].spacing(tokens::spacing::XXS);

        for (symbol, schemas) in &tree {
            let sym_expanded = self.expanded_symbols.contains(*symbol);

            // Aggregate size/count for this symbol
            let sym_size: u64 = schemas.values().flatten().map(|(_, e)| e.size_bytes).sum();
            let sym_count: usize = schemas.values().map(|v| v.len()).sum();

            let chevron = if sym_expanded {
                Icon::ChevronDown
            } else {
                Icon::ExpandRight
            };

            let sym_row = button(
                row![
                    icon_text(chevron, 12),
                    primitives::label_text(symbol.to_string()),
                    space::horizontal().width(Length::Fill),
                    primitives::small(format!("{} \u{00B7} {}", format_size(sym_size), sym_count,)),
                ]
                .spacing(tokens::spacing::SM)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([tokens::spacing::SM, tokens::spacing::MD])
            .on_press(CacheManagementMessage::ToggleSymbolExpanded(
                symbol.to_string(),
            ))
            .style(|theme, status| style::button::transparent(theme, status, false));

            col = col.push(sym_row);

            if !sym_expanded {
                continue;
            }

            for (schema, entries) in schemas {
                let schema_key = (symbol.to_string(), schema.to_string());
                let schema_expanded = self.expanded_schemas.contains(&schema_key);

                let schema_size: u64 = entries.iter().map(|(_, e)| e.size_bytes).sum();
                let schema_count = entries.len();

                let chevron = if schema_expanded {
                    Icon::ChevronDown
                } else {
                    Icon::ExpandRight
                };

                let schema_row = button(
                    row![
                        space::horizontal().width(tokens::spacing::XL),
                        icon_text(chevron, 10),
                        primitives::body(schema.to_string()),
                        space::horizontal().width(Length::Fill),
                        primitives::small(format!(
                            "{} \u{00B7} {}",
                            format_size(schema_size),
                            schema_count,
                        )),
                    ]
                    .spacing(tokens::spacing::SM)
                    .align_y(Alignment::Center),
                )
                .width(Length::Fill)
                .padding([tokens::spacing::XS, tokens::spacing::MD])
                .on_press(CacheManagementMessage::ToggleSchemaExpanded(
                    symbol.to_string(),
                    schema.to_string(),
                ))
                .style(|theme, status| style::button::transparent(theme, status, false));

                col = col.push(schema_row);

                if !schema_expanded {
                    continue;
                }

                for &(idx, entry) in entries {
                    let is_selected = self.selected_entries.contains(&idx);

                    let date_row = row![
                        space::horizontal().width(tokens::spacing::XL * 2.0),
                        checkbox(is_selected)
                            .on_toggle(move |_| {
                                CacheManagementMessage::ToggleEntrySelected(idx)
                            })
                            .spacing(tokens::spacing::SM)
                            .size(14),
                        primitives::body(entry.date.format("%Y-%m-%d").to_string()),
                        space::horizontal().width(Length::Fill),
                        primitives::small(format_size(entry.size_bytes,)),
                    ]
                    .spacing(tokens::spacing::SM)
                    .align_y(Alignment::Center)
                    .padding([tokens::spacing::XS, tokens::spacing::MD]);

                    col = col.push(date_row);
                }
            }
        }

        let scrollable_tree = scrollable(col).height(380).style(style::scroll_bar);

        container(scrollable_tree).width(Length::Fill).into()
    }

    /// Footer with Clear All and Delete Selected buttons.
    fn view_footer(&self) -> Element<'_, CacheManagementMessage> {
        let clear_all_btn = button(
            row![
                icon_text(Icon::TrashBin, 12),
                text("Clear All").size(tokens::text::BODY),
            ]
            .spacing(tokens::spacing::SM)
            .align_y(Alignment::Center),
        )
        .on_press_maybe(if self.deleting || self.entries.is_empty() {
            None
        } else {
            Some(CacheManagementMessage::RequestDelete(DeleteTarget::All))
        })
        .style(style::button::danger)
        .padding([tokens::spacing::SM, tokens::spacing::LG]);

        let selected = self.selected_count();
        let delete_label = if selected > 0 {
            format!("Delete Selected ({})", selected)
        } else {
            "Delete Selected".into()
        };
        let delete_selected_btn = button(text(delete_label).size(tokens::text::BODY))
            .on_press_maybe(if selected > 0 && !self.deleting {
                Some(CacheManagementMessage::RequestDelete(
                    DeleteTarget::Selected,
                ))
            } else {
                None
            })
            .style(style::button::primary)
            .padding([tokens::spacing::SM, tokens::spacing::LG]);

        let deselect: Element<'_, CacheManagementMessage> = if selected > 0 {
            button(text("Deselect All").size(tokens::text::BODY))
                .on_press(CacheManagementMessage::DeselectAll)
                .style(|theme, status| style::button::transparent(theme, status, false))
                .padding([tokens::spacing::SM, tokens::spacing::LG])
                .into()
        } else {
            space::horizontal().width(0).into()
        };

        row![
            clear_all_btn,
            deselect,
            space::horizontal().width(Length::Fill),
            delete_selected_btn,
        ]
        .spacing(tokens::spacing::MD)
        .align_y(Alignment::Center)
        .into()
    }
}
