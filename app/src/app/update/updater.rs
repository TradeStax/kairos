use iced::Task;

use crate::app::messages::UpdateMessage;
use crate::app::{Kairos, Message};
use crate::components::display::toast::Toast;
use crate::services::updater;

impl Kairos {
    pub(crate) fn handle_update_message(&mut self, msg: UpdateMessage) -> Task<Message> {
        use crate::app::state::modals::UpdateStatus;

        match msg {
            UpdateMessage::CheckForUpdates => {
                self.modals.update_state.status = UpdateStatus::Checking;

                let version = crate::app::APP_VERSION.to_string();
                Task::perform(
                    async move { updater::check_for_update(&version).await },
                    |result| Message::Update(UpdateMessage::CheckComplete(result)),
                )
            }

            UpdateMessage::CheckComplete(result) => {
                match result {
                    Ok(Some(info)) => {
                        let version_str = info.new_version.to_string();
                        let is_critical = info.is_critical;

                        // Check if user skipped this version
                        if self
                            .persistence
                            .auto_update_prefs
                            .skipped_versions
                            .contains(&version_str)
                            && !is_critical
                        {
                            self.modals.update_state.status = UpdateStatus::Idle;
                            return Task::none();
                        }

                        self.modals.update_state.status = UpdateStatus::Available;
                        self.modals.update_state.update_info = Some(info);

                        if is_critical {
                            self.ui.push_notification(Toast::warn(format!(
                                "Critical update v{version_str} available"
                            )));
                            self.modals.update_state.show_modal = true;
                        } else {
                            self.ui.push_notification(Toast::info(format!(
                                "Update v{version_str} available — \
                                 File > Check for Updates to view"
                            )));
                        }

                        self.persistence.auto_update_prefs.last_check_epoch =
                            Some(chrono::Utc::now().timestamp());
                    }
                    Ok(None) => {
                        self.modals.update_state.status = UpdateStatus::Idle;
                        // Only show toast if user manually triggered the check
                        // (auto-check doesn't need "up to date" feedback)
                        self.persistence.auto_update_prefs.last_check_epoch =
                            Some(chrono::Utc::now().timestamp());
                    }
                    Err(e) => {
                        self.modals.update_state.status = UpdateStatus::Failed(e.clone());
                        log::error!("Update check failed: {e}");
                    }
                }
                Task::none()
            }

            UpdateMessage::ShowUpdateModal => {
                self.modals.update_state.show_modal = true;
                Task::none()
            }

            UpdateMessage::StartDownload => {
                let Some(info) = self.modals.update_state.update_info.clone() else {
                    return Task::none();
                };
                self.modals.update_state.status = UpdateStatus::Downloading;
                self.modals.update_state.download_progress = Some((0, info.size));

                let data_dir = crate::infra::platform::data_path(None);
                let archive_name = info
                    .download_url
                    .rsplit('/')
                    .next()
                    .unwrap_or("update-archive")
                    .to_string();
                let dest = data_dir.join("updates").join(&archive_name);
                let url = info.download_url.clone();
                let sha256 = info.sha256.clone();
                let tx = crate::app::core::globals::get_update_sender().clone();

                Task::perform(
                    async move { updater::download_update(&url, &dest, &sha256, &tx).await },
                    |result| Message::Update(UpdateMessage::DownloadComplete(result)),
                )
            }

            UpdateMessage::DownloadProgress { downloaded, total } => {
                self.modals.update_state.download_progress = Some((downloaded, total));
                Task::none()
            }

            UpdateMessage::DownloadComplete(result) => {
                match result {
                    Ok(path) => {
                        self.modals.update_state.downloaded_archive = Some(path.clone());

                        // Extract to staging
                        let data_dir = crate::infra::platform::data_path(None);
                        let staging = data_dir.join("updates").join("staged");
                        if let Err(e) = updater::extract_archive(&path, &staging) {
                            self.modals.update_state.status = UpdateStatus::Failed(e.clone());
                            self.ui
                                .push_notification(Toast::error(format!("Extract failed: {e}")));
                        } else {
                            self.modals.update_state.status = UpdateStatus::ReadyToInstall;
                            self.ui.push_notification(Toast::success(
                                "Update downloaded. Restart to install.",
                            ));
                        }
                    }
                    Err(e) => {
                        self.modals.update_state.status = UpdateStatus::Failed(e.clone());
                        self.modals.update_state.download_progress = None;
                        self.ui
                            .push_notification(Toast::error(format!("Download failed: {e}")));
                    }
                }
                Task::none()
            }

            UpdateMessage::InstallAndRestart => {
                self.modals.update_state.status = UpdateStatus::Installing;

                // Save state before restart
                let windows = std::collections::HashMap::new();
                self.save_state_to_disk(&windows);

                // Relaunch the application
                if let Ok(exe) = std::env::current_exe() {
                    let _ = std::process::Command::new(&exe).spawn();
                }
                iced::exit()
            }

            UpdateMessage::RemindLater => {
                self.modals.update_state.show_modal = false;
                Task::none()
            }

            UpdateMessage::SkipVersion(version) => {
                if !self
                    .persistence
                    .auto_update_prefs
                    .skipped_versions
                    .contains(&version)
                {
                    self.persistence
                        .auto_update_prefs
                        .skipped_versions
                        .push(version);
                }
                self.modals.update_state.show_modal = false;
                self.modals.update_state.status = UpdateStatus::Idle;
                self.modals.update_state.update_info = None;
                Task::none()
            }

            UpdateMessage::Dismiss => {
                self.modals.update_state.show_modal = false;
                Task::none()
            }
        }
    }
}
