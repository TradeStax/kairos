//! Tickers Table - CME Globex Futures ticker data and compact selection dropdown.
//!
//! Provides a searchable, filterable list of CME Globex futures contracts for the
//! ticker selection dropdown in pane modals.

use crate::{
    modal::pane::tickers::RowSelection,
    style::{self, tokens},
};
use data::state::pane_config::ContentKind;
use exchange::{
    FuturesTicker, FuturesTickerInfo, FuturesVenue, TickerStats,
};
use iced::{
    Alignment, Element, Length, Subscription, Task,
    alignment,
    padding,
    widget::{button, column, container, row, rule, text, text_input},
};
use rustc_hash::{FxHashMap, FxHashSet};
use std::cmp::Ordering;

const UPDATE_INTERVAL: u64 = 300;
const COMPACT_ROW_HEIGHT: f32 = 28.0;

/// CME Futures Products - Standard continuous contracts
const FUTURES_PRODUCTS: &[(&str, &str, f32, f32, f32)] = &[
    // Symbol, Product Name, Tick Size, Min Qty, Contract Size
    ("ES.c.0", "E-mini S&P 500", 0.25, 1.0, 50.0),
    ("NQ.c.0", "E-mini Nasdaq-100", 0.25, 1.0, 20.0),
    ("YM.c.0", "E-mini Dow", 1.0, 1.0, 5.0),
    ("RTY.c.0", "E-mini Russell 2000", 0.1, 1.0, 50.0),
    ("CL.c.0", "Crude Oil", 0.01, 1.0, 1000.0),
    ("GC.c.0", "Gold", 0.10, 1.0, 100.0),
    ("SI.c.0", "Silver", 0.005, 1.0, 5000.0),
    ("ZN.c.0", "10-Year T-Note", 0.015625, 1.0, 1000.0),
    ("ZB.c.0", "30-Year T-Bond", 0.03125, 1.0, 1000.0),
    ("ZF.c.0", "5-Year T-Note", 0.0078125, 1.0, 1000.0),
    ("NG.c.0", "Natural Gas", 0.001, 1.0, 10000.0),
    ("HG.c.0", "Copper", 0.0005, 1.0, 25000.0),
];

pub enum Action {
    TickerSelected(FuturesTickerInfo, Option<ContentKind>),
    ErrorOccurred(data::InternalError),
}

