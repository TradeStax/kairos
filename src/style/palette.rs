use iced::Color;

use data::feed::FeedStatus;

// ── Semantic colors ─────────────────────────────────────────────────
// These are the single source of truth for recurring UI colors.
// Chart-domain colors (candle bodies, heatmap gradients, indicator lines)
// live in `src/chart/` and are intentionally kept separate.

/// Green -- success, "connected", buy side.
pub fn success_color() -> Color {
    Color::from_rgb(0.2, 0.8, 0.2)
}

/// Red -- error, sell side.
pub fn error_color() -> Color {
    Color::from_rgb(0.9, 0.2, 0.2)
}

/// Amber -- warning, "disconnected but has data".
pub fn warning_color() -> Color {
    Color::from_rgb(0.7, 0.5, 0.2)
}

/// Blue -- informational, downloading, dataset indicator.
pub fn info_color() -> Color {
    Color::from_rgb(0.3, 0.6, 1.0)
}

/// Neutral gray.
pub fn neutral_color() -> Color {
    Color::from_rgb(0.5, 0.5, 0.5)
}

// ── Feed status ─────────────────────────────────────────────────────

/// Connection status colors -- single source of truth.
/// Previously hardcoded in connections_menu.rs and data_feeds.rs.
pub fn status_color(status: &FeedStatus) -> Color {
    match status {
        FeedStatus::Connected => success_color(),
        FeedStatus::Connecting => Color::from_rgb(0.9, 0.7, 0.1),
        FeedStatus::Downloading { .. } => info_color(),
        FeedStatus::Error(_) => error_color(),
        FeedStatus::Disconnected => neutral_color(),
    }
}
