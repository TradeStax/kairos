use exchange::FuturesTickerInfo;
use iced::Element;

use crate::screen::dashboard::tickers_table::TickersTable;

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
        table: &'a TickersTable,
        selected_tickers: Option<&'a [FuturesTickerInfo]>,
        base_ticker: Option<FuturesTickerInfo>,
    ) -> Element<'a, Message> {
        table.view_compact_with(
            &self.search_query,
            &self.search_box_id,
            Message::RowSelected,
            Message::SearchChanged,
            selected_tickers,
            base_ticker,
        )
    }
}
