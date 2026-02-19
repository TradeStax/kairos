use crate::component::primitives::label::label_text;
use crate::style::tokens;
use crate::component::primitives::{Icon, icon_text};
use crate::style;

use data::ChartBasis;
use exchange::{FuturesTickerInfo, FuturesVenue, Timeframe};
use iced::{
    Element, Length,
    alignment::Horizontal,
    widget::{button, column, container, row, rule, scrollable, text},
};
use serde::{Deserialize, Serialize};

const NUMERIC_INPUT_BUF_SIZE: usize = 5; // Max 5 digits for u16 (65535)

const TICK_COUNT_MIN: u16 = 4;
const TICK_COUNT_MAX: u16 = 1000;

// TickMultiplier removed - only needed for crypto, not futures
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub enum ModifierKind {
    Candlestick(ChartBasis),
    Footprint(ChartBasis),
    Heatmap(ChartBasis),
    Orderbook(ChartBasis),
    Comparison(ChartBasis),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct NumericInput {
    buffer: [u8; NUMERIC_INPUT_BUF_SIZE],
    len: u8,
}

impl NumericInput {
    pub fn new() -> Self {
        Self {
            buffer: [0; NUMERIC_INPUT_BUF_SIZE],
            len: 0,
        }
    }

    pub fn from_str(s: &str) -> Self {
        let mut buffer = [0; NUMERIC_INPUT_BUF_SIZE];
        let bytes = s.as_bytes();
        let len = bytes.len().min(NUMERIC_INPUT_BUF_SIZE);
        buffer[..len].copy_from_slice(&bytes[..len]);
        Self {
            buffer,
            len: len as u8,
        }
    }

    pub fn from_tick_count(tc: u16) -> Self {
        Self::from_str(&tc.to_string())
    }

    pub fn to_display_string(self) -> String {
        if self.len == 0 {
            return String::new();
        }
        String::from_utf8_lossy(&self.buffer[..self.len as usize]).into_owned()
    }

    pub fn is_empty(self) -> bool {
        self.len == 0
    }

    pub fn parse_tick_count(self) -> Option<u16> {
        if self.len == 0 {
            return None;
        }
        std::str::from_utf8(&self.buffer[..self.len as usize])
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
    }
}

impl Default for NumericInput {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ViewMode {
    BasisSelection,
    // TicksizeSelection removed - tick multiplier only needed for crypto
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SelectedTab {
    Timeframe,
    TickCount {
        raw_input_buf: NumericInput,
        parsed_input: Option<u16>,
        is_input_valid: bool,
    },
}

pub enum Action {
    BasisSelected(ChartBasis),
    TabSelected(SelectedTab),
}

#[derive(Debug, Clone)]
pub enum Message {
    BasisSelected(ChartBasis),
    TabSelected(SelectedTab),
    TickCountInputChanged(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Modifier {
    pub tab: SelectedTab,
    pub view_mode: ViewMode,
    kind: ModifierKind,
    base_ticksize: Option<f32>,
    exchange: Option<FuturesVenue>,
}

impl Modifier {
    pub fn new(kind: ModifierKind) -> Self {
        let tab = SelectedTab::from(&kind);

        Self {
            tab,
            kind,
            view_mode: ViewMode::BasisSelection,
            base_ticksize: None,
            exchange: None,
        }
    }

    pub fn with_view_mode(mut self, view_mode: ViewMode) -> Self {
        self.view_mode = view_mode;
        self
    }

    // with_ticksize_view removed - tick multiplier only for crypto

    pub fn update_kind_with_basis(&mut self, basis: ChartBasis) {
        match self.kind {
            ModifierKind::Candlestick(_) => self.kind = ModifierKind::Candlestick(basis),
            ModifierKind::Comparison(_) => {
                self.kind = ModifierKind::Comparison(basis);
            }
            ModifierKind::Footprint(_) => {
                self.kind = ModifierKind::Footprint(basis);
            }
            ModifierKind::Heatmap(_) => {
                self.kind = ModifierKind::Heatmap(basis);
            }
            ModifierKind::Orderbook(_) => {
                self.kind = ModifierKind::Orderbook(basis);
            }
        }
    }

    // update_kind_with_multiplier removed - tick multiplier only for crypto

    pub fn update(&mut self, message: Message) -> Option<Action> {
        match message {
            Message::TabSelected(tab) => Some(Action::TabSelected(tab)),
            Message::BasisSelected(basis) => match basis {
                ChartBasis::Time(_) => Some(Action::BasisSelected(basis)),
                ChartBasis::Tick(new_tc) => {
                    if let SelectedTab::TickCount {
                        raw_input_buf,
                        parsed_input,
                        is_input_valid,
                    } = &mut self.tab
                    {
                        // new_tc is u32, parsed_input is Option<u16> - compare by converting u32 to u16
                        if *parsed_input == Some(new_tc as u16) {
                            *is_input_valid = true;
                        } else {
                            *raw_input_buf = NumericInput::default();
                            *parsed_input = None;
                            *is_input_valid = true;
                        };

                        Some(Action::BasisSelected(basis))
                    } else {
                        None
                    }
                }
            },
            Message::TickCountInputChanged(value_str) => {
                if let SelectedTab::TickCount {
                    ref mut raw_input_buf,
                    ref mut parsed_input,
                    ref mut is_input_valid,
                } = self.tab
                {
                    let numeric_value_str: String =
                        value_str.chars().filter(char::is_ascii_digit).collect();

                    *raw_input_buf = NumericInput::from_str(&numeric_value_str);
                    *parsed_input = raw_input_buf.parse_tick_count();

                    if raw_input_buf.is_empty() {
                        *is_input_valid = true;
                    } else {
                        match parsed_input {
                            Some(tc) => {
                                // tc is u16, not a newtype - compare directly
                                *is_input_valid = *tc >= TICK_COUNT_MIN && *tc <= TICK_COUNT_MAX;
                            }
                            None => {
                                *is_input_valid = false;
                            }
                        }
                    }
                }
                None
            }
        }
    }

    pub fn view<'a>(&self, ticker_info: Option<FuturesTickerInfo>) -> Element<'a, Message> {
        let kind = self.kind;

        let selected_basis = match kind {
            ModifierKind::Candlestick(basis)
            | ModifierKind::Comparison(basis)
            | ModifierKind::Footprint(basis)
            | ModifierKind::Heatmap(basis)
            | ModifierKind::Orderbook(basis) => Some(basis),
        };

        let create_button = |content: iced::widget::text::Text<'a>,
                             msg: Option<Message>,
                             is_selected: bool| {
            let btn = button(content.align_x(iced::Alignment::Center))
                .width(Length::Fill)
                .style(move |theme, status| style::button::menu_body(theme, status, is_selected));

            if let Some(msg) = msg {
                btn.on_press(msg)
            } else {
                btn
            }
        };

        match self.view_mode {
            ViewMode::BasisSelection => {
                let mut basis_selection_column =
                    column![].padding(tokens::spacing::XS).spacing(tokens::spacing::MD).align_x(Horizontal::Center);

                let allows_tick_basis = match kind {
                    ModifierKind::Candlestick(_) | ModifierKind::Footprint(_) => true,
                    ModifierKind::Heatmap(_)
                    | ModifierKind::Orderbook(_)
                    | ModifierKind::Comparison(_) => false,
                };

                if selected_basis.is_some() {
                    let (timeframe_tab_is_selected, tick_count_tab_is_selected) = match self.tab {
                        SelectedTab::Timeframe => (true, false),
                        SelectedTab::TickCount { .. } => (false, true),
                    };

                    let tabs_row = {
                        if allows_tick_basis {
                            let is_timeframe_selected =
                                matches!(selected_basis, Some(ChartBasis::Time(_)));

                            let tab_button =
                                |content: iced::widget::text::Text<'a>,
                                 msg: Option<Message>,
                                 active: bool,
                                 checkmark: bool| {
                                    let content = if checkmark {
                                        row![
                                            content,
                                            iced::widget::space::horizontal(),
                                            icon_text(Icon::Checkmark, 12)
                                        ]
                                    } else {
                                        row![content]
                                    }
                                    .width(Length::Fill);

                                    let btn = button(content).style(move |theme, status| {
                                        style::button::transparent(theme, status, active)
                                    });

                                    if let Some(msg) = msg {
                                        btn.on_press(msg)
                                    } else {
                                        btn
                                    }
                                };

                            row![
                                tab_button(
                                    text("Timeframe"),
                                    if timeframe_tab_is_selected {
                                        None
                                    } else {
                                        Some(Message::TabSelected(SelectedTab::Timeframe))
                                    },
                                    !timeframe_tab_is_selected,
                                    is_timeframe_selected,
                                ),
                                tab_button(
                                    text("Ticks"),
                                    if tick_count_tab_is_selected {
                                        None
                                    } else {
                                        let tick_count_tab = match self.tab {
                                            SelectedTab::TickCount {
                                                raw_input_buf,
                                                parsed_input,
                                                is_input_valid,
                                            } => SelectedTab::TickCount {
                                                raw_input_buf,
                                                parsed_input,
                                                is_input_valid,
                                            },
                                            _ => SelectedTab::TickCount {
                                                raw_input_buf: NumericInput::default(),
                                                parsed_input: None,
                                                is_input_valid: true,
                                            },
                                        };
                                        Some(Message::TabSelected(tick_count_tab))
                                    },
                                    !tick_count_tab_is_selected,
                                    !is_timeframe_selected,
                                ),
                            ]
                            .spacing(tokens::spacing::XS)
                        } else {
                            let text_content = match kind {
                                ModifierKind::Comparison(_) => "Timeframe",
                                _ => "Aggregation",
                            };
                            row![label_text(text_content)]
                        }
                    };

                    basis_selection_column = basis_selection_column
                        .push(tabs_row)
                        .push(rule::horizontal(1).style(style::split_ruler));
                }

                match self.tab {
                    SelectedTab::Timeframe => {
                        let selected_tf = match selected_basis {
                            Some(ChartBasis::Time(tf)) => Some(tf),
                            _ => None,
                        };

                        if allows_tick_basis {
                            let kline_timeframe_grid = modifiers_grid(
                                &Timeframe::KLINE,
                                selected_tf,
                                |tf| Message::BasisSelected(tf.into()),
                                &create_button,
                                3,
                            );
                            basis_selection_column =
                                basis_selection_column.push(kline_timeframe_grid);
                        } else if let Some(_info) = ticker_info {
                            match kind {
                                ModifierKind::Comparison(_) => {
                                    let kline_timeframe_grid = modifiers_grid(
                                        &Timeframe::KLINE,
                                        selected_tf,
                                        |tf| Message::BasisSelected(tf.into()),
                                        &create_button,
                                        3,
                                    );
                                    basis_selection_column =
                                        basis_selection_column.push(kline_timeframe_grid);
                                }
                                ModifierKind::Heatmap(_) => {
                                    // All heatmap timeframes are supported for futures
                                    let heatmap_timeframes: Vec<Timeframe> =
                                        Timeframe::HEATMAP.to_vec();
                                    let heatmap_timeframe_grid = modifiers_grid(
                                        &heatmap_timeframes,
                                        selected_tf,
                                        |tf| Message::BasisSelected(tf.into()),
                                        &create_button,
                                        2,
                                    );
                                    basis_selection_column =
                                        basis_selection_column.push(heatmap_timeframe_grid);
                                }
                                _ => { /* No other chart types support non-time basis */ }
                            }
                        }
                    }
                    SelectedTab::TickCount {
                        raw_input_buf,
                        parsed_input,
                        is_input_valid,
                    } => {
                        let selected_tick_count = match selected_basis {
                            Some(ChartBasis::Tick(tc)) => Some(tc),
                            _ => None,
                        };

                        // Define standard tick counts
                        const TICK_COUNTS: [u16; 8] = [10, 25, 50, 100, 250, 500, 1000, 2500];

                        let tick_count_grid = modifiers_grid(
                            &TICK_COUNTS,
                            selected_tick_count.map(|tc| tc as u16),
                            |tc| Message::BasisSelected(ChartBasis::Tick(tc as u32)),
                            &create_button,
                            3,
                        );

                        let custom_input: Element<'_, Message> = {
                            let tick_count_to_submit = parsed_input
                                .filter(|tc| *tc >= TICK_COUNT_MIN && *tc <= TICK_COUNT_MAX);

                            let mut input = iced::widget::text_input(
                                &format!("{}-{}", TICK_COUNT_MIN, TICK_COUNT_MAX),
                                &raw_input_buf.to_display_string(),
                            )
                            .on_input(Message::TickCountInputChanged)
                            .align_x(iced::Alignment::Center)
                            .style(move |theme, status| {
                                style::validated_text_input(theme, status, is_input_valid)
                            });
                            if let Some(tc) = tick_count_to_submit {
                                input = input.on_submit(
                                    Message::BasisSelected(ChartBasis::Tick(tc as u32)),
                                );
                            }

                            row![label_text("Custom: "), input]
                                .spacing(tokens::spacing::XS)
                                .align_y(iced::Alignment::Center)
                                .into()
                        };

                        basis_selection_column = basis_selection_column.push(custom_input);
                        basis_selection_column = basis_selection_column.push(tick_count_grid);
                    }
                }

                container(scrollable::Scrollable::with_direction(
                    basis_selection_column,
                    scrollable::Direction::Vertical(
                        scrollable::Scrollbar::new().width(4).scroller_width(4),
                    ),
                ))
                .max_width(240)
                .padding(tokens::spacing::XL)
                .style(style::chart_modal)
                .into()
            }
            // TicksizeSelection view removed - tick multiplier only for crypto
        }
    }
}

/// A `Column` grid of buttons from `items_source`.
///
/// Buttons are arranged in rows of up to `items_per_row`.
/// If the last row would otherwise contain only one item,
/// one item is shifted from the previous row so that no row ends up
/// with a single button.
fn modifiers_grid<'a, T, FMsg>(
    items_source: &[T],
    selected_value: Option<T>,
    to_message: FMsg,
    create_button_fn: &impl Fn(
        iced::widget::text::Text<'a>,
        Option<Message>,
        bool,
    ) -> iced::widget::Button<'a, Message>,
    items_per_row: usize,
) -> iced::widget::Column<'a, Message>
where
    T: Copy + PartialEq + ToString,
    FMsg: Fn(T) -> Message,
{
    let mut grid_column = column![].spacing(tokens::spacing::XS);
    let mut remaining_slice = items_source;

    while !remaining_slice.is_empty() {
        let count = remaining_slice.len();
        let mut take = items_per_row;

        let rows_left = count.div_ceil(items_per_row);
        let last_row_size = count % items_per_row;

        if rows_left == 2 && last_row_size == 1 {
            take -= 1;
        }

        take = take.min(count);
        let (chunk, rest) = remaining_slice.split_at(take);
        remaining_slice = rest;

        let mut button_row = row![].spacing(tokens::spacing::XS);

        for &item_value in chunk {
            let is_selected = selected_value == Some(item_value);
            let msg = if is_selected {
                None
            } else {
                Some(to_message(item_value))
            };
            button_row = button_row.push(create_button_fn(
                text(item_value.to_string()),
                msg,
                is_selected,
            ));
        }

        grid_column = grid_column.push(button_row);
    }

    grid_column
}

impl From<&ModifierKind> for SelectedTab {
    fn from(kind: &ModifierKind) -> Self {
        match kind {
            ModifierKind::Candlestick(basis)
            | ModifierKind::Footprint(basis)
            | ModifierKind::Heatmap(basis)
            | ModifierKind::Orderbook(basis)
            | ModifierKind::Comparison(basis) => match basis {
                ChartBasis::Time(_) => SelectedTab::Timeframe,
                ChartBasis::Tick(tc) => {
                    const TICK_COUNTS: [u16; 8] = [10, 25, 50, 100, 250, 500, 1000, 2500];
                    // tc is u32, convert to u16 for comparison
                    let tc_u16 = *tc as u16;
                    let is_custom = !TICK_COUNTS.contains(&tc_u16);
                    SelectedTab::TickCount {
                        raw_input_buf: if is_custom {
                            NumericInput::from_tick_count(tc_u16)
                        } else {
                            NumericInput::default()
                        },
                        parsed_input: if is_custom { Some(tc_u16) } else { None },
                        is_input_valid: true,
                    }
                }
            },
        }
    }
}
