pub mod animation;
pub mod button;
mod canvas;
mod container;
pub mod palette;
pub mod slider;
pub(crate) mod theme;
#[allow(dead_code)]
pub mod tokens;
mod widget;

pub use canvas::{dashed_line, dashed_line_from_palette};
pub use container::{
    chart_modal, colored_circle_container, confirm_modal, dashboard_modal,
    dragger_row_container, dropdown_container, floating_panel, floating_panel_header,
    menu_bar, modal_container, pane_background, pane_title_bar, ticker_card, tooltip,
    window_title_bar,
};
pub use widget::{pane_grid, progress_bar, scroll_bar, split_ruler, validated_text_input};

/// Use `tokens::layout::TITLE_PADDING_TOP` for new code.
pub const TITLE_PADDING_TOP: f32 = tokens::layout::TITLE_PADDING_TOP;

#[cfg(target_os = "macos")]
pub fn title_text(theme: &iced::Theme) -> iced::widget::text::Style {
    let palette = theme.extended_palette();

    iced::widget::text::Style {
        color: Some(palette.background.weakest.color),
    }
}
