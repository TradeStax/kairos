use crate::style::{self, tokens};
use data::{FuturesTicker, FuturesTickerInfo};
use iced::{
    Alignment, Element, Length, alignment, padding,
    widget::{button, column, container, row, rule, text, text_input},
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::app::FUTURES_PRODUCTS;

use tokens::component::ticker_panel::COMPACT_ROW_HEIGHT;

#[derive(Debug, Clone, PartialEq)]
pub enum RowSelection {
    Switch(FuturesTickerInfo),
    Add(FuturesTickerInfo),
    Remove(FuturesTickerInfo),
}

pub enum Action {
    RowSelected(RowSelection),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MiniPanel {
    search_query: String,
    pub search_box_id: iced::widget::Id,
}

impl Default for MiniPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    SearchChanged(String),
    RowSelected(RowSelection),
}

impl MiniPanel {
    pub fn new() -> Self {
        Self {
            search_query: String::new(),
            search_box_id: iced::widget::Id::unique(),
        }
    }

    pub fn update(&mut self, message: Message) -> Option<Action> {
        match message {
            Message::SearchChanged(q) => self.search_query = q.to_uppercase(),
            Message::RowSelected(t) => {
                return Some(Action::RowSelected(t));
            }
        }
        None
    }

    pub fn view<'a>(
        &'a self,
        tickers_info: &'a FxHashMap<FuturesTicker, FuturesTickerInfo>,
        selected_tickers: Option<&[FuturesTickerInfo]>,
        base_ticker: Option<FuturesTickerInfo>,
        ticker_ranges: &'a std::collections::HashMap<String, String>,
    ) -> Element<'a, Message> {
        if tickers_info.is_empty() {
            let top_bar = compact_top_bar(
                &self.search_query,
                &self.search_box_id,
                Message::SearchChanged,
            );
            let empty_msg = container(
                text("No connections available")
                    .size(tokens::text::SMALL)
                    .color(iced::Color::from_rgba(0.6, 0.6, 0.6, 0.6)),
            )
            .width(Length::Fill)
            .padding(tokens::spacing::MD)
            .align_x(Alignment::Center);

            return column![top_bar, empty_msg]
                .spacing(tokens::spacing::MD)
                .padding(padding::right(tokens::spacing::MD))
                .width(Length::Fill)
                .into();
        }

        view_compact_with(
            tickers_info,
            &self.search_query,
            &self.search_box_id,
            Message::RowSelected,
            Message::SearchChanged,
            selected_tickers,
            base_ticker,
            ticker_ranges,
        )
    }
}

// ── Compact dropdown view ─────────────────────────────────────────────

struct TickerRowData {
    ticker: FuturesTicker,
    product_name: String,
}

fn build_ticker_rows(
    tickers_info: &FxHashMap<FuturesTicker, FuturesTickerInfo>,
) -> Vec<TickerRowData> {
    let venue = data::FuturesVenue::CMEGlobex;
    let mut rows = Vec::new();

    for (symbol, product_name, _, _, _) in FUTURES_PRODUCTS {
        let ticker = FuturesTicker::new_with_display(
            symbol,
            venue,
            // split('.').next() always returns Some — at minimum the full string
            Some(symbol.split('.').next().unwrap()),
            Some(product_name),
        );

        if tickers_info.contains_key(&ticker) {
            rows.push(TickerRowData {
                ticker,
                product_name: product_name.to_string(),
            });
        }
    }

    rows
}

