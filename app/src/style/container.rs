//! Container styles. Only `container::Style` return types.

use iced::widget::container::Style;
use iced::{Border, Color, Shadow, Theme};

use super::tokens;

// ── Window Title Bar ──────────────────────────────────────────────────

pub fn window_title_bar(theme: &Theme, hovered: bool) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: Some(if hovered {
            palette.background.weak.color
        } else {
            palette.background.base.color
        }.into()),
        ..Default::default()
    }
}

// ── Menu Bar ──────────────────────────────────────────────────────────

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
        background: {
            if palette.is_dark {
                Some(
                    palette
                        .background
                        .weak
                        .color
                        .scale_alpha(tokens::alpha::FAINT)
                        .into(),
                )
            } else {
                Some(
                    palette
                        .background
                        .strong
                        .color
                        .scale_alpha(tokens::alpha::FAINT)
                        .into(),
                )
            }
        },
        ..Default::default()
    }
}

pub fn pane_background(theme: &Theme, is_focused: bool) -> Style {
    let palette = theme.extended_palette();

    let color = if palette.is_dark {
        palette.background.weak.color
    } else {
        palette.background.strong.color
    };

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
                    color: color.scale_alpha(tokens::alpha::MEDIUM),
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
            color: Color::BLACK.scale_alpha(if palette.is_dark {
                tokens::alpha::LIGHT
            } else {
                tokens::alpha::FAINT
            }),
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
            color: Color::BLACK.scale_alpha(if palette.is_dark {
                tokens::alpha::HEAVY
            } else {
                tokens::alpha::LIGHT
            }),
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
            color: Color::BLACK.scale_alpha(if palette.is_dark {
                tokens::alpha::HEAVY
            } else {
                tokens::alpha::FAINT
            }),
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
            color: Color::BLACK.scale_alpha(if palette.is_dark {
                tokens::alpha::STRONG
            } else {
                tokens::alpha::SUBTLE
            }),
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
            color: if palette.is_dark {
                palette
                    .background
                    .weak
                    .color
                    .scale_alpha(tokens::alpha::MEDIUM)
            } else {
                palette
                    .background
                    .strong
                    .color
                    .scale_alpha(tokens::alpha::MEDIUM)
            },
            radius: tokens::radius::MD.into(),
        },
        shadow: Shadow {
            offset: iced::Vector { x: 0.0, y: 2.0 },
            blur_radius: tokens::shadow::XL,
            color: Color::BLACK.scale_alpha(if palette.is_dark {
                tokens::alpha::HEAVY
            } else {
                tokens::alpha::LIGHT
            }),
        },
        snap: true,
    }
}

pub fn floating_panel_header(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: {
            if palette.is_dark {
                Some(
                    palette
                        .background
                        .weak
                        .color
                        .scale_alpha(tokens::alpha::FAINT)
                        .into(),
                )
            } else {
                Some(
                    palette
                        .background
                        .strong
                        .color
                        .scale_alpha(tokens::alpha::FAINT)
                        .into(),
                )
            }
        },
        ..Default::default()
    }
}

// ── Domain-specific ───────────────────────────────────────────────────

pub fn ticker_card(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: {
            if palette.is_dark {
                Some(
                    palette
                        .background
                        .weak
                        .color
                        .scale_alpha(tokens::alpha::LIGHT)
                        .into(),
                )
            } else {
                Some(
                    palette
                        .background
                        .strong
                        .color
                        .scale_alpha(tokens::alpha::LIGHT)
                        .into(),
                )
            }
        },
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
            color: Color::BLACK.scale_alpha(if palette.is_dark {
                tokens::alpha::HEAVY
            } else {
                tokens::alpha::FAINT
            }),
        },
        snap: true,
    }
}
