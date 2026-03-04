//! Container styles. Only `container::Style` return types.

use iced::theme::palette::Extended as ExtendedPalette;
use iced::widget::container::Style;
use iced::{Border, Color, Shadow, Theme};

use crate::style::tokens;

// ── Theme-polarity helpers ────────────────────────────────────────────
//
// Dark themes need lighter surfaces for contrast, light themes need darker
// surfaces for the same effect — the polarity is inverted.
//
// These helpers encapsulate the recurring pattern:
//   if palette.is_dark { lighter_value } else { darker_value }

/// Returns `weak.color` on dark themes, `strong.color` on light themes.
/// Use for subtle surface-level backgrounds that must adapt to theme polarity.
fn shade_color(palette: &ExtendedPalette) -> Color {
    if palette.is_dark {
        palette.background.weak.color
    } else {
        palette.background.strong.color
    }
}

/// Returns `Color::BLACK` scaled by `dark_alpha` on dark themes,
/// `light_alpha` on light themes.
/// Use for shadows that must be heavier on dark themes to remain visible.
fn shadow_color(palette: &ExtendedPalette, dark_alpha: f32, light_alpha: f32) -> Color {
    Color::BLACK.scale_alpha(if palette.is_dark {
        dark_alpha
    } else {
        light_alpha
    })
}

// ── Window Title Bar ──────────────────────────────────────────────────

#[allow(dead_code)]
pub fn window_title_bar(theme: &Theme, hovered: bool) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: Some(
            if hovered {
                palette.background.weak.color
            } else {
                palette.background.base.color
            }
            .into(),
        ),
        ..Default::default()
    }
}

// ── Menu Bar ──────────────────────────────────────────────────────────

#[allow(dead_code)]
pub fn menu_bar(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: Some(palette.background.base.color.into()),
        ..Default::default()
    }
}

// ── Panes ─────────────────────────────────────────────────────────────

pub fn pane_title_bar(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: Some(
            shade_color(palette)
                .scale_alpha(tokens::alpha::FAINT)
                .into(),
        ),
        ..Default::default()
    }
}

pub fn pane_background(theme: &Theme, is_focused: bool) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: Some(palette.background.base.text),
        background: Some(palette.background.weakest.color.into()),
        border: {
            if is_focused {
                Border {
                    width: tokens::border::THIN,
                    color: palette.background.strong.color,
                    radius: tokens::radius::MD.into(),
                }
            } else {
                Border {
                    width: tokens::border::THIN,
                    color: shade_color(palette).scale_alpha(tokens::alpha::MEDIUM),
                    radius: tokens::radius::SM.into(),
                }
            }
        },
        ..Default::default()
    }
}

// ── Modals ────────────────────────────────────────────────────────────

pub fn chart_modal(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: Some(palette.background.base.text),
        background: Some(
            Color {
                a: tokens::alpha::OPAQUE,
                ..palette.background.base.color
            }
            .into(),
        ),
        border: Border {
            width: tokens::border::THIN,
            color: palette.background.weak.color,
            radius: tokens::radius::MD.into(),
        },
        shadow: Shadow {
            offset: iced::Vector { x: 0.0, y: 0.0 },
            blur_radius: tokens::shadow::XL,
            color: shadow_color(palette, tokens::alpha::LIGHT, tokens::alpha::FAINT),
        },
        snap: true,
    }
}

pub fn dashboard_modal(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: Some(
            Color {
                a: tokens::alpha::OPAQUE,
                ..palette.background.base.color
            }
            .into(),
        ),
        border: Border {
            width: tokens::border::THIN,
            color: palette.background.weak.color,
            radius: tokens::radius::MD.into(),
        },
        shadow: Shadow {
            offset: iced::Vector { x: 0.0, y: 0.0 },
            blur_radius: tokens::shadow::XXL,
            color: shadow_color(palette, tokens::alpha::HEAVY, tokens::alpha::LIGHT),
        },
        ..Default::default()
    }
}