fn view_compact_with<'a, M, FSelect, FSearch>(
    tickers_info: &'a FxHashMap<FuturesTicker, FuturesTickerInfo>,
    search_query: &'a str,
    search_box_id: &'a iced::widget::Id,
    on_select: FSelect,
    on_search: FSearch,
    selected_tickers: Option<&[FuturesTickerInfo]>,
    base_ticker: Option<FuturesTickerInfo>,
    ticker_ranges: &'a std::collections::HashMap<String, String>,
) -> Element<'a, M>
where
    M: 'a + Clone,
    FSelect: 'static + Copy + Fn(RowSelection) -> M,
    FSearch: 'static + Copy + Fn(String) -> M,
{
    let injected_q = search_query.to_uppercase();
    let selection_enabled = selected_tickers.is_some();

    let mut excluded: FxHashSet<FuturesTicker> = selected_tickers
        .map(|slice| slice.iter().map(|ti| ti.ticker).collect())
        .unwrap_or_default();
    if let Some(bt) = base_ticker {
        excluded.insert(bt.ticker);
    }

    let ticker_rows = build_ticker_rows(tickers_info);
    let rows = filtered_rows(&ticker_rows, &injected_q, Some(&excluded));

    let base_ticker_id = base_ticker.map(|bt| bt.ticker);
    let selected_list: Vec<FuturesTickerInfo> = selected_tickers
        .map(|slice| {
            slice
                .iter()
                .copied()
                .filter(|ti| Some(ti.ticker) != base_ticker_id)
                .collect()
        })
        .unwrap_or_default();

    let top_bar = compact_top_bar(search_query, search_box_id, on_search);
    let selected_section = compact_selected_section(
        base_ticker,
        selected_list,
        on_select,
        selection_enabled,
        ticker_ranges,
    );

    let mut content = column![top_bar]
        .spacing(tokens::spacing::MD)
        .padding(padding::right(tokens::spacing::MD))
        .width(Length::Fill);
    if let Some(sel) = selected_section {
        content = content
            .push(sel)
            .push(rule::horizontal(1.0).style(style::split_ruler));
    }
    if rows.is_empty() {
        content = content.push(
            container(
                text("No other tickers available")
                    .size(tokens::text::SMALL)
                    .color(iced::Color::from_rgba(0.6, 0.6, 0.6, 0.6)),
            )
            .width(Length::Fill)
            .padding(tokens::spacing::MD)
            .align_x(Alignment::Center),
        );
    } else {
        content = content.push(compact_all_rows(
            tickers_info,
            &rows,
            on_select,
            selection_enabled,
            ticker_ranges,
        ));
    }

    content.into()
}

fn filtered_rows<'a>(
    ticker_rows: &'a [TickerRowData],
    search_upper: &str,
    excluded: Option<&FxHashSet<FuturesTicker>>,
) -> Vec<&'a TickerRowData> {
    let mut rows: Vec<_> = ticker_rows
        .iter()
        .filter(|row| !excluded.is_some_and(|ex| ex.contains(&row.ticker)))
        .filter_map(|row| calc_search_rank(row, search_upper).map(|rank| (row, rank)))
        .collect();

    rows.sort_by(|(_, ra), (_, rb)| (ra.bucket, ra.pos, ra.len).cmp(&(rb.bucket, rb.pos, rb.len)));

    rows.into_iter().map(|(row, _)| row).collect()
}

fn label_for(ticker: FuturesTicker) -> String {
    ticker.display_name().unwrap_or(ticker.as_str()).to_string()
}

fn compact_selected_section<'a, M, FSelect>(
    base_ticker: Option<FuturesTickerInfo>,
    selected_list: Vec<FuturesTickerInfo>,
    on_select: FSelect,
    selection_enabled: bool,
    ticker_ranges: &'a std::collections::HashMap<String, String>,
) -> Option<Element<'a, M>>
where
    M: 'a + Clone,
    FSelect: 'static + Copy + Fn(RowSelection) -> M,
{
    if !selection_enabled || (selected_list.is_empty() && base_ticker.is_none()) {
        return None;
    }

    let mut col = column![].spacing(tokens::spacing::XXS);

    if let Some(bt) = base_ticker {
        let label = label_for(bt.ticker);
        let range = ticker_ranges.get(bt.ticker.as_str()).cloned();
        col = col.push(mini_ticker_card(label, None, None, range, on_select));
    }

    for info in selected_list {
        let label = label_for(info.ticker);
        let range = ticker_ranges.get(info.ticker.as_str()).cloned();

        let (left_action, right) = if selection_enabled {
            (
                Some(RowSelection::Switch(info)),
                Some(("Remove", Some(RowSelection::Remove(info)))),
            )
        } else {
            (Some(RowSelection::Switch(info)), None)
        };

        col = col.push(mini_ticker_card(
            label,
            left_action,
            right,
            range,
            on_select,
        ));
    }

    Some(col.into())
}

fn compact_all_rows<'a, M, FSelect>(
    tickers_info: &FxHashMap<FuturesTicker, FuturesTickerInfo>,
    rows: &[&TickerRowData],
    on_select: FSelect,
    selection_enabled: bool,
    ticker_ranges: &std::collections::HashMap<String, String>,
) -> Element<'a, M>
where
    M: 'a + Clone,
    FSelect: 'static + Copy + Fn(RowSelection) -> M,
{
    let mut list = column![].spacing(tokens::spacing::XXS);
    for row_ref in rows {
        let label = label_for(row_ref.ticker);
        let info_opt: Option<FuturesTickerInfo> = tickers_info.get(&row_ref.ticker).copied();
        let range = ticker_ranges.get(row_ref.ticker.as_str()).cloned();

        let (left_action, right_action) = if selection_enabled {
            (
                info_opt.map(RowSelection::Switch),
                Some(("Add", info_opt.map(RowSelection::Add))),
            )
        } else {
            (info_opt.map(RowSelection::Switch), None)
        };

        list = list.push(mini_ticker_card(
            label,
            left_action,
            right_action,
            range,
            on_select,
        ));
    }
    list.into()
}

