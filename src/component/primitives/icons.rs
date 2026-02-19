use exchange::FuturesVenue;

use iced::font::{Family, Stretch, Weight};
use iced::widget::Text;
use iced::{Font, Renderer, Theme};

pub const ICONS_BYTES: &[u8] = include_bytes!("../../../assets/fonts/icons.ttf");
pub const ICONS_FONT: Font = Font::with_name("icons");

pub const AZERET_MONO_BYTES: &[u8] =
    include_bytes!("../../../assets/fonts/AzeretMono-Regular.ttf");
pub const AZERET_MONO: Font = Font {
    family: Family::Name("Azeret Mono"),
    weight: Weight::Normal,
    stretch: Stretch::Normal,
    style: iced::font::Style::Normal,
};

#[derive(Debug, Clone, Copy)]
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
    // Drawing tool icons
    DrawCursor,
    DrawLine,
    DrawRay,
    DrawHLine,
    DrawVLine,
    DrawRectangle,
    DrawTrendLine,
    // UI icons for drawing tools
    ExpandRight,
    SnapOn,
    SnapOff,
    // Replay controls
    Play,
    Pause,
    Stop,
    SkipForward,
    SkipBackward,
    Replay,
}

impl Icon {
    /// Whether this icon uses the default system font instead of the custom icon font.
    pub fn uses_default_font(self) -> bool {
        false
    }
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
            // Drawing tool icons - using Edit icon as placeholder
            // These should be replaced with proper icons when available in the font
            Icon::DrawCursor => '\u{E802}', // Search icon as cursor placeholder
            Icon::DrawLine => '\u{E811}',   // Edit icon as line placeholder
            Icon::DrawRay => '\u{E811}',    // Edit icon as ray placeholder
            Icon::DrawHLine => '\u{E817}',  // DragHandle as h-line placeholder
            Icon::DrawVLine => '\u{E817}',  // DragHandle as v-line placeholder
            Icon::DrawRectangle => '\u{E803}', // ResizeFull as rectangle placeholder
            Icon::DrawTrendLine => '\u{E811}', // Edit icon as trendline placeholder
            // UI icons
            Icon::ExpandRight => '\u{E808}', // Right arrow/caret
            Icon::SnapOn => '\u{E807}',      // Link icon for snap on
            Icon::SnapOff => '\u{E801}',     // Unlocked for snap off
            // Replay controls - custom icon font glyphs
            Icon::Play => '\u{E818}',
            Icon::Pause => '\u{E819}',
            Icon::Stop => '\u{E81A}',
            Icon::SkipForward => '\u{E81B}',
            Icon::SkipBackward => '\u{E81C}',
            Icon::Replay => '\u{E81D}',
        }
    }
}

pub fn icon_text<'a>(icon: Icon, size: u16) -> Text<'a, Theme, Renderer> {
    if icon.uses_default_font() {
        // Unicode transport symbols render in the default font
        iced::widget::text(char::from(icon).to_string())
            .size(iced::Pixels(size.into()))
    } else {
        iced::widget::text(char::from(icon).to_string())
            .font(ICONS_FONT)
            .size(iced::Pixels(size.into()))
    }
}

pub fn exchange_icon(venue: FuturesVenue) -> Icon {
    match venue {
        FuturesVenue::CMEGlobex => Icon::CmeGlobexLogo,
    }
}