#[derive(Debug, Clone)]
pub enum Message {
    ChangeSortOption(SortOptions),
    TickerSelected(FuturesTicker, Option<ContentKind>),
    FavoriteTicker(FuturesTicker),
    ToggleFavorites,
    FetchForTickerStats,
    UpdateTickerStats(Vec<TickerRowData>),
    ErrorOccurred(data::InternalError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum SortOptions {
    #[default]
    VolumeDesc,
    VolumeAsc,
    ChangeDesc,
    ChangeAsc,
}


pub struct TickersTable {
    ticker_rows: Vec<TickerRowData>,
    pub favorited_tickers: FxHashSet<FuturesTicker>,
    display_cache: FxHashMap<FuturesTicker, TickerDisplayData>,
    selected_sort_option: SortOptions,
    pub tickers_info: FxHashMap<FuturesTicker, FuturesTickerInfo>,
    show_favorites: bool,
    row_index: FxHashMap<FuturesTicker, usize>,
    cached_tickers_filter: Option<std::collections::HashSet<String>>,
}

#[derive(Debug, Clone)]
pub struct TickerRowData {
    ticker: FuturesTicker,
    #[allow(dead_code)]
    ticker_info: FuturesTickerInfo,
    stats: TickerStats,
    previous_stats: Option<TickerStats>,
    is_favorited: bool,
    product_name: String,
    contract_type_display: String,
}

#[derive(Debug, Clone)]
struct TickerDisplayData {
    display_ticker: String,
    product_name: String,
    contract_type: String,
    mark_price_display: String,
    daily_change_pct: String,
    volume_display: String,
    price_unchanged_part: String,
    price_changed_part: String,
    price_change_direction: PriceChangeDirection,
    card_color_alpha: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PriceChangeDirection {
    Increased,
    Decreased,
    Unchanged,
}

// ---------------------------------------------------------------------------
// State management
// ---------------------------------------------------------------------------

impl TickersTable {
    pub fn new() -> (Self, Task<Message>) {
        Self::new_with_favorited(FxHashSet::default())
    }

    pub fn new_with_favorited(
        favorited_tickers: FxHashSet<FuturesTicker>,
    ) -> (Self, Task<Message>) {
        let mut instance = Self {
            ticker_rows: Vec::new(),
            display_cache: FxHashMap::default(),
            favorited_tickers,
            selected_sort_option: SortOptions::default(),
            tickers_info: FxHashMap::default(),
            show_favorites: false,
            row_index: FxHashMap::default(),
            cached_tickers_filter: None,
        };

        instance.initialize_futures_products();

        (instance, Task::none())
    }

    fn initialize_futures_products(&mut self) {
        let venue = FuturesVenue::CMEGlobex;

        for (symbol, product_name, tick_size, min_qty, contract_size) in
            FUTURES_PRODUCTS
        {
            let ticker = FuturesTicker::new_with_display(
                symbol,
                venue,
                Some(symbol.split('.').next().unwrap()),
                Some(product_name),
            );

            let ticker_info = FuturesTickerInfo::new(
                ticker,
                *tick_size,
                *min_qty,
                *contract_size,
            );

            self.tickers_info.insert(ticker, ticker_info);

            let (_, contract_type_display) = ticker.display_symbol_and_type();

            let row = TickerRowData {
                ticker,
                ticker_info,
                stats: TickerStats::default(),
                previous_stats: None,
                is_favorited: self.favorited_tickers.contains(&ticker),
                product_name: product_name.to_string(),
                contract_type_display,
            };

            self.ticker_rows.push(row);
        }

        self.rebuild_index();
        self.update_display_cache();
    }

    pub fn update(&mut self, message: Message) -> Option<Action> {
        match message {
            Message::ChangeSortOption(option) => {
                self.change_sort_option(option);
            }
            Message::FavoriteTicker(ticker) => {
                self.favorite_ticker(ticker);
            }
            Message::ToggleFavorites => {
                self.show_favorites = !self.show_favorites;
            }
            Message::TickerSelected(ticker, content) => {
                let ticker_info = self.tickers_info.get(&ticker).copied();

                if let Some(ticker_info) = ticker_info {
                    return Some(Action::TickerSelected(ticker_info, content));
                } else {
                    log::warn!("Ticker info not found for {ticker:?}");
                }
            }
            Message::FetchForTickerStats => {
                log::debug!(
                    "Ticker stats refresh - real-time data not yet implemented"
                );
            }
            Message::UpdateTickerStats(new_rows) => {
                self.update_ticker_rows(new_rows);
                self.sort_ticker_rows();
            }
            Message::ErrorOccurred(err) => {
                log::error!("Error occurred: {err}");
                return Some(Action::ErrorOccurred(err));
            }
        }
        None
    }

    pub fn subscription(&self) -> Subscription<Message> {
        iced::time::every(std::time::Duration::from_secs(UPDATE_INTERVAL))
            .map(|_| Message::FetchForTickerStats)
    }

    fn sort_ticker_rows(&mut self) {
        match self.selected_sort_option {
            SortOptions::VolumeDesc => {
                self.ticker_rows.sort_unstable_by(|a, b| {
                    b.stats
                        .daily_volume
                        .total_cmp(&a.stats.daily_volume)
                        .then_with(|| Ordering::Equal)
                });
            }
            SortOptions::VolumeAsc => {
                self.ticker_rows.sort_unstable_by(|a, b| {
                    a.stats
                        .daily_volume
                        .total_cmp(&b.stats.daily_volume)
                        .then_with(|| Ordering::Equal)
                });
            }
            SortOptions::ChangeDesc => {
                self.ticker_rows.sort_unstable_by(|a, b| {
                    b.stats
                        .daily_price_chg
                        .total_cmp(&a.stats.daily_price_chg)
                        .then_with(|| Ordering::Equal)
                });
            }
            SortOptions::ChangeAsc => {
                self.ticker_rows.sort_unstable_by(|a, b| {
                    a.stats
                        .daily_price_chg
                        .total_cmp(&b.stats.daily_price_chg)
                        .then_with(|| Ordering::Equal)
                });
            }
        }
        self.rebuild_index();
    }

    fn change_sort_option(&mut self, option: SortOptions) {
        if self.selected_sort_option == option {
            self.selected_sort_option = match self.selected_sort_option {
                SortOptions::VolumeDesc => SortOptions::VolumeAsc,
                SortOptions::VolumeAsc => SortOptions::VolumeDesc,
                SortOptions::ChangeDesc => SortOptions::ChangeAsc,
                SortOptions::ChangeAsc => SortOptions::ChangeDesc,
            };
        } else {
            self.selected_sort_option = option;
        }

        self.sort_ticker_rows();
    }

    fn rebuild_index(&mut self) {
        self.row_index.clear();
        for (i, row) in self.ticker_rows.iter().enumerate() {
            self.row_index.insert(row.ticker, i);
        }
    }

    fn favorite_ticker(&mut self, ticker: FuturesTicker) {
        if let Some(&idx) = self.row_index.get(&ticker) {
            let row = &mut self.ticker_rows[idx];
            row.is_favorited = !row.is_favorited;

            if row.is_favorited {
                self.favorited_tickers.insert(ticker);
            } else {
                self.favorited_tickers.remove(&ticker);
            }
        }
    }

    fn update_ticker_rows(&mut self, new_rows: Vec<TickerRowData>) {
        for new_row in new_rows {
            if let Some(&idx) = self.row_index.get(&new_row.ticker) {
                let row = &mut self.ticker_rows[idx];
                row.previous_stats = Some(row.stats);
                row.stats = new_row.stats;
            }
        }
        self.update_display_cache();
    }

    fn update_display_cache(&mut self) {
        self.display_cache.clear();
        let Some(filter) = &self.cached_tickers_filter else {
            return;
        };
        for row in &self.ticker_rows {
            if !filter.contains(&row.ticker.to_string()) {
                continue;
            }
            let display_data =
                compute_display_data(row, row.previous_stats);
            self.display_cache.insert(row.ticker, display_data);
        }
    }

    pub fn set_cached_filter(
        &mut self,
        cached_symbols: std::collections::HashSet<String>,
    ) {
        log::info!(
            "TABLE: Applying cached ticker filter: {} tickers available",
            cached_symbols.len()
        );
        self.cached_tickers_filter = Some(cached_symbols);
        self.update_display_cache();
    }

    fn filtered_rows<'a>(
        &'a self,
        search_upper: &str,
        excluded: Option<&FxHashSet<FuturesTicker>>,
    ) -> (Vec<&'a TickerRowData>, Vec<&'a TickerRowData>) {
        let mut fav_rows: Vec<_> = if self.show_favorites {
            self.ticker_rows
                .iter()
                .filter(|row| {
                    if !row.is_favorited
                        || excluded
                            .is_some_and(|ex| ex.contains(&row.ticker))
                    {
                        return false;
                    }
                    match &self.cached_tickers_filter {
                        Some(filter) => {
                            filter.contains(&row.ticker.to_string())
                        }
                        None => false,
                    }
                })
                .filter_map(|row| {
                    calc_search_rank(row, search_upper)
                        .map(|rank| (row, rank))
                })
                .collect()
        } else {
            Vec::new()
        };

        fav_rows.sort_by(|(a, ra), (b, rb)| {
            (ra.bucket, ra.pos)
                .cmp(&(rb.bucket, rb.pos))
                .then_with(|| match self.selected_sort_option {
                    SortOptions::VolumeDesc => b
                        .stats
                        .daily_volume
                        .total_cmp(&a.stats.daily_volume),
                    SortOptions::VolumeAsc => a
                        .stats
                        .daily_volume
                        .total_cmp(&b.stats.daily_volume),
                    SortOptions::ChangeDesc => b
                        .stats
                        .daily_price_chg
                        .total_cmp(&a.stats.daily_price_chg),
                    SortOptions::ChangeAsc => a
                        .stats
                        .daily_price_chg
                        .total_cmp(&b.stats.daily_price_chg),
                })
                .then_with(|| ra.len.cmp(&rb.len))
        });
        let fav_rows: Vec<&TickerRowData> =
            fav_rows.into_iter().map(|(row, _)| row).collect();

        let mut rest_rows: Vec<_> = self
            .ticker_rows
            .iter()
            .filter(|row| {
                if (self.show_favorites && row.is_favorited)
                    || excluded
                        .is_some_and(|ex| ex.contains(&row.ticker))
                {
                    return false;
                }
                match &self.cached_tickers_filter {
                    Some(filter) => {
                        filter.contains(&row.ticker.to_string())
                    }
                    None => false,
                }
            })
            .filter_map(|row| {
                calc_search_rank(row, search_upper)
                    .map(|rank| (row, rank))
            })
            .collect();

        rest_rows.sort_by(|(a, ra), (b, rb)| {
            (ra.bucket, ra.pos)
                .cmp(&(rb.bucket, rb.pos))
                .then_with(|| match self.selected_sort_option {
                    SortOptions::VolumeDesc => b
                        .stats
                        .daily_volume
                        .total_cmp(&a.stats.daily_volume),
                    SortOptions::VolumeAsc => a
                        .stats
                        .daily_volume
                        .total_cmp(&b.stats.daily_volume),
                    SortOptions::ChangeDesc => b
                        .stats
                        .daily_price_chg
                        .total_cmp(&a.stats.daily_price_chg),
                    SortOptions::ChangeAsc => a
                        .stats
                        .daily_price_chg
                        .total_cmp(&b.stats.daily_price_chg),
                })
                .then_with(|| ra.len.cmp(&rb.len))
        });
        let rest_rows: Vec<&TickerRowData> =
            rest_rows.into_iter().map(|(row, _)| row).collect();

        (fav_rows, rest_rows)
    }

    fn filtered_rows_compact<'a>(
        &'a self,
        injected_q: &str,
        excluded: &FxHashSet<FuturesTicker>,
    ) -> (Vec<&'a TickerRowData>, Vec<&'a TickerRowData>) {
        self.filtered_rows(injected_q, Some(excluded))
    }
}

// ---------------------------------------------------------------------------
// Compact dropdown view (used by MiniPanel)
// ---------------------------------------------------------------------------

impl TickersTable {
    pub fn view_compact_with<'a, M, FSelect, FSearch>(
        &'a self,
        search_query: &'a str,
        search_box_id: &'a iced::widget::Id,
        on_select: FSelect,
        on_search: FSearch,
        selected_tickers: Option<&'a [FuturesTickerInfo]>,
        base_ticker: Option<FuturesTickerInfo>,
    ) -> Element<'a, M>
    where
        M: 'a + Clone,
        FSelect: 'static + Copy + Fn(RowSelection) -> M,
        FSearch: 'static + Copy + Fn(String) -> M,
    {
        let injected_q = search_query.to_uppercase();

        let selection_enabled = selected_tickers.is_some();

        let mut selected_set: FxHashSet<FuturesTicker> = selected_tickers
            .map(|slice| slice.iter().map(|ti| ti.ticker).collect())
            .unwrap_or_default();
        if let Some(bt) = base_ticker {
            selected_set.insert(bt.ticker);
        }

        let (fav_rows, rest_rows) =
            self.filtered_rows_compact(&injected_q, &selected_set);

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

        let top_bar =
            compact_top_bar(search_query, search_box_id, on_search);
        let selected_section = self.compact_selected_section(
            base_ticker,
            selected_list,
            on_select,
            selection_enabled,
        );

        let list = self.compact_all_rows(
            &fav_rows,
            &rest_rows,
            on_select,
            selection_enabled,
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
        content = content.push(list);

        content.into()
    }

    fn compact_selected_section<'a, M, FSelect>(
        &'a self,
        base_ticker: Option<FuturesTickerInfo>,
        selected_list: Vec<FuturesTickerInfo>,
        on_select: FSelect,
        selection_enabled: bool,
    ) -> Option<Element<'a, M>>
    where
        M: 'a + Clone,
        FSelect: 'static + Copy + Fn(RowSelection) -> M,
    {
        if base_ticker.is_none() && selected_list.is_empty() {
            return None;
        }

        let mut col = column![].spacing(tokens::spacing::XXS);

        if let Some(bt) = base_ticker {
            let label = self.label_for(bt.ticker);
            col = col.push(mini_ticker_card(
                label, None, None, None, on_select,
            ));
        }

        for info in selected_list {
            let label = self.label_for(info.ticker);

            let (left_action, right) = if selection_enabled {
                (
                    Some(RowSelection::Switch(info)),
                    Some((
                        "Remove",
                        Some(RowSelection::Remove(info)),
                    )),
                )
            } else {
                (Some(RowSelection::Switch(info)), None)
            };

            col = col.push(mini_ticker_card(
                label,
                left_action,
                right,
                None,
                on_select,
            ));
        }

        Some(col.into())
    }

