pub mod button;
mod canvas;
mod container;
pub mod palette;
#[allow(dead_code)]
pub mod tokens;
mod widget;

pub use canvas::*;
pub use container::*;
pub use widget::*;

pub const TITLE_PADDING_TOP: f32 = if cfg!(target_os = "macos") { 20.0 } else { 0.0 };

#[cfg(target_os = "macos")]
pub fn title_text(theme: &iced::Theme) -> iced::widget::text::Style {
    let palette = theme.extended_palette();

    iced::widget::text::Style {
        color: Some(palette.background.weakest.color),
    }
}
