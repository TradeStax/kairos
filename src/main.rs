#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod chart;
mod components;
mod infra;
mod layout;
mod modals;
mod screen;
mod style;
mod window;

use app::Kairos;
use std::borrow::Cow;

fn main() {
    infra::logger::setup(cfg!(debug_assertions)).expect("Failed to initialize logger");

    let _ = iced::daemon(Kairos::new, Kairos::update, Kairos::view)
        .settings(iced::Settings {
            antialiasing: true,
            fonts: vec![
                Cow::Borrowed(components::primitives::AZERET_MONO_BYTES),
                Cow::Borrowed(components::primitives::ICONS_BYTES),
            ],
            default_text_size: iced::Pixels(12.0),
            ..Default::default()
        })
        .title(Kairos::title)
        .theme(Kairos::theme)
        .scale_factor(Kairos::scale_factor)
        .subscription(Kairos::subscription)
        .run();
}
