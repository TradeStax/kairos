use iced::theme::palette::Extended;
use iced::widget::canvas::{LineDash, Stroke};
use iced::widget::container::Style;
use iced::widget::pane_grid::{Highlight, Line};
use iced::widget::scrollable::{AutoScroll, Rail, Scroller};
use iced::{Border, Color, Shadow, Theme, widget};

// Panes
pub fn pane_grid(theme: &Theme) -> widget::pane_grid::Style {
    let palette = theme.extended_palette();

    widget::pane_grid::Style {
        hovered_region: Highlight {
            background: palette.background.strongest.color.scale_alpha(0.5).into(),
            border: Border {
                width: 1.0,
                color: palette.background.strongest.color,
                radius: 4.0.into(),
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

pub fn pane_title_bar(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: {
            if palette.is_dark {
                Some(palette.background.weak.color.scale_alpha(0.2).into())
            } else {
                Some(palette.background.strong.color.scale_alpha(0.2).into())
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
                    width: 1.0,
                    color: palette.background.strong.color,
                    radius: 4.0.into(),
                }
            } else {
                Border {
                    width: 1.0,
                    color: color.scale_alpha(0.5),
                    radius: 2.0.into(),
                }
            }
        },
        ..Default::default()
    }
}

// Modals
pub fn chart_modal(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: Some(palette.background.base.text),
        background: Some(
            Color {
                a: 0.99,
                ..palette.background.base.color
            }
            .into(),
        ),
        border: Border {
            width: 1.0,
            color: palette.background.weak.color,
            radius: 4.0.into(),
        },
        shadow: Shadow {
            offset: iced::Vector { x: 0.0, y: 0.0 },
            blur_radius: 12.0,
            color: Color::BLACK.scale_alpha(if palette.is_dark { 0.4 } else { 0.2 }),
        },
        snap: true,
    }
}

pub fn dashboard_modal(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: Some(
            Color {
                a: 0.99,
                ..palette.background.base.color
            }
            .into(),
        ),
        border: Border {
            width: 1.0,
            color: palette.background.weak.color,
            radius: 4.0.into(),
        },
        shadow: Shadow {
            offset: iced::Vector { x: 0.0, y: 0.0 },
            blur_radius: 20.0,
            color: Color::BLACK.scale_alpha(if palette.is_dark { 0.8 } else { 0.4 }),
        },
        ..Default::default()
    }
}

/// Compact dropdown container for tool selection
pub fn dropdown_container(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: Some(palette.background.base.color.into()),
        border: Border {
            width: 1.0,
            color: palette.background.strong.color,
            radius: 4.0.into(),
        },
        shadow: Shadow {
            offset: iced::Vector { x: 2.0, y: 2.0 },
            blur_radius: 8.0,
            color: Color::BLACK.scale_alpha(if palette.is_dark { 0.6 } else { 0.3 }),
        },
        ..Default::default()
    }
}

/// Container for drawing tools section in sidebar
pub fn drawing_tools_container(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: Some(
            if palette.is_dark {
                palette.background.weak.color.scale_alpha(0.3)
            } else {
                palette.background.strong.color.scale_alpha(0.3)
            }
            .into(),
        ),
        border: Border {
            width: 1.0,
            color: if palette.is_dark {
                palette.background.weak.color.scale_alpha(0.5)
            } else {
                palette.background.strong.color.scale_alpha(0.5)
            },
            radius: 6.0.into(),
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
                a: 0.98,
                ..palette.background.base.color
            }
            .into(),
        ),
        border: Border {
            width: 1.5,
            color: palette.primary.strong.color,
            radius: 6.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
            offset: iced::Vector { x: 0.0, y: 4.0 },
            blur_radius: 12.0,
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
            width: 1.0,
            color: palette.background.weak.color,
            radius: 4.0.into(),
        },
        shadow: Shadow {
            offset: iced::Vector { x: 0.0, y: 0.0 },
            blur_radius: 2.0,
            color: Color::BLACK.scale_alpha(if palette.is_dark { 0.8 } else { 0.2 }),
        },
        snap: true,
    }
}

