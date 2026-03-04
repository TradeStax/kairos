//! Button styles. Generic variants first, then domain-specific.

use iced::{
    Border, Theme,
    widget::button::{Status, Style},
};

use crate::style::tokens;

// ── Generic ───────────────────────────────────────────────────────────
// Used across many components — the standard button "variants".

pub fn primary(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: palette.primary.base.text,
        background: match status {
            Status::Hovered => Some(palette.primary.strong.color.into()),
            Status::Pressed => Some(palette.primary.weak.color.into()),
            Status::Disabled => Some(palette.background.weak.color.into()),
            Status::Active => Some(palette.primary.base.color.into()),
        },
        border: Border {
            radius: tokens::radius::MD.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn secondary(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: palette.background.base.text,
        background: match status {
            Status::Hovered => Some(palette.background.strong.color.into()),
            Status::Pressed => Some(palette.background.strongest.color.into()),
            Status::Disabled => Some(palette.background.weakest.color.into()),
            Status::Active => Some(palette.background.weak.color.into()),
        },
        border: Border {
            radius: tokens::radius::MD.into(),
            width: tokens::border::THIN,
            color: palette.background.strong.color,
        },
        ..Default::default()
    }
}

pub fn danger(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: match status {
            Status::Disabled => palette.background.weak.text,
            _ => palette.danger.base.text,
        },
        background: match status {
            Status::Hovered => Some(palette.danger.strong.color.into()),
            Status::Pressed => Some(palette.danger.weak.color.into()),
            Status::Disabled => Some(palette.background.weak.color.into()),
            Status::Active => Some(palette.danger.base.color.into()),
        },
        border: Border {
            radius: tokens::radius::MD.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn transparent(theme: &Theme, status: Status, is_clicked: bool) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: palette.background.base.text,
        border: Border {
            radius: tokens::radius::MD.into(),
            ..Default::default()
        },
        background: match status {
            Status::Active => {
                if is_clicked {
                    Some(palette.background.weak.color.into())
                } else {
                    None
                }
            }
            Status::Pressed => Some(palette.background.weak.color.into()),
            Status::Hovered => Some(palette.background.strong.color.into()),
            Status::Disabled => {
                if is_clicked {
                    Some(palette.background.strongest.color.into())
                } else {
                    Some(palette.background.strong.color.into())
                }
            }
        },
        ..Default::default()
    }
}

pub fn info(theme: &Theme, _status: Status) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: palette.background.base.text,
        border: Border {
            radius: tokens::radius::MD.into(),
            ..Default::default()
        },
        background: Some(palette.background.weakest.color.into()),
        ..Default::default()
    }
}

/// Transparent button with subtle hover highlight for list items
pub fn list_item(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: palette.background.base.text,
        background: match status {
            Status::Hovered => Some(palette.background.weak.color.into()),
            Status::Pressed => Some(palette.background.strong.color.into()),
            Status::Active | Status::Disabled => None,
        },
        border: Border {
            radius: tokens::radius::MD.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// List item with optional selected highlight
pub fn list_item_selected(theme: &Theme, status: Status, is_selected: bool) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: palette.background.base.text,
        background: match status {
            Status::Hovered => Some(palette.background.weak.color.into()),
            Status::Pressed => Some(palette.background.strong.color.into()),
            Status::Active | Status::Disabled => {
                if is_selected {
                    Some(palette.background.weak.color.into())
                } else {
                    None
                }
            }
        },
        border: Border {
            radius: tokens::radius::MD.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

// ── Toggles ───────────────────────────────────────────────────────────

pub fn bordered_toggle(theme: &Theme, status: Status, is_active: bool) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: if is_active {
            palette.secondary.strong.color
        } else {
            palette.secondary.base.color
        },
        border: Border {
            radius: tokens::radius::MD.into(),
            width: if is_active {
                tokens::border::THICK
            } else {
                tokens::border::THIN
            },
            color: palette.background.weak.color,
        },
        background: match status {
            Status::Active => {
                if is_active {
                    Some(palette.background.base.color.into())
                } else {
                    Some(palette.background.weakest.color.into())
                }
            }
            Status::Pressed => Some(palette.background.weakest.color.into()),
            Status::Hovered => Some(palette.background.weak.color.into()),
            Status::Disabled => {
                if is_active {
                    None
                } else {
                    Some(palette.secondary.base.color.into())
                }
            }
        },
        ..Default::default()
    }
}

pub fn modifier(theme: &Theme, status: Status, is_clicked: bool) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: palette.background.base.text,
        border: Border {
            radius: tokens::radius::MD.into(),
            ..Default::default()
        },
        background: match status {
            Status::Active => {
                if is_clicked {
                    Some(palette.background.weak.color.into())
                } else {
                    Some(palette.background.base.color.into())
                }
            }
            Status::Pressed => Some(palette.background.strongest.color.into()),
            Status::Hovered => Some(palette.background.strong.color.into()),
            Status::Disabled => {
                if is_clicked {
                    None
                } else {
                    Some(palette.secondary.weak.color.into())
                }
            }
        },
        ..Default::default()
    }
}

