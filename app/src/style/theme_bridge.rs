//! Converts between data-layer theme/color and iced at the app boundary.
//! Keeps GUI code using iced types while state stays data-only.

use data::config::theme::default_theme_palette;
use data::{Rgba, Theme};
use iced_core::theme::{Custom, Palette};
use iced_core::{Color, Theme as IcedTheme};

/// data Rgba → iced Color (for rendering). Use this instead of From to avoid orphan impl.
#[inline]
pub(crate) fn rgba_to_iced_color(r: Rgba) -> Color {
    Color {
        r: r.r,
        g: r.g,
        b: r.b,
        a: r.a,
    }
}

/// iced Color → data Rgba (for persistence).
pub(crate) fn iced_color_to_rgba(c: Color) -> Rgba {
    Rgba::new(c.r, c.g, c.b, c.a)
}

/// data Theme → iced Theme (for rendering and pick list).
pub(crate) fn theme_to_iced(theme: &Theme) -> IcedTheme {
    if theme.id == "custom" {
        if let Some(ref pal) = theme.custom_palette {
            let palette = Palette {
                background: rgba_to_iced_color(pal.background),
                text: rgba_to_iced_color(pal.text),
                primary: rgba_to_iced_color(pal.primary),
                success: rgba_to_iced_color(pal.success),
                danger: rgba_to_iced_color(pal.danger),
                warning: rgba_to_iced_color(pal.warning),
            };
            return IcedTheme::Custom(Custom::new("Custom".to_string(), palette).into());
        }
    }
    match theme.id.as_str() {
        "ferra" => IcedTheme::Ferra,
        "dark" => IcedTheme::Dark,
        "light" => IcedTheme::Light,
        "dracula" => IcedTheme::Dracula,
        "nord" => IcedTheme::Nord,
        "solarized_light" => IcedTheme::SolarizedLight,
        "solarized_dark" => IcedTheme::SolarizedDark,
        "gruvbox_light" => IcedTheme::GruvboxLight,
        "gruvbox_dark" => IcedTheme::GruvboxDark,
        "catppuccin_latte" => IcedTheme::CatppuccinLatte,
        "catppuccin_frappe" => IcedTheme::CatppuccinFrappe,
        "catppuccin_macchiato" => IcedTheme::CatppuccinMacchiato,
        "catppuccin_mocha" => IcedTheme::CatppuccinMocha,
        "tokyo_night" => IcedTheme::TokyoNight,
        "tokyo_night_storm" => IcedTheme::TokyoNightStorm,
        "tokyo_night_light" => IcedTheme::TokyoNightLight,
        "kanagawa_wave" => IcedTheme::KanagawaWave,
        "kanagawa_dragon" => IcedTheme::KanagawaDragon,
        "kanagawa_lotus" => IcedTheme::KanagawaLotus,
        "moonfly" => IcedTheme::Moonfly,
        "nightfly" => IcedTheme::Nightfly,
        "oxocarbon" => IcedTheme::Oxocarbon,
        "kairos" | _ => default_iced_theme(),
    }
}

/// iced Theme → data Theme (for persistence when user picks a theme).
pub(crate) fn iced_theme_to_data(iced: IcedTheme) -> Theme {
    use iced_core::Theme as I;
    let (id, custom_palette) = match &iced {
        I::Ferra => ("ferra".to_string(), None),
        I::Dark => ("dark".to_string(), None),
        I::Light => ("light".to_string(), None),
        I::Dracula => ("dracula".to_string(), None),
        I::Nord => ("nord".to_string(), None),
        I::SolarizedLight => ("solarized_light".to_string(), None),
        I::SolarizedDark => ("solarized_dark".to_string(), None),
        I::GruvboxLight => ("gruvbox_light".to_string(), None),
        I::GruvboxDark => ("gruvbox_dark".to_string(), None),
        I::CatppuccinLatte => ("catppuccin_latte".to_string(), None),
        I::CatppuccinFrappe => ("catppuccin_frappe".to_string(), None),
        I::CatppuccinMacchiato => ("catppuccin_macchiato".to_string(), None),
        I::CatppuccinMocha => ("catppuccin_mocha".to_string(), None),
        I::TokyoNight => ("tokyo_night".to_string(), None),
        I::TokyoNightStorm => ("tokyo_night_storm".to_string(), None),
        I::TokyoNightLight => ("tokyo_night_light".to_string(), None),
        I::KanagawaWave => ("kanagawa_wave".to_string(), None),
        I::KanagawaDragon => ("kanagawa_dragon".to_string(), None),
        I::KanagawaLotus => ("kanagawa_lotus".to_string(), None),
        I::Moonfly => ("moonfly".to_string(), None),
        I::Nightfly => ("nightfly".to_string(), None),
        I::Oxocarbon => ("oxocarbon".to_string(), None),
        I::Custom(_) => {
            let palette = iced.palette();
            let ser = data::config::theme::SerPalette {
                background: iced_color_to_rgba(palette.background),
                text: iced_color_to_rgba(palette.text),
                primary: iced_color_to_rgba(palette.primary),
                success: iced_color_to_rgba(palette.success),
                danger: iced_color_to_rgba(palette.danger),
                warning: iced_color_to_rgba(palette.warning),
            };
            let id = "custom";
            (id.to_string(), Some(ser))
        }
        _ => ("kairos".to_string(), None),
    };
    Theme { id, custom_palette }
}

/// Default iced theme (Kairos built-in) for pick list and initial state.
pub(crate) fn default_iced_theme() -> IcedTheme {
    let pal = default_theme_palette();
    let palette = Palette {
        background: rgba_to_iced_color(pal.background),
        text: rgba_to_iced_color(pal.text),
        primary: rgba_to_iced_color(pal.primary),
        success: rgba_to_iced_color(pal.success),
        danger: rgba_to_iced_color(pal.danger),
        warning: rgba_to_iced_color(pal.warning),
    };
    IcedTheme::Custom(Custom::new("Kairos".to_string(), palette).into())
}