// ── Free functions ────────────────────────────────────────────────────

fn compact_top_bar<'a, M, FSearch>(
    search_query: &'a str,
    search_box_id: &'a iced::widget::Id,
    on_search: FSearch,
) -> Element<'a, M>
where
    M: 'a + Clone,
    FSearch: 'static + Copy + Fn(String) -> M,
{
    row![
        text_input("Search for a ticker...", search_query)
            .style(|theme, status| { crate::style::validated_text_input(theme, status, true) })
            .on_input(on_search)
            .id(search_box_id.clone())
            .align_x(Alignment::Start)
            .padding(tokens::spacing::SM),
    ]
    .align_y(Alignment::Center)
    .spacing(tokens::spacing::XS)
    .into()
}

fn mini_ticker_card<'a, M, FSelect>(
    label: String,
    left_action: Option<RowSelection>,
    right_label_and_action: Option<(&'static str, Option<RowSelection>)>,
    date_range_label: Option<String>,
    on_select: FSelect,
) -> Element<'a, M>
where
    M: 'a + Clone,
    FSelect: 'static + Copy + Fn(RowSelection) -> M,
{
    let mut left_row = row![text(label)]
        .spacing(tokens::spacing::SM)
        .align_y(alignment::Vertical::Center)
        .height(Length::Fill);

    if let Some(range_lbl) = date_range_label {
        left_row = left_row.push(
            text(range_lbl)
                .size(9.0)
                .color(iced::Color::from_rgba(0.5, 0.5, 0.5, 0.8)),
        );
    }

    let left_btn_base = button(left_row)
        .style(|theme, status| style::button::transparent(theme, status, false))
        .width(Length::Fill)
        .height(Length::Fill);

    let left_btn = if let Some(sel) = left_action {
        left_btn_base.on_press(on_select(sel))
    } else {
        left_btn_base
    };

    let right_el: Option<Element<'a, M>> = right_label_and_action.map(|(lbl, action)| {
        let btn_base = button(
            row![text(lbl).size(tokens::text::SMALL)]
                .align_y(alignment::Vertical::Center)
                .height(Length::Fill),
        )
        .style(|theme, status| style::button::transparent(theme, status, false))
        .height(Length::Fill);

        let btn = if let Some(act) = action {
            btn_base.on_press(on_select(act))
        } else {
            btn_base
        };

        btn.into()
    });

    let mut row_content = row![left_btn].align_y(alignment::Vertical::Center);

    if let Some(right) = right_el {
        row_content = row_content.push(iced::widget::rule::vertical(1.0));
        row_content = row_content.push(right);
    }

    container(row_content)
        .style(style::ticker_card)
        .height(Length::Fixed(COMPACT_ROW_HEIGHT))
        .width(Length::Fill)
        .into()
}

/// Rank for search matching (lower = better).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SearchRank {
    bucket: u8,
    pos: u16,
    len: u16,
}

fn calc_search_rank(row: &TickerRowData, query: &str) -> Option<SearchRank> {
    if query.is_empty() {
        return Some(SearchRank {
            bucket: 0,
            pos: 0,
            len: 0,
        });
    }

    let symbol = row.ticker.as_str().to_uppercase();
    let product = row.product_name.to_uppercase();

    let score_candidate = |cand: &str| -> Option<SearchRank> {
        let (bucket, pos) = if cand == query {
            (0_u8, 0_usize)
        } else if cand.starts_with(query) {
            (1_u8, 0_usize)
        } else if cand.ends_with(query) {
            (2_u8, 0_usize)
        } else if let Some(p) = cand.find(query) {
            (3_u8, p)
        } else {
            return None;
        };

        Some(SearchRank {
            bucket,
            pos: (pos.min(u16::MAX as usize)) as u16,
            len: (cand.len().min(u16::MAX as usize)) as u16,
        })
    };

    let mut best: Option<SearchRank> = None;

    for cand in [symbol.as_str(), product.as_str()] {
        let Some(rank) = score_candidate(cand) else {
            continue;
        };

        best = Some(match best {
            None => rank,
            Some(cur) => {
                if (rank.bucket, rank.pos, rank.len) < (cur.bucket, cur.pos, cur.len) {
                    rank
                } else {
                    cur
                }
            }
        });
    }

    best
}
