pub mod animation;
#[allow(dead_code)] // Design-system tokens are pre-defined ahead of usage
pub mod tokens;

pub(crate) mod theme;
mod widgets;

// Preserve module paths: style::palette, style::button, style::slider
pub use theme::palette;
pub use widgets::button;
pub use widgets::slider;

// Preserve function re-exports: style::dashed_line, style::chart_modal, etc.
pub use widgets::canvas::{dashed_line, dashed_line_from_palette};
pub use widgets::common::{pane_grid, progress_bar, scroll_bar, split_ruler, validated_text_input};
pub use widgets::container::{
    chart_modal, colored_circle_container, confirm_modal, dashboard_modal, dragger_row_container,
    dropdown_container, floating_panel, floating_panel_header, menu_bar, modal_container,
    pane_background, pane_title_bar, ticker_card, tooltip, window_title_bar,
};

/// Use `tokens::layout::TITLE_PADDING_TOP` for new code.
pub const TITLE_PADDING_TOP: f32 = tokens::layout::TITLE_PADDING_TOP;

#[cfg(target_os = "macos")]
pub fn title_text(theme: &iced::Theme) -> iced::widget::text::Style {
    let palette = theme.extended_palette();

    iced::widget::text::Style {
        color: Some(palette.background.weakest.color),
    }
}