pub fn confirm_modal(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: Some(palette.background.base.text),
        background: Some(
            Color {
                a: tokens::alpha::OPAQUE,
                ..palette.background.base.color
            }
            .into(),
        ),
        border: Border {
            width: tokens::border::MEDIUM,
            color: palette.primary.strong.color,
            radius: tokens::radius::LG.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, tokens::alpha::MEDIUM),
            offset: iced::Vector { x: 0.0, y: 4.0 },
            blur_radius: tokens::shadow::XL,
        },
        ..Default::default()
    }
}

pub fn modal_container(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: Some(palette.background.base.text),
        background: Some(palette.background.weakest.color.into()),
        border: Border {
            width: tokens::border::THIN,
            color: palette.background.weak.color,
            radius: tokens::radius::MD.into(),
        },
        shadow: Shadow {
            offset: iced::Vector { x: 0.0, y: 0.0 },
            blur_radius: tokens::shadow::SM,
            color: shadow_color(palette, tokens::alpha::HEAVY, tokens::alpha::FAINT),
        },
        snap: true,
    }
}

// ── Overlays ──────────────────────────────────────────────────────────

pub fn dropdown_container(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: Some(palette.background.base.color.into()),
        border: Border {
            width: tokens::border::THIN,
            color: palette.background.strong.color,
            radius: tokens::radius::MD.into(),
        },
        shadow: Shadow {
            offset: iced::Vector { x: 2.0, y: 2.0 },
            blur_radius: tokens::shadow::LG,
            color: shadow_color(palette, tokens::alpha::STRONG, tokens::alpha::SUBTLE),
        },
        ..Default::default()
    }
}

pub fn tooltip(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: Some(palette.background.weakest.color.into()),
        border: Border {
            width: tokens::border::THIN,
            color: palette.background.weak.color,
            radius: tokens::radius::MD.into(),
        },
        ..Default::default()
    }
}

// ── Floating panels ───────────────────────────────────────────────────

pub fn floating_panel(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: Some(palette.background.base.text),
        background: Some(palette.background.weakest.color.into()),
        border: Border {
            width: tokens::border::THIN,
            color: shade_color(palette).scale_alpha(tokens::alpha::MEDIUM),
            radius: tokens::radius::MD.into(),
        },
        shadow: Shadow {
            offset: iced::Vector { x: 0.0, y: 2.0 },
            blur_radius: tokens::shadow::XL,
            color: shadow_color(palette, tokens::alpha::HEAVY, tokens::alpha::LIGHT),
        },
        snap: true,
    }
}

pub fn floating_panel_header(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: Some(
            shade_color(palette)
                .scale_alpha(tokens::alpha::FAINT)
                .into(),
        ),
        ..Default::default()
    }
}

// ── Domain-specific ───────────────────────────────────────────────────

pub fn ticker_card(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: Some(
            shade_color(palette)
                .scale_alpha(tokens::alpha::LIGHT)
                .into(),
        ),
        border: Border {
            radius: tokens::radius::MD.into(),
            width: tokens::border::THIN,
            color: palette.background.strong.color,
        },
        ..Default::default()
    }
}

pub fn colored_circle_container(theme: &Theme, color: iced::Color) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: Some(color.into()),
        border: Border {
            width: tokens::border::THIN,
            color: palette.background.weak.color,
            radius: tokens::radius::ROUND.into(),
        },
        snap: true,
        ..Default::default()
    }
}

pub fn dragger_row_container(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    let bg_color = palette.background.strong.color;

    Style {
        text_color: Some(palette.background.base.text),
        background: Some(bg_color.into()),
        border: Border {
            width: tokens::border::THIN,
            color: bg_color,
            radius: tokens::radius::MD.into(),
        },
        shadow: Shadow {
            offset: iced::Vector { x: 0.0, y: 0.0 },
            blur_radius: tokens::shadow::MD,
            color: shadow_color(palette, tokens::alpha::HEAVY, tokens::alpha::FAINT),
        },
        snap: true,
    }
}
