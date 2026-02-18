use iced::{
    Border, Theme,
    widget::button::{Status, Style},
};

pub fn confirm(theme: &Theme, status: Status, is_active: bool) -> Style {
    let palette = theme.extended_palette();

    let color_alpha = if palette.is_dark { 0.2 } else { 0.6 };

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
            radius: 3.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn cancel(theme: &Theme, status: Status, is_active: bool) -> Style {
    let palette = theme.extended_palette();

    let color_alpha = if palette.is_dark { 0.2 } else { 0.6 };

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
            radius: 3.0.into(),
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
            radius: 4.0.into(),
            width: 1.0,
            color: iced::Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

pub fn transparent(theme: &Theme, status: Status, is_clicked: bool) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: palette.background.base.text,
        border: Border {
            radius: 3.0.into(),
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

pub fn modifier(theme: &Theme, status: Status, is_clicked: bool) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: palette.background.base.text,
        border: Border {
            radius: 3.0.into(),
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

pub fn bordered_toggle(theme: &Theme, status: Status, is_active: bool) -> Style {
    let palette = theme.extended_palette();

    iced::widget::button::Style {
        text_color: if is_active {
            palette.secondary.strong.color
        } else {
            palette.secondary.base.color
        },
        border: iced::Border {
            radius: 3.0.into(),
            width: if is_active { 2.0 } else { 1.0 },
            color: palette.background.weak.color,
        },
        background: match status {
            iced::widget::button::Status::Active => {
                if is_active {
                    Some(palette.background.base.color.into())
                } else {
                    Some(palette.background.weakest.color.into())
                }
            }
            iced::widget::button::Status::Pressed => {
                Some(palette.background.weakest.color.into())
            }
            iced::widget::button::Status::Hovered => Some(palette.background.weak.color.into()),
            iced::widget::button::Status::Disabled => {
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

pub fn info(theme: &Theme, _status: Status) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: palette.background.base.text,
        border: Border {
            radius: 3.0.into(),
            ..Default::default()
        },
        background: Some(palette.background.weakest.color.into()),
        ..Default::default()
    }
}

pub fn menu_body(theme: &Theme, status: Status, is_selected: bool) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: palette.background.base.text,
        border: Border {
            radius: 3.0.into(),
            width: if is_selected { 2.0 } else { 0.0 },
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

pub fn ticker_card(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();

    let color = if palette.is_dark {
        palette.background.weak.color
    } else {
        palette.background.strong.color
    };

    match status {
        Status::Hovered => Style {
            text_color: palette.background.base.text,
            background: Some(palette.background.weak.color.into()),
            border: Border {
                width: 1.0,
                radius: 2.0.into(),
                color,
            },
            ..Default::default()
        },
        _ => Style {
            background: Some(color.scale_alpha(0.4).into()),
            text_color: palette.background.base.text,
            border: Border {
                width: 1.0,
                radius: 2.0.into(),
                color: color.scale_alpha(0.8),
            },
            ..Default::default()
        },
    }
}

pub fn tab_active(theme: &Theme, _status: Status) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: palette.primary.base.text,
        background: Some(palette.primary.base.color.into()),
        border: Border {
            radius: 4.0.into(),
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
            radius: 4.0.into(),
            width: 1.0,
            color: palette.background.strong.color,
        },
        ..Default::default()
    }
}

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
            radius: 4.0.into(),
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
            radius: 4.0.into(),
            width: 1.0,
            color: palette.background.strong.color,
        },
        ..Default::default()
    }
}

#[allow(dead_code)]
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
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
