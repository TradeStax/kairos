use iced::{
    Alignment, Element, Length, padding,
    widget::{container, mouse_area, opaque},
};

pub mod calendar;
pub mod connections_menu;
pub mod data_feeds;
pub mod data_management;
pub mod historical_download;
pub mod indicators;
pub mod mini_tickers_list;
pub mod settings;
pub mod stream;

/// Futures products for ticker dropdown (shared by data_management
/// and historical_download)
pub const FUTURES_PRODUCTS: &[(&str, &str)] = &[
    ("ES.c.0", "E-mini S&P 500"),
    ("NQ.c.0", "E-mini Nasdaq-100"),
    ("YM.c.0", "E-mini Dow"),
    ("RTY.c.0", "E-mini Russell 2000"),
    ("CL.c.0", "Crude Oil"),
    ("GC.c.0", "Gold"),
    ("SI.c.0", "Silver"),
    ("ZN.c.0", "10-Year T-Note"),
    ("ZB.c.0", "30-Year T-Bond"),
    ("ZF.c.0", "5-Year T-Note"),
    ("NG.c.0", "Natural Gas"),
    ("HG.c.0", "Copper"),
];

/// Schemas with display names and cost rating
pub const SCHEMAS: &[(exchange::DatabentoSchema, &str, u8)] = &[
    (exchange::DatabentoSchema::Trades, "Trades", 2),
    (exchange::DatabentoSchema::Mbp10, "MBP-10 (10 Levels)", 3),
    (exchange::DatabentoSchema::Mbp1, "MBP-1 (Top of Book)", 2),
    (exchange::DatabentoSchema::Ohlcv1M, "OHLCV-1M", 1),
    (exchange::DatabentoSchema::Tbbo, "TBBO (Top BBO)", 2),
    (exchange::DatabentoSchema::Mbo, "MBO (VERY EXPENSIVE)", 10),
];

#[derive(Debug, Clone, PartialEq)]
pub enum Modal {
    StreamModifier(super::stream::Modifier),
    MiniTickersList(mini_tickers_list::MiniPanel),
    DataManagement(data_management::DataManagementPanel),
    Settings,
    Indicators,
    LinkGroup,
    Controls,
}

pub fn stack_modal<'a, Message>(
    base: impl Into<Element<'a, Message>>,
    content: impl Into<Element<'a, Message>>,
    on_blur: Message,
    padding: padding::Padding,
    alignment: Alignment,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    iced::widget::stack![
        base.into(),
        mouse_area(
            container(opaque(content))
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(padding)
                .align_x(alignment)
        )
        .on_press(on_blur)
    ]
    .into()
}