pub fn tab_active(theme: &Theme, _status: Status) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: palette.primary.base.text,
        background: Some(palette.primary.base.color.into()),
        border: Border {
            radius: tokens::radius::MD.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn tab_inactive(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: palette.background.base.text,
        background: match status {
            Status::Hovered => Some(palette.background.strong.color.into()),
            _ => None,
        },
        border: Border {
            radius: tokens::radius::MD.into(),
            width: tokens::border::NONE,
            color: iced::Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

// ── Menus & Lists ─────────────────────────────────────────────────────

#[allow(dead_code)]
pub fn menu_bar_item(theme: &Theme, status: Status, is_open: bool) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: palette.background.base.text,
        background: match status {
            _ if is_open => Some(palette.background.strong.color.into()),
            Status::Hovered => Some(palette.background.weak.color.into()),
            Status::Pressed => Some(palette.background.strong.color.into()),
            Status::Active | Status::Disabled => None,
        },
        border: Border {
            radius: tokens::radius::SM.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn menu_body(theme: &Theme, status: Status, is_selected: bool) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: palette.background.base.text,
        border: Border {
            radius: tokens::radius::MD.into(),
            width: if is_selected {
                tokens::border::THICK
            } else {
                tokens::border::NONE
            },
            color: palette.background.strong.color,
        },
        background: match status {
            Status::Active => {
                if is_selected {
                    Some(palette.background.base.color.into())
                } else {
                    Some(palette.background.weakest.color.into())
                }
            }
            Status::Pressed => Some(palette.background.base.color.into()),
            Status::Hovered => Some(palette.background.weak.color.into()),
            Status::Disabled => {
                if is_selected {
                    None
                } else {
                    Some(palette.secondary.base.color.into())
                }
            }
        },
        ..Default::default()
    }
}

/// Item inside a dropdown menu (pick_list-like)
pub fn pick_list_item(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: palette.background.base.text,
        background: match status {
            Status::Hovered => Some(palette.primary.weak.color.into()),
            Status::Pressed => Some(palette.primary.base.color.into()),
            _ => None,
        },
        border: Border {
            radius: tokens::radius::NONE.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

// ── Domain-specific ───────────────────────────────────────────────────

pub fn confirm(theme: &Theme, status: Status, is_active: bool) -> Style {
    let palette = theme.extended_palette();

    let color_alpha = if palette.is_dark {
        tokens::alpha::FAINT
    } else {
        tokens::alpha::STRONG
    };

    Style {
        text_color: match status {
            Status::Active => palette.success.base.color,
            Status::Pressed => palette.success.weak.color,
            Status::Hovered => palette.success.strong.color,
            Status::Disabled => palette.background.base.text,
        },
        background: match (status, is_active) {
            (Status::Disabled, false) => {
                Some(palette.success.weak.color.scale_alpha(color_alpha).into())
            }
            _ => None,
        },
        border: Border {
            radius: tokens::radius::MD.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn cancel(theme: &Theme, status: Status, is_active: bool) -> Style {
    let palette = theme.extended_palette();

    let color_alpha = if palette.is_dark {
        tokens::alpha::FAINT
    } else {
        tokens::alpha::STRONG
    };

    Style {
        text_color: match status {
            Status::Active => palette.danger.base.color,
            Status::Pressed => palette.danger.weak.color,
            Status::Hovered => palette.danger.strong.color,
            Status::Disabled => palette.background.base.text,
        },
        background: match (status, is_active) {
            (Status::Disabled, false) => {
                Some(palette.danger.weak.color.scale_alpha(color_alpha).into())
            }
            _ => None,
        },
        border: Border {
            radius: tokens::radius::MD.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

// ── Chat Bubbles ─────────────────────────────────────────────────────

/// Assistant message bubble with subtle hover dimming.
pub fn chat_bubble(theme: &Theme, status: Status) -> Style {
    let p = theme.extended_palette();

    Style {
        text_color: p.background.weak.text,
        background: Some(match status {
            Status::Hovered => p
                .background
                .weak
                .color
                .scale_alpha(tokens::alpha::HOVER_DIM)
                .into(),
            _ => p.background.weak.color.into(),
        }),
        border: Border {
            radius: tokens::radius::MD.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// User message bubble with subtle hover dimming.
pub fn chat_bubble_user(theme: &Theme, status: Status) -> Style {
    let p = theme.extended_palette();

    Style {
        text_color: p.primary.weak.text,
        background: Some(match status {
            Status::Hovered => p
                .primary
                .weak
                .color
                .scale_alpha(tokens::alpha::HOVER_DIM)
                .into(),
            _ => p.primary.weak.color.into(),
        }),
        border: Border {
            radius: tokens::radius::MD.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

// ── Window Controls ───────────────────────────────────────────────────

#[allow(dead_code)]
pub fn window_control(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: palette.background.base.text,
        background: match status {
            Status::Hovered => Some(palette.background.strong.color.into()),
            Status::Pressed => Some(palette.background.strongest.color.into()),
            Status::Active | Status::Disabled => None,
        },
        border: Border {
            radius: tokens::radius::SM.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[allow(dead_code)]
pub fn window_close(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: match status {
            Status::Hovered | Status::Pressed => iced::Color::WHITE,
            _ => palette.background.base.text,
        },
        background: match status {
            Status::Hovered => Some(palette.danger.base.color.into()),
            Status::Pressed => Some(palette.danger.strong.color.into()),
            Status::Active | Status::Disabled => None,
        },
        border: Border {
            radius: tokens::radius::SM.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn layout_name(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();

    let bg_color = match status {
        Status::Pressed => Some(palette.background.weak.color.into()),
        Status::Hovered => Some(palette.background.strong.color.into()),
        Status::Disabled | Status::Active => None,
    };

    Style {
        background: bg_color,
        text_color: palette.background.base.text,
        border: Border {
            radius: tokens::radius::MD.into(),
            width: tokens::border::THIN,
            color: iced::Color::TRANSPARENT,
        },
        ..Default::default()
    }
}
