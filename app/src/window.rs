use std::collections::HashMap;

use data::state::WindowSpec;
use iced::{Point, Size, Subscription, Task, window};

pub use iced::window::{Id, Position, Settings, close, open};
use iced_futures::MaybeSend;

const ICON_RGBA: &[u8] = include_bytes!("../../assets/icons/icon_32x32.rgba");
const ICON_SIZE: u32 = 32;

pub fn icon() -> Option<window::Icon> {
    window::icon::from_rgba(ICON_RGBA.to_vec(), ICON_SIZE, ICON_SIZE).ok()
}

#[derive(Debug, Clone, Copy)]
pub struct Window {
    pub id: Id,
    pub position: Option<Point>,
    pub is_maximized: bool,
}

impl Window {
    pub fn new(id: Id) -> Self {
        Self {
            id,
            position: None,
            is_maximized: false,
        }
    }
}

pub fn default_size() -> Size {
    let spec = WindowSpec::default();
    Size::new(spec.width as f32, spec.height as f32)
}

#[derive(Debug, Clone, Copy)]
pub enum Event {
    CloseRequested(window::Id),
}

pub fn events() -> Subscription<Event> {
    iced::event::listen_with(filtered_events)
}

fn filtered_events(
    event: iced::Event,
    _status: iced::event::Status,
    window: window::Id,
) -> Option<Event> {
    match &event {
        iced::Event::Window(iced::window::Event::CloseRequested) => {
            Some(Event::CloseRequested(window))
        }
        _ => None,
    }
}

pub fn collect_window_specs<M, F>(window_ids: Vec<window::Id>, message: F) -> Task<M>
where
    F: Fn(HashMap<window::Id, WindowSpec>) -> M + Send + 'static,
    M: MaybeSend + 'static,
{
    // Create a task that collects specs for each window
    let window_spec_tasks = window_ids
        .into_iter()
        .map(|window_id| {
            // Map both tasks to produce an enum or tuple to distinguish them
            let pos_task: Task<(Option<Point>, Option<Size>)> =
                iced::window::position(window_id).map(|pos| (pos, None));

            let size_task: Task<(Option<Point>, Option<Size>)> =
                iced::window::size(window_id).map(|size| (None, Some(size)));

            Task::batch(vec![pos_task, size_task])
                .collect()
                .map(move |results| {
                    let position = results.iter().find_map(|(pos, _)| *pos);
                    let size = results
                        .iter()
                        .find_map(|(_, size)| *size)
                        .unwrap_or_else(|| Size::new(1024.0, 768.0));

                    (window_id, (position, size))
                })
        })
        .collect::<Vec<_>>();

    // Batch all window tasks together and collect results
    Task::batch(window_spec_tasks)
        .collect()
        .map(move |results| {
            let specs: HashMap<window::Id, WindowSpec> = results
                .into_iter()
                .filter_map(|(id, (pos, size))| {
                    pos.map(|position| {
                        (
                            id,
                            WindowSpec {
                                x: Some(position.x as i32),
                                y: Some(position.y as i32),
                                width: size.width as u32,
                                height: size.height as u32,
                            },
                        )
                    })
                })
                .collect();

            message(specs)
        })
}

#[cfg(target_os = "linux")]
pub fn settings() -> Settings {
    Settings {
        decorations: false,
        icon: icon(),
        min_size: Some(Size::new(800.0, 600.0)),
        ..Default::default()
    }
}

#[cfg(target_os = "macos")]
pub fn settings() -> Settings {
    use iced::window;

    Settings {
        platform_specific: window::settings::PlatformSpecific {
            title_hidden: true,
            titlebar_transparent: true,
            fullsize_content_view: true,
        },
        icon: icon(),
        min_size: Some(Size::new(800.0, 600.0)),
        ..Default::default()
    }
}

#[cfg(target_os = "windows")]
pub fn settings() -> Settings {
    Settings {
        decorations: false,
        icon: icon(),
        min_size: Some(Size::new(800.0, 600.0)),
        ..Default::default()
    }
}
