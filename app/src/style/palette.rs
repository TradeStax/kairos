use iced::widget::text;
use iced::{Color, Theme};

// ── Semantic colors ───────────────────────────────────────────────────
// These are the single source of truth for recurring UI colors.
// Chart-domain colors (candle bodies, heatmap gradients, indicator lines)
// live in `src/chart/` and are intentionally kept separate.
//
// All functions derive from the active Iced theme's extended palette,
// so colors automatically adapt when the user switches themes.

/// Green -- success, "connected", buy side.
pub fn success_color(theme: &Theme) -> Color {
    theme.extended_palette().success.base.color
}

/// Red -- error, sell side.
pub fn error_color(theme: &Theme) -> Color {
    theme.extended_palette().danger.base.color
}

/// Amber -- warning, "disconnected but has data".
pub fn warning_color(theme: &Theme) -> Color {
    theme.extended_palette().warning.base.color
}

/// Blue -- informational, downloading, dataset indicator.
pub fn info_color(theme: &Theme) -> Color {
    theme.extended_palette().primary.base.color
}

/// Neutral gray -- secondary/muted text.
pub fn neutral_color(theme: &Theme) -> Color {
    theme.extended_palette().secondary.weak.color
}

// ── Text style helpers ───────────────────────────────────────────────
// For use with `text.style(palette::success_text)` in builder contexts
// where `&Theme` is not directly available.

pub fn success_text(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(success_color(theme)),
    }
}

pub fn info_text(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(info_color(theme)),
    }
}

pub fn error_text(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(error_color(theme)),
    }
}

pub fn warning_text(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(warning_color(theme)),
    }
}

pub fn neutral_text(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(neutral_color(theme)),
    }
}

