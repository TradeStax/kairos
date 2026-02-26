//! Styles for non-container Iced widgets: pane grid, scrollable,
//! text input, progress bar, and rules.

use iced::widget::pane_grid::{Highlight, Line};
use iced::widget::scrollable::{AutoScroll, Rail, Scroller};
use iced::{Border, Color, Shadow, Theme, widget};

use crate::style::tokens;

// ── Pane Grid ─────────────────────────────────────────────────────────

pub fn pane_grid(theme: &Theme) -> widget::pane_grid::Style {
    let palette = theme.extended_palette();

    widget::pane_grid::Style {
        hovered_region: Highlight {
            background: palette
                .background
                .strongest
                .color
                .scale_alpha(tokens::alpha::MEDIUM)
                .into(),
            border: Border {
                width: tokens::border::THIN,
                color: palette.background.strongest.color,
                radius: tokens::radius::MD.into(),
            },
        },
        picked_split: Line {
            color: palette.primary.strong.color,
            width: 4.0,
        },
        hovered_split: Line {
            color: palette.primary.weak.color,
            width: 4.0,
        },
    }
}

// ── Scrollable ────────────────────────────────────────────────────────

pub fn scroll_bar(theme: &Theme, status: widget::scrollable::Status) -> widget::scrollable::Style {
    let palette = theme.extended_palette();

    let (rail_bg, scroller_bg) = match status {
        widget::scrollable::Status::Hovered { .. } | widget::scrollable::Status::Dragged { .. } => {
            (
                palette.background.weakest.color,
                palette.background.weak.color,
            )
        }
        _ => (
            palette.background.base.color,
            palette.background.weakest.color,
        ),
    };

    let rail = Rail {
        background: Some(iced::Background::Color(rail_bg)),
        border: Border {
            radius: tokens::radius::SM.into(),
            width: tokens::border::THIN,
            color: Color::TRANSPARENT,
        },
        scroller: Scroller {
            background: iced::Background::Color(scroller_bg),
            border: Border {
                radius: tokens::radius::SM.into(),
                width: tokens::border::NONE,
                color: Color::TRANSPARENT,
            },
        },
    };

    let auto_scroll = AutoScroll {
        background: iced::Background::Color(palette.background.weakest.color),
        border: Border {
            radius: tokens::radius::SM.into(),
            width: tokens::border::THIN,
            color: Color::TRANSPARENT,
        },
        shadow: Shadow {
            color: Color::TRANSPARENT,
            ..Default::default()
        },
        icon: palette.background.strong.color,
    };

    widget::scrollable::Style {
        container: widget::container::Style {
            ..Default::default()
        },
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        auto_scroll,
    }
}

// ── Text Input ────────────────────────────────────────────────────────

pub fn validated_text_input(
    theme: &Theme,
    status: widget::text_input::Status,
    is_valid: bool,
) -> widget::text_input::Style {
    let palette = theme.extended_palette();

    let (background, border_color, placeholder) = match status {
        widget::text_input::Status::Active => (
            palette.background.weakest.color,
            palette.background.weak.color,
            palette.background.strongest.color,
        ),
        widget::text_input::Status::Hovered => (
            palette.background.weak.color,
            palette.background.strong.color,
            palette.background.weak.text,
        ),
        widget::text_input::Status::Focused { .. } | widget::text_input::Status::Disabled => (
            palette.background.base.color,
            palette.background.strong.color,
            palette.background.strong.color,
        ),
    };

    widget::text_input::Style {
        background: background.into(),
        border: Border {
            radius: tokens::radius::MD.into(),
            width: tokens::border::THIN,
            color: if is_valid {
                border_color
            } else {
                palette.danger.base.color
            },
        },
        icon: palette.background.strong.text,
        placeholder,
        value: palette.background.base.text,
        selection: palette.background.strongest.color,
    }
}

// ── Progress Bar ──────────────────────────────────────────────────────

pub fn progress_bar(theme: &Theme) -> widget::progress_bar::Style {
    let palette = theme.extended_palette();

    widget::progress_bar::Style {
        background: palette.background.weak.color.into(),
        bar: palette.primary.base.color.into(),
        border: Border {
            width: tokens::border::NONE,
            color: Color::TRANSPARENT,
            radius: tokens::radius::MD.into(),
        },
    }
}

// ── Rule ──────────────────────────────────────────────────────────────

pub fn split_ruler(theme: &Theme) -> iced::widget::rule::Style {
    let palette = theme.extended_palette();

    iced::widget::rule::Style {
        color: palette
            .background
            .strong
            .color
            .scale_alpha(tokens::alpha::SUBTLE),
        radius: iced::border::Radius::default(),
        fill_mode: iced::widget::rule::FillMode::Full,
        snap: true,
    }
}
