//! Renders a `LoadingStatus` enum as a descriptive text element.

use iced::widget::text;
use iced::{Element, Renderer, Theme};

use crate::style::tokens;

/// Render a `data::LoadingStatus` as a compact text element.
pub fn loading_status_display<'a, Message: 'a>(
    status: &data::LoadingStatus,
) -> Element<'a, Message, Theme, Renderer> {
    let label = match status {
        data::LoadingStatus::Idle => "Idle".to_string(),
        data::LoadingStatus::Downloading {
            days_total,
            days_complete,
            current_day,
            ..
        } => {
            format!("Downloading {days_complete}/{days_total} ({current_day})")
        }
        data::LoadingStatus::LoadingFromCache {
            days_total,
            days_loaded,
            items_loaded,
            ..
        } => {
            format!("Loading cache {days_loaded}/{days_total} ({items_loaded} items)")
        }
        data::LoadingStatus::Building {
            operation,
            progress,
        } => {
            let pct = (*progress * 100.0) as u32;
            format!("{operation} {pct}%")
        }
        data::LoadingStatus::Ready => "Ready".to_string(),
        data::LoadingStatus::Error { message } => {
            format!("Error: {message}")
        }
    };

    text(label).size(tokens::text::SMALL).into()
}
