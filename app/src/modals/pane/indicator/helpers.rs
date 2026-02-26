//! Badge helpers for indicator manager.

use super::*;

use crate::components::primitives::badge::{BadgeKind, badge};

use iced::Element;

pub(super) fn placement_badge(placement: study::StudyPlacement) -> Element<'static, Message> {
    let label = match placement {
        study::StudyPlacement::Overlay => "Overlay",
        study::StudyPlacement::Panel => "Panel",
        study::StudyPlacement::Background => "Background",
        study::StudyPlacement::CandleReplace => "Candle Replace",
        study::StudyPlacement::SidePanel => "Side Panel",
    };
    badge(label, BadgeKind::Info)
}

pub(super) fn category_badge(category: study::StudyCategory) -> Element<'static, Message> {
    let kind = match category {
        study::StudyCategory::Trend => BadgeKind::Info,
        study::StudyCategory::Momentum => BadgeKind::Warning,
        study::StudyCategory::Volume => BadgeKind::Success,
        study::StudyCategory::Volatility => BadgeKind::Danger,
        study::StudyCategory::OrderFlow => BadgeKind::Default,
        study::StudyCategory::Custom => BadgeKind::Default,
    };
    let label = match category {
        study::StudyCategory::Trend => "Trend",
        study::StudyCategory::Momentum => "Momentum",
        study::StudyCategory::Volume => "Volume",
        study::StudyCategory::Volatility => "Volatility",
        study::StudyCategory::OrderFlow => "Order Flow",
        study::StudyCategory::Custom => "Custom",
    };
    badge(label, kind)
}

/// Format an integer value according to `DisplayFormat`.
pub(crate) fn format_integer(v: i64, fmt: study::DisplayFormat) -> String {
    match fmt {
        study::DisplayFormat::Integer { suffix } => {
            format!("{v}{suffix}")
        }
        study::DisplayFormat::IntegerOrNone { none_value } => {
            if v == none_value {
                "None".to_string()
            } else {
                format!("{v}")
            }
        }
        study::DisplayFormat::Percent => format!("{v}%"),
        study::DisplayFormat::Float { decimals } => {
            format!("{v:.prec$}", prec = decimals as usize)
        }
        study::DisplayFormat::Auto => format!("{v}"),
    }
}

/// Format a float value according to `DisplayFormat`.
pub(crate) fn format_float(v: f32, fmt: study::DisplayFormat) -> String {
    match fmt {
        study::DisplayFormat::Percent => {
            format!("{:.0}%", v * 100.0)
        }
        study::DisplayFormat::Float { decimals } => {
            format!("{v:.prec$}", prec = decimals as usize)
        }
        study::DisplayFormat::Integer { suffix } => {
            format!("{}{suffix}", v as i64)
        }
        study::DisplayFormat::IntegerOrNone { none_value } => {
            let i = v as i64;
            if i == none_value {
                "None".to_string()
            } else {
                format!("{i}")
            }
        }
        study::DisplayFormat::Auto => format!("{v:.2}"),
    }
}
