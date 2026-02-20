//! Badge helpers for indicator manager.

use super::*;

use crate::components::primitives::badge::{BadgeKind, badge};

use iced::Element;

pub(super) fn placement_badge(
    placement: study::StudyPlacement,
) -> Element<'static, Message> {
    let label = match placement {
        study::StudyPlacement::Overlay => "Overlay",
        study::StudyPlacement::Panel => "Panel",
        study::StudyPlacement::Background => "Background",
    };
    badge(label, BadgeKind::Info)
}

pub(super) fn category_badge(
    category: study::StudyCategory,
) -> Element<'static, Message> {
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
