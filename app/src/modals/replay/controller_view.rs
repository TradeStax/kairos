use super::*;

use super::volume_trackbar::volume_trackbar;
use crate::components::primitives::Icon;
use crate::components::primitives::icon_button::toolbar_icon;
use crate::components::primitives::label::{mono, small};
use crate::style::{self, tokens};
use iced::mouse;
use iced::widget::{button, column, container, mouse_area, pick_list, row, space, text};
use iced::{Alignment, Element, Length};

impl ReplayManager {
    // ── Floating Controller ───────────────────────────────────────────

    /// Compact floating controller with volume trackbar.
    pub fn view_floating_controller(&self, timezone: UserTimezone) -> Element<'_, Message> {
        let info_label = {
            let ticker = self
                .selected_stream
                .as_ref()
                .map(|s| s.ticker.to_string())
                .unwrap_or_default();
            small(format!("{} \u{00B7} {}", ticker, self.format_trade_count()))
        };

        let close_btn = button(text("\u{00D7}").size(tokens::text::TITLE))
            .on_press(Message::CloseController)
            .style(|theme, status| style::button::transparent(theme, status, false))
            .padding([0.0, tokens::spacing::XS]);

        let title_row = row![
            info_label,
            space::horizontal().width(Length::Fill),
            close_btn,
        ]
        .align_y(Alignment::Center);

        let title_bar = mouse_area(
            container(title_row)
                .width(Length::Fill)
                .padding([tokens::spacing::XS, tokens::spacing::MD])
                .style(style::floating_panel_header),
        )
        .on_press(Message::DragStart)
        .interaction(mouse::Interaction::Grab);

        let jump_back: Element<'_, Message> =
            toolbar_icon(Icon::SkipBackward, Message::JumpBackward)
                .size(10.0)
                .style(|theme, status| {
                    let mut s = style::button::transparent(theme, status, false);
                    s.border.radius = tokens::radius::ROUND.into();
                    s
                })
                .tooltip("-30s")
                .into();

        let play_pause: Element<'_, Message> = match self.playback_status {
            PlaybackStatus::Playing => toolbar_icon(Icon::Pause, Message::Pause)
                .size(12.0)
                .style(|theme, status| {
                    let mut s = style::button::transparent(theme, status, false);
                    s.border.radius = tokens::radius::ROUND.into();
                    s
                })
                .tooltip("Pause")
                .into(),
            _ => toolbar_icon(Icon::Play, Message::Play)
                .size(12.0)
                .style(|theme, status| {
                    let mut s = style::button::transparent(theme, status, false);
                    s.border.radius = tokens::radius::ROUND.into();
                    s
                })
                .tooltip("Play")
                .into(),
        };

        let jump_fwd: Element<'_, Message> = toolbar_icon(Icon::SkipForward, Message::JumpForward)
            .size(10.0)
            .style(|theme, status| {
                let mut s = style::button::transparent(theme, status, false);
                s.border.radius = tokens::radius::ROUND.into();
                s
            })
            .tooltip("+30s")
            .into();

        let trackbar = volume_trackbar(
            &self.volume_buckets,
            self.progress,
            self.time_range.as_ref(),
            timezone,
            Message::Seek,
        );

        let main_row = row![jump_back, play_pause, jump_fwd, trackbar]
            .spacing(tokens::spacing::SM)
            .align_y(Alignment::Center);

        let speed_picker = pick_list(
            &[
                SpeedPreset::Quarter,
                SpeedPreset::Half,
                SpeedPreset::Normal,
                SpeedPreset::Double,
                SpeedPreset::Five,
                SpeedPreset::Ten,
            ][..],
            Some(self.speed),
            Message::SetSpeed,
        )
        .text_size(tokens::text::TINY)
        .padding([tokens::spacing::XXS, tokens::spacing::XS]);

        let footer = row![
            mono(self.format_position(timezone)),
            space::horizontal().width(Length::Fill),
            speed_picker,
            space::horizontal().width(Length::Fill),
            mono(self.format_end_time(timezone)),
        ]
        .align_y(Alignment::Center);

        let body = column![main_row, footer]
            .spacing(tokens::spacing::XS)
            .padding([tokens::spacing::XS, tokens::spacing::MD]);

        let content = column![title_bar, body];

        container(content)
            .width(Length::Fixed(450.0))
            .style(style::floating_panel)
            .into()
    }
}