    fn compact_all_rows<'a, M, FSelect>(
        &'a self,
        fav_rows: &[&'a TickerRowData],
        rest_rows: &[&'a TickerRowData],
        on_select: FSelect,
        selection_enabled: bool,
    ) -> Element<'a, M>
    where
        M: 'a + Clone,
        FSelect: 'static + Copy + Fn(RowSelection) -> M,
    {
        let mut list = column![].spacing(tokens::spacing::XXS);
        for row_ref in fav_rows.iter().chain(rest_rows.iter()) {
            let label = self.label_for(row_ref.ticker);
            let info_opt: Option<FuturesTickerInfo> =
                self.tickers_info.get(&row_ref.ticker).copied();

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
                None,
                on_select,
            ));
        }
        list.into()
    }

    fn label_for(&self, ticker: FuturesTicker) -> String {
        if let Some(dd) = self.display_cache.get(&ticker) {
            dd.display_ticker.clone()
        } else {
            ticker.as_str().to_string()
        }
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

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
            .style(|theme, status| {
                crate::style::validated_text_input(theme, status, true)
            })
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
    chip_label: Option<&'static str>,
    on_select: FSelect,
) -> Element<'a, M>
where
    M: 'a + Clone,
    FSelect: 'static + Copy + Fn(RowSelection) -> M,
{
    let left_btn_base = button(
        row![text(label)]
            .spacing(tokens::spacing::SM)
            .align_y(alignment::Vertical::Center)
            .height(Length::Fill),
    )
    .style(|theme, status| style::button::transparent(theme, status, false))
    .width(Length::Fill)
    .height(Length::Fill);

    let left_btn = if let Some(sel) = left_action {
        left_btn_base.on_press(on_select(sel))
    } else {
        left_btn_base
    };

    let right_el: Option<Element<'a, M>> =
        right_label_and_action.map(|(lbl, action)| {
            let btn_base = button(
                row![text(lbl).size(tokens::text::SMALL)]
                    .align_y(alignment::Vertical::Center)
                    .height(Length::Fill),
            )
            .style(|theme, status| {
                style::button::transparent(theme, status, false)
            })
            .height(Length::Fill);

            let btn = if let Some(act) = action {
                btn_base.on_press(on_select(act))
            } else {
                btn_base
            };

            btn.into()
        });

    let chip_el: Option<Element<'a, M>> = chip_label.map(|lbl| {
        container(text(lbl).size(tokens::text::SMALL))
            .padding([
                tokens::spacing::XXS as u16,
                tokens::spacing::SM as u16,
            ])
            .style(style::dragger_row_container)
            .into()
    });

    let mut row_content =
        row![left_btn].align_y(alignment::Vertical::Center);

    if let Some(chip) = chip_el {
        row_content = row_content.push(chip);
    }
    if let Some(right) = right_el {
        row_content =
            row_content.push(iced::widget::rule::vertical(1.0));
        row_content = row_content.push(right);
    }

    container(row_content)
        .style(style::ticker_card)
        .height(Length::Fixed(COMPACT_ROW_HEIGHT))
        .width(Length::Fill)
        .into()
}

