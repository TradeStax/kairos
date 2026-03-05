#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod chart;
mod components;
mod config;
pub mod drawing;
mod infra;
mod modals;
pub mod persistence;
mod screen;
pub mod services;
mod style;
mod window;

use app::Kairos;
use std::borrow::Cow;

fn main() {
    // Apply staged update if one was downloaded in a previous session.
    // Runs before logger/GUI: swaps binary + assets on disk.
    services::updater_install::check_and_apply_staged_update();

    if let Err(e) = infra::logger::setup(cfg!(debug_assertions)) {
        // Logger failed to initialize (e.g., OS refused to create logging thread).
        // Fall through — the app can still run without logging.
        // In debug mode this is printed to stderr; in release mode it is silent.
        eprintln!("Warning: Failed to initialize logger: {e}");
    }

    // run() returns Err on GPU init failure or window creation failure.
    // In release mode (windows_subsystem = "windows") there is no console,
    // so we must surface errors through a panic message that Windows Error Reporting captures.
    iced::daemon(Kairos::new, Kairos::update, Kairos::view)
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
        .run()
        .expect("Kairos failed to start. Check GPU drivers and display configuration.");
}