// Progress bar styles
pub fn progress_bar(theme: &Theme) -> widget::progress_bar::Style {
    let palette = theme.extended_palette();

    widget::progress_bar::Style {
        background: palette.background.weak.color.into(),
        bar: palette.primary.base.color.into(),
        border: Border {
            width: 0.0,
            color: Color::TRANSPARENT,
            radius: 3.0.into(),
        },
    }
}

pub fn colored_circle_container(theme: &Theme, color: iced::Color) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: Some(color.into()),
        border: Border {
            width: 1.0,
            color: palette.background.weak.color,
            radius: 16.0.into(),
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
            width: 1.0,
            color: bg_color,
            radius: 4.0.into(),
        },
        shadow: Shadow {
            offset: iced::Vector { x: 0.0, y: 0.0 },
            blur_radius: 4.0,
            color: Color::BLACK.scale_alpha(if palette.is_dark { 0.8 } else { 0.2 }),
        },
        snap: true,
    }
}

pub fn tooltip(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: Some(palette.background.weakest.color.into()),
        border: Border {
            width: 1.0,
            color: palette.background.weak.color,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

// Tickers Table
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
            radius: 3.0.into(),
            width: 1.0,
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

pub fn ticker_card(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: {
            if palette.is_dark {
                Some(palette.background.weak.color.scale_alpha(0.4).into())
            } else {
                Some(palette.background.strong.color.scale_alpha(0.4).into())
            }
        },
        border: Border {
            radius: 4.0.into(),
            width: 1.0,
            color: palette.background.strong.color,
        },
        ..Default::default()
    }
}

// the bar that lights up depending on the price change
pub fn ticker_card_bar(theme: &Theme, color_alpha: f32) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: {
            if color_alpha > 0.0 {
                Some(palette.success.strong.color.scale_alpha(color_alpha).into())
            } else {
                Some(palette.danger.strong.color.scale_alpha(-color_alpha).into())
            }
        },
        border: Border {
            radius: 4.0.into(),
            width: 1.0,
            color: if color_alpha > 0.0 {
                palette.success.strong.color.scale_alpha(color_alpha)
            } else {
                palette.danger.strong.color.scale_alpha(-color_alpha)
            },
        },
        ..Default::default()
    }
}

// Scrollable
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
            radius: 2.0.into(),
            width: 1.0,
            color: Color::TRANSPARENT,
        },
        scroller: Scroller {
            background: iced::Background::Color(scroller_bg),
            border: Border {
                radius: 2.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
        },
    };

    let auto_scroll = AutoScroll {
        background: iced::Background::Color(palette.background.weakest.color),
        border: Border {
            radius: 2.0.into(),
            width: 1.0,
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

// custom widgets
pub fn split_ruler(theme: &Theme) -> iced::widget::rule::Style {
    let palette = theme.extended_palette();

    iced::widget::rule::Style {
        color: palette.background.strong.color.scale_alpha(0.25),
        radius: iced::border::Radius::default(),
        fill_mode: iced::widget::rule::FillMode::Full,
        snap: true,
    }
}

// crosshair dashed line for charts
pub fn dashed_line(theme: &'_ Theme) -> Stroke<'_> {
    let palette = theme.extended_palette();

    Stroke::with_color(
        Stroke {
            width: 1.0,
            line_dash: LineDash {
                segments: &[4.0, 4.0],
                offset: 8,
            },
            ..Default::default()
        },
        palette
            .secondary
            .strong
            .color
            .scale_alpha(if palette.is_dark { 0.8 } else { 1.0 }),
    )
}

pub fn dashed_line_from_palette(palette: &'_ Extended) -> Stroke<'_> {
    Stroke::with_color(
        Stroke {
            width: 1.0,
            line_dash: LineDash {
                segments: &[4.0, 4.0],
                offset: 8,
            },
            ..Default::default()
        },
        palette
            .secondary
            .strong
            .color
            .scale_alpha(if palette.is_dark { 0.8 } else { 1.0 }),
    )
}