/// Rank for search matching (lower = better).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SearchRank {
    bucket: u8,
    pos: u16,
    len: u16,
}

fn calc_search_rank(
    row: &TickerRowData,
    query: &str,
) -> Option<SearchRank> {
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
                if (rank.bucket, rank.pos, rank.len)
                    < (cur.bucket, cur.pos, cur.len)
                {
                    rank
                } else {
                    cur
                }
            }
        });
    }

    best
}

fn compute_display_data(
    row: &TickerRowData,
    previous_stats: Option<TickerStats>,
) -> TickerDisplayData {
    let mark_price = row.stats.mark_price;
    let daily_change_pct = row.stats.daily_price_chg;
    let volume = row.stats.daily_volume;

    let has_data = mark_price > 0.0
        || daily_change_pct.abs() > 0.0
        || volume > 0.0;

    let price_str = if has_data {
        format!("{:.2}", mark_price)
    } else {
        "N/A".to_string()
    };

    let (unchanged, changed, direction) = if !has_data {
        (
            price_str.clone(),
            String::new(),
            PriceChangeDirection::Unchanged,
        )
    } else if let Some(prev) = previous_stats {
        if mark_price > prev.mark_price {
            let parts: Vec<&str> = price_str
                .splitn(2, |c: char| !c.is_ascii_digit())
                .collect();
            if parts.len() > 1 {
                (
                    parts[0].to_string(),
                    format!(".{}", parts[1]),
                    PriceChangeDirection::Increased,
                )
            } else {
                (
                    price_str.clone(),
                    String::new(),
                    PriceChangeDirection::Increased,
                )
            }
        } else if mark_price < prev.mark_price {
            let parts: Vec<&str> = price_str
                .splitn(2, |c: char| !c.is_ascii_digit())
                .collect();
            if parts.len() > 1 {
                (
                    parts[0].to_string(),
                    format!(".{}", parts[1]),
                    PriceChangeDirection::Decreased,
                )
            } else {
                (
                    price_str.clone(),
                    String::new(),
                    PriceChangeDirection::Decreased,
                )
            }
        } else {
            (
                price_str.clone(),
                String::new(),
                PriceChangeDirection::Unchanged,
            )
        }
    } else {
        (
            price_str.clone(),
            String::new(),
            PriceChangeDirection::Unchanged,
        )
    };

    let volume_display = if !has_data {
        "N/A".to_string()
    } else if volume >= 1_000_000.0 {
        format!("{:.1}M", volume / 1_000_000.0)
    } else if volume >= 1_000.0 {
        format!("{:.1}K", volume / 1_000.0)
    } else if volume > 0.0 {
        format!("{:.0}", volume)
    } else {
        "0".to_string()
    };

    let daily_change_display = if !has_data {
        "N/A".to_string()
    } else {
        let change_sign = if daily_change_pct > 0.0 { "+" } else { "" };
        format!("{}{:.2}%", change_sign, daily_change_pct)
    };

    let card_color_alpha =
        if has_data && daily_change_pct.abs() > 0.01 {
            0.15
        } else {
            0.0
        };

    TickerDisplayData {
        display_ticker: row
            .ticker
            .display_name()
            .unwrap_or(row.ticker.as_str())
            .to_string(),
        product_name: row.product_name.clone(),
        contract_type: row.contract_type_display.clone(),
        mark_price_display: price_str,
        daily_change_pct: daily_change_display,
        volume_display,
        price_unchanged_part: unchanged,
        price_changed_part: changed,
        price_change_direction: direction,
        card_color_alpha,
    }
}
