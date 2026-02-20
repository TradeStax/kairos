use exchange::FuturesVenue;

use iced::font::{Family, Stretch, Weight};
use iced::widget::Text;
use iced::{Font, Renderer, Theme};

pub const ICONS_BYTES: &[u8] = include_bytes!("../../../../assets/fonts/icons.ttf");
pub const ICONS_FONT: Font = Font::with_name("icons");

pub const AZERET_MONO_BYTES: &[u8] = include_bytes!("../../../../assets/fonts/AzeretMono-Regular.ttf");
pub const AZERET_MONO: Font = Font {
    family: Family::Name("Azeret Mono"),
    weight: Weight::Normal,
    stretch: Stretch::Normal,
    style: iced::font::Style::Normal,
};

/// Icon glyphs from Feather Icons (feathericons.com).
///
/// All icons use the custom "icons" font except `Minimize` which
/// uses the default system font.
#[derive(Debug, Clone, Copy)]
pub enum Icon {
    // ── General UI ────────────────────────────────────────────────
    Locked,        // lock
    Unlocked,      // unlock
    Search,        // search
    ResizeFull,    // maximize-2
    ResizeSmall,   // minimize-2
    Close,         // x
    Layout,        // grid
    Link,          // link-2
    CmeGlobexLogo, // globe
    Star,          // star (outline)
    StarFilled,    // star (filled)
    Return,        // corner-down-left
    Popout,        // maximize
    ChartOutline,  // activity
    TrashBin,      // trash-2
    Cog,           // settings
    Edit,          // edit-3
    Checkmark,     // check
    Clone,         // copy
    Sort,          // filter
    SortDesc,      // chevron-down
    SortAsc,       // chevron-up
    DragHandle,    // more-vertical
    Folder,        // folder
    ExternalLink,  // external-link
    // ── Replay controls ──────────────────────────────────────────
    Play,         // play
    Pause,        // pause
    Stop,         // stop-circle
    SkipForward,  // skip-forward
    SkipBackward, // skip-back
    Replay,       // rotate-ccw
    // ── Drawing tools ────────────────────────────────────────────
    DrawCursor,         // mouse-pointer
    DrawLine,           // custom: diagonal line with endpoint dots
    DrawRay,            // custom: line from origin dot
    DrawExtendedLine,   // custom: edge-to-edge diagonal
    DrawHLine,          // minus
    DrawVLine,          // custom: vertical line
    DrawTrendLine,      // trending-up
    DrawFibRetracement, // custom: horizontal levels + left bracket
    DrawFibExtension,   // custom: horizontal levels + right bracket
    DrawChannel,        // custom: two parallel diagonal lines
    DrawRectangle,      // square
    DrawEllipse,        // circle
    DrawText,           // type
    DrawPriceLabel,     // tag
    DrawArrow,          // arrow-up-right
    DrawPriceRange,     // bar-chart-2
    DrawDateRange,      // calendar
    // ── UI chrome ────────────────────────────────────────────────
    ExpandRight, // chevron-right
    SnapOn,      // zap
    SnapOff,     // zap-off
    ChevronUp,   // chevron-up
    ChevronDown, // chevron-down
    // ── Window controls (system font) ────────────────────────────
    Minimize,
}

impl Icon {
    /// Whether this icon uses the default system font instead of the
    /// custom icon font.
    pub fn uses_default_font(self) -> bool {
        matches!(self, Icon::Minimize)
    }
}

impl From<Icon> for char {
    fn from(icon: Icon) -> Self {
        match icon {
            // ── General UI (feather icons) ────────────────────────
            Icon::Locked => '\u{E800}',        // lock
            Icon::Unlocked => '\u{E801}',      // unlock
            Icon::Search => '\u{E802}',        // search
            Icon::ResizeFull => '\u{E803}',    // maximize-2
            Icon::ResizeSmall => '\u{E804}',   // minimize-2
            Icon::Close => '\u{E805}',         // x
            Icon::Layout => '\u{E806}',        // grid
            Icon::Link => '\u{E807}',          // link-2
            Icon::CmeGlobexLogo => '\u{E808}', // globe
            Icon::Star => '\u{E809}',          // star
            Icon::StarFilled => '\u{E80A}',    // star-filled
            Icon::Return => '\u{E80B}',        // corner-down-left
            Icon::Popout => '\u{E80C}',        // maximize
            Icon::ChartOutline => '\u{E80D}',  // activity
            Icon::TrashBin => '\u{E80E}',      // trash-2
            Icon::Cog => '\u{E80F}',           // settings
            Icon::Edit => '\u{E810}',          // edit-3
            Icon::Checkmark => '\u{E811}',     // check
            Icon::Clone => '\u{E812}',         // copy
            Icon::Sort => '\u{E813}',          // filter
            Icon::SortDesc => '\u{E814}',      // chevron-down
            Icon::SortAsc => '\u{E815}',       // chevron-up
            Icon::DragHandle => '\u{E816}',    // more-vertical
            Icon::Folder => '\u{E817}',        // folder
            Icon::ExternalLink => '\u{E818}',  // external-link
            // ── Replay controls ──────────────────────────────────
            Icon::Play => '\u{E819}',         // play
            Icon::Pause => '\u{E81A}',        // pause
            Icon::Stop => '\u{E81B}',         // stop-circle
            Icon::SkipForward => '\u{E81C}',  // skip-forward
            Icon::SkipBackward => '\u{E81D}', // skip-back
            Icon::Replay => '\u{E81E}',       // rotate-ccw
            // ── Drawing tools ────────────────────────────────────
            Icon::DrawCursor => '\u{E81F}',         // mouse-pointer
            Icon::DrawLine => '\u{E820}',           // custom line with endpoints
            Icon::DrawRay => '\u{E821}',            // custom ray from origin
            Icon::DrawExtendedLine => '\u{E822}',   // custom edge-to-edge
            Icon::DrawHLine => '\u{E823}',          // minus
            Icon::DrawVLine => '\u{E824}',          // custom vertical line
            Icon::DrawTrendLine => '\u{E826}',      // trending-up
            Icon::DrawFibRetracement => '\u{E827}', // custom levels + left
            Icon::DrawFibExtension => '\u{E828}',   // custom levels + right
            Icon::DrawChannel => '\u{E829}',        // custom parallel lines
            Icon::DrawRectangle => '\u{E82A}',      // square
            Icon::DrawEllipse => '\u{E82B}',        // circle
            Icon::DrawText => '\u{E82C}',           // type
            Icon::DrawPriceLabel => '\u{E82D}',     // tag
            Icon::DrawArrow => '\u{E82E}',          // arrow-up-right
            Icon::DrawPriceRange => '\u{E82F}',     // bar-chart-2
            Icon::DrawDateRange => '\u{E830}',      // calendar
            // ── UI chrome ────────────────────────────────────────
            Icon::ExpandRight => '\u{E831}', // chevron-right
            Icon::SnapOn => '\u{E833}',      // zap
            Icon::SnapOff => '\u{E834}',     // zap-off
            Icon::ChevronUp => '\u{E815}',   // chevron-up (same as SortAsc)
            Icon::ChevronDown => '\u{E814}', // chevron-down (same as SortDesc)
            // ── Window controls (system font) ────────────────────
            Icon::Minimize => '\u{2013}', // EN DASH
        }
    }
}

pub fn icon_text<'a>(icon: Icon, size: u16) -> Text<'a, Theme, Renderer> {
    if icon.uses_default_font() {
        iced::widget::text(char::from(icon).to_string()).size(iced::Pixels(size.into()))
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
