//! Update modal — shows version info, release notes, download progress, and actions.

use iced::widget::{button, column, container, progress_bar, row, text};
use iced::{Alignment, Element, Length, padding};

use crate::app::messages::UpdateMessage;
use crate::app::state::modals::{UpdateState, UpdateStatus};
use crate::style::{self, palette, tokens};

pub fn view_update_modal(state: &UpdateState) -> Element<'_, UpdateMessage> {
    let Some(info) = &state.update_info else {
        return text("No update information available.").into();
    };

    let mut content = column![].spacing(tokens::spacing::MD);

    // Version header
    content = content.push(
        text(format!("v{} → v{}", info.current_version, info.new_version))
            .size(tokens::text::HEADING)
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            }),
    );

    // Release date
    content = content.push(
        text(format!("Released: {}", info.release_date))
            .size(tokens::text::BODY)
            .style(palette::neutral_text),
    );

    // File size
    let size_mb = info.size as f64 / (1024.0 * 1024.0);
    content = content.push(
        text(format!("Download size: {size_mb:.1} MB"))
            .size(tokens::text::BODY)
            .style(palette::neutral_text),
    );

    // Critical warning
    if info.is_critical {
        content = content.push(
            text("This is a critical update. Please install it as soon as possible.")
                .size(tokens::text::BODY)
                .style(palette::warning_text),
        );
    }

    // Release notes
    if !info.release_notes.is_empty() {
        content = content.push(
            text("Release Notes")
                .size(tokens::text::LABEL)
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                }),
        );
        content = content.push(text(&info.release_notes).size(tokens::text::BODY));
    }

    // Download progress
    if let Some((downloaded, total)) = state.download_progress {
        let pct = if total > 0 {
            (downloaded as f32 / total as f32) * 100.0
        } else {
            0.0
        };
        let downloaded_mb = downloaded as f64 / (1024.0 * 1024.0);
        let total_mb = total as f64 / (1024.0 * 1024.0);

        content = content.push(
            column![
                progress_bar(0.0..=100.0, pct),
                text(format!("{downloaded_mb:.1} / {total_mb:.1} MB ({pct:.0}%)"))
                    .size(tokens::text::TINY)
                    .style(palette::neutral_text),
            ]
            .spacing(tokens::spacing::XS),
        );
    }

    // Action buttons
    let footer = match &state.status {
        UpdateStatus::Available => {
            let version_str = info.new_version.to_string();
            let download_btn = button(text("Download & Install").size(tokens::text::BODY))
                .padding(padding::Padding {
                    top: tokens::spacing::SM,
                    bottom: tokens::spacing::SM,
                    left: tokens::spacing::LG,
                    right: tokens::spacing::LG,
                })
                .on_press(UpdateMessage::StartDownload)
                .style(style::button::primary);

            let remind_btn = button(text("Remind Later").size(tokens::text::BODY))
                .padding(padding::Padding {
                    top: tokens::spacing::SM,
                    bottom: tokens::spacing::SM,
                    left: tokens::spacing::MD,
                    right: tokens::spacing::MD,
                })
                .on_press(UpdateMessage::RemindLater)
                .style(style::button::secondary);

            let skip_btn = button(text("Skip Version").size(tokens::text::BODY))
                .padding(padding::Padding {
                    top: tokens::spacing::SM,
                    bottom: tokens::spacing::SM,
                    left: tokens::spacing::MD,
                    right: tokens::spacing::MD,
                })
                .on_press(UpdateMessage::SkipVersion(version_str))
                .style(style::button::secondary);

            row![download_btn, remind_btn, skip_btn]
                .spacing(tokens::spacing::SM)
                .align_y(Alignment::Center)
        }
        UpdateStatus::Downloading => {
            let cancel_btn = button(text("Cancel").size(tokens::text::BODY))
                .padding(padding::Padding {
                    top: tokens::spacing::SM,
                    bottom: tokens::spacing::SM,
                    left: tokens::spacing::MD,
                    right: tokens::spacing::MD,
                })
                .on_press(UpdateMessage::Dismiss)
                .style(style::button::secondary);

            row![text("Downloading...").size(tokens::text::BODY), cancel_btn,]
                .spacing(tokens::spacing::SM)
                .align_y(Alignment::Center)
        }
        UpdateStatus::ReadyToInstall => {
            let restart_btn = button(text("Restart Now").size(tokens::text::BODY))
                .padding(padding::Padding {
                    top: tokens::spacing::SM,
                    bottom: tokens::spacing::SM,
                    left: tokens::spacing::LG,
                    right: tokens::spacing::LG,
                })
                .on_press(UpdateMessage::InstallAndRestart)
                .style(style::button::primary);

            let later_btn = button(text("Later").size(tokens::text::BODY))
                .padding(padding::Padding {
                    top: tokens::spacing::SM,
                    bottom: tokens::spacing::SM,
                    left: tokens::spacing::MD,
                    right: tokens::spacing::MD,
                })
                .on_press(UpdateMessage::RemindLater)
                .style(style::button::secondary);

            row![restart_btn, later_btn]
                .spacing(tokens::spacing::SM)
                .align_y(Alignment::Center)
        }
        UpdateStatus::Failed(e) => {
            let retry_btn = button(text("Retry").size(tokens::text::BODY))
                .padding(padding::Padding {
                    top: tokens::spacing::SM,
                    bottom: tokens::spacing::SM,
                    left: tokens::spacing::MD,
                    right: tokens::spacing::MD,
                })
                .on_press(UpdateMessage::CheckForUpdates)
                .style(style::button::secondary);

            let dismiss_btn = button(text("Dismiss").size(tokens::text::BODY))
                .padding(padding::Padding {
                    top: tokens::spacing::SM,
                    bottom: tokens::spacing::SM,
                    left: tokens::spacing::MD,
                    right: tokens::spacing::MD,
                })
                .on_press(UpdateMessage::Dismiss)
                .style(style::button::secondary);

            content = content.push(
                text(format!("Error: {e}"))
                    .size(tokens::text::TINY)
                    .style(palette::error_text),
            );

            row![retry_btn, dismiss_btn]
                .spacing(tokens::spacing::SM)
                .align_y(Alignment::Center)
        }
        _ => row![],
    };

    content = content.push(footer);

    container(content)
        .width(Length::Fixed(480.0))
        .padding(tokens::spacing::XL)
        .style(style::dashboard_modal)
        .into()
}
