use exchange::FuturesVenue;

use iced::font::{Family, Stretch, Weight};
use iced::widget::Text;
use iced::{Font, Renderer, Theme};

pub mod button;
mod container;

// Re-export container styles at module level for backwards compatibility
pub use container::*;

pub const ICONS_BYTES: &[u8] = include_bytes!("../../assets/fonts/icons.ttf");
pub const ICONS_FONT: Font = Font::with_name("icons");

pub const AZERET_MONO_BYTES: &[u8] = include_bytes!("../../assets/fonts/AzeretMono-Regular.ttf");
pub const AZERET_MONO: Font = Font {
    family: Family::Name("Azeret Mono"),
    weight: Weight::Normal,
    stretch: Stretch::Normal,
    style: iced::font::Style::Normal,
};

pub const TITLE_PADDING_TOP: f32 = if cfg!(target_os = "macos") { 20.0 } else { 0.0 };

pub enum Icon {
    Locked,
    Unlocked,
    ResizeFull,
    ResizeSmall,
    Close,
    Layout,
    Cog,
    Link,
    CmeGlobexLogo,
    Search,
    Sort,
    SortDesc,
    SortAsc,
    Star,
    StarFilled,
    Return,
    Popout,
    ChartOutline,
    TrashBin,
    Edit,
    Checkmark,
    Clone,
    SpeakerOff,
    SpeakerLow,
    SpeakerHigh,
    DragHandle,
    Folder,
    ExternalLink,
    Database,
}

impl From<Icon> for char {
    fn from(icon: Icon) -> Self {
        match icon {
            Icon::Locked => '\u{E800}',
            Icon::Unlocked => '\u{E801}',
            Icon::Search => '\u{E802}',
            Icon::ResizeFull => '\u{E803}',
            Icon::ResizeSmall => '\u{E804}',
            Icon::Close => '\u{E805}',
            Icon::Layout => '\u{E806}',
            Icon::Link => '\u{E807}',
            Icon::CmeGlobexLogo => '\u{E801}',
            Icon::Cog => '\u{E810}',
            Icon::Sort => '\u{F0DC}',
            Icon::SortDesc => '\u{F0DD}',
            Icon::SortAsc => '\u{F0DE}',
            Icon::Star => '\u{E80A}',
            Icon::StarFilled => '\u{E80B}',
            Icon::Return => '\u{E80C}',
            Icon::Popout => '\u{E80D}',
            Icon::ChartOutline => '\u{E80E}',
            Icon::TrashBin => '\u{E80F}',
            Icon::Edit => '\u{E811}',
            Icon::Checkmark => '\u{E812}',
            Icon::Clone => '\u{F0C5}',
            Icon::SpeakerOff => '\u{E814}',
            Icon::SpeakerHigh => '\u{E815}',
            Icon::SpeakerLow => '\u{E816}',
            Icon::DragHandle => '\u{E817}',
            Icon::Folder => '\u{F114}',
            Icon::ExternalLink => '\u{F14C}',
            Icon::Database => '\u{F1C0}',
        }
    }
}

pub fn icon_text<'a>(icon: Icon, size: u16) -> Text<'a, Theme, Renderer> {
    iced::widget::text(char::from(icon).to_string())
        .font(ICONS_FONT)
        .size(iced::Pixels(size.into()))
}

pub fn exchange_icon(venue: FuturesVenue) -> Icon {
    match venue {
        FuturesVenue::CMEGlobex => Icon::CmeGlobexLogo,
    }
}

#[cfg(target_os = "macos")]
pub fn title_text(theme: &Theme) -> iced::widget::text::Style {
    let palette = theme.extended_palette();

    iced::widget::text::Style {
        color: Some(palette.background.weakest.color),
    }
}
