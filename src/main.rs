#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod chart;
mod component;
mod layout;
mod logger;
mod modal;
mod screen;
mod style;
mod widget;
mod window;

use app::Flowsurface;
use std::borrow::Cow;

fn main() {
    logger::setup(cfg!(debug_assertions)).expect("Failed to initialize logger");

    let _ = iced::daemon(Flowsurface::new, Flowsurface::update, Flowsurface::view)
        .settings(iced::Settings {
            antialiasing: true,
            fonts: vec![
                Cow::Borrowed(component::primitives::AZERET_MONO_BYTES),
                Cow::Borrowed(component::primitives::ICONS_BYTES),
            ],
            default_text_size: iced::Pixels(12.0),
            ..Default::default()
        })
        .title(Flowsurface::title)
        .theme(Flowsurface::theme)
        .scale_factor(Flowsurface::scale_factor)
        .subscription(Flowsurface::subscription)
        .run();
}
