#![allow(unused_imports)]
// ── Component Library ─────────────────────────────────────────────────
//
// Reusable UI components for the Flowsurface application, organized
// into six categories. Each sub-module groups related components by
// purpose; the top-level re-exports below provide a flat, ergonomic
// API so consumers can write:
//
//   use crate::components::{body, Icon, tooltip, Card, FormModalBuilder};
//
// ── Available Components ──────────────────────────────────────────────
//
// DISPLAY — read-only status and feedback
//   EmptyStateBuilder     Icon + message placeholder for empty views
//   KeyValueBuilder       "Label: Value" display row
//   loading_status_display  Renders a LoadingStatus enum as text
//   ProgressBarBuilder    Horizontal progress bar with optional label
//   status_dot            Small colored circle indicator
//   status_badge          Dot + label row
//   status_row            Dot + label + optional detail text
//   Toast / toast::Manager  Notification toasts with auto-dismiss
//   Notification          Toast notification kind (Error/Info/Warn)
//   tooltip               Themed tooltip wrapper
//   tooltip_with_delay    Tooltip with custom delay
//   button_with_tooltip   Button with integrated tooltip
//   TooltipPosition       Re-export of iced tooltip position
//   DEFAULT_TOOLTIP_DELAY Default 500ms tooltip delay
//
// FORM — form structure and grouping
//   FormFieldBuilder      Label + control + error/tooltip wrapper
//   form_row              Quick horizontal label:control layout
//   FormSectionBuilder    Groups fields under a section header
//
// INPUT — interactive controls
//   CheckboxFieldBuilder  Checkbox with tooltip support
//   color_picker          HSVA color picker with sat/val grid + hue
//   combo_select          Labeled combo-box style pick list
//   DropdownBuilder       Labeled dropdown selector (builder)
//   link_group_button     Pane link-group indicator button
//   multi_select          Multi-selection checkbox column
//   NumericFieldBuilder   Centered numeric text input
//   RadioGroupBuilder     Mutually exclusive radio group
//   radio_group::Direction  Row | Column layout direction
//   SearchFieldBuilder    Text input with search icon + clear
//   SecureFieldBuilder    Masked input for passwords/keys
//   SliderFieldBuilder    Labeled slider with formatted value
//   labeled_slider        Convenience slider function
//   classic_slider_row    Slider in standard row layout
//   StepperBuilder        [-] value [+] increment control
//   TextFieldBuilder      Text input with label + validation
//   ToggleButtonBuilder   Pressable on/off button
//   toggle_switch         Simple labeled toggle/switch
//   volume_trackbar       Canvas seek bar with volume histogram
//
// LAYOUT — structural containers and arrangement
//   button_grid           Grid of mutually exclusive toggle buttons
//   ButtonGroupBuilder    Tab or segmented button row
//   CardBuilder / CardKind  Themed container card
//   collapsible           Expandable/collapsible section
//   decorate              Low-level widget decorator (from Halloy)
//   dragger_row           Row with drag handle indicator
//   InteractiveCardBuilder  Clickable card with selection accent
//   ListItemBuilder       Selectable list row with leading/trailing
//   MultiSplit            Resizable vertical panel splitter
//   DRAG_SIZE             MultiSplit drag handle size constant
//   reorderable_list      Drag-to-reorder column widget
//   DragEvent             Reorder drag event enum
//   scrollable_content    Vertical scrollable wrapper
//   SectionHeaderBuilder  Section title with optional divider
//   split_section         Column with divider rules between items
//   split_column!         Macro for split_section shorthand
//   toolbar / ToolbarItem Horizontal toolbar with separators
//
// OVERLAY — modals, menus, and dialogs
//   ConfirmDialog         Confirmation dialog data type
//   ConfirmDialogBuilder  Confirmation dialog with confirm/cancel
//   context_menu          Right-click positioned context menu
//   DropdownMenuBuilder   Positioned dropdown overlay
//   FormModalBuilder      Form dialog with Save/Cancel
//   ModalShell / ModalKind  Composable modal container
//   chart_modal           ModalShell preset for chart context
//   dashboard_modal       ModalShell preset for dashboard context
//
// PRIMITIVES — atoms and foundational elements
//   badge / BadgeKind     Pill-shaped colored status badge
//   icon_button / IconButtonBuilder  Icon-only button with tooltip
//   toolbar_icon          Small icon button preset for toolbars
//   Icon                  Icon enum (~50 Feather + custom glyphs)
//   icon_text             Icon rendered as a Text element
//   exchange_icon         Maps FuturesVenue to Icon
//   ICONS_FONT            Custom icon font reference
//   ICONS_BYTES           Raw icon TTF font bytes
//   AZERET_MONO           Monospace font reference
//   AZERET_MONO_BYTES     Raw monospace TTF font bytes
//   heading               Text at 16px (modal headings)
//   title                 Text at 14px (dialog titles)
//   label_text            Text at 13px (form labels)
//   body                  Text at 12px (default UI text)
//   small                 Text at 11px (chart labels)
//   tiny                  Text at 10px (badges)
//   mono                  Text at 11px monospace
//   colored               Text at 12px with custom color
//   divider               Thin 1px horizontal rule
//   thick_divider         Thick 2px horizontal rule
//   vertical_divider      Thin 1px vertical rule
//   flex_space            Fill-width shrink-height spacer
//   truncated             Clipped text without wrapping
//
// ──────────────────────────────────────────────────────────────────────

// ── Sub-modules ───────────────────────────────────────────────────────

pub mod display;
pub mod form;
pub mod input;
pub mod layout;
pub mod overlay;
pub mod primitives;

// ── Display re-exports ────────────────────────────────────────────────

pub use display::empty_state::EmptyStateBuilder;
pub use display::key_value::KeyValueBuilder;
pub use display::loading_status::loading_status_display;
pub use display::progress_bar::ProgressBarBuilder;
pub use display::status_dot::{status_badge, status_dot, status_row};
pub use display::toast::{self, Notification, Toast};
pub use display::tooltip::{
    DEFAULT_TOOLTIP_DELAY, TooltipPosition, button_with_tooltip, tooltip, tooltip_with_delay,
};

// ── Form re-exports ───────────────────────────────────────────────────

pub use form::form_field::FormFieldBuilder;
pub use form::form_row::form_row;
pub use form::form_section::FormSectionBuilder;

// ── Input re-exports ──────────────────────────────────────────────────

pub use input::checkbox_field::CheckboxFieldBuilder;
pub use input::color_picker::color_picker;
pub use input::combo_select::combo_select;
pub use input::dropdown::DropdownBuilder;
pub use input::link_group_button::link_group_button;
pub use input::multi_select::multi_select;
pub use input::numeric_field::NumericFieldBuilder;
pub use input::radio_group::RadioGroupBuilder;
pub use input::search_field::SearchFieldBuilder;
pub use input::secure_field::SecureFieldBuilder;
pub use input::slider_field::{SliderFieldBuilder, classic_slider_row, labeled_slider};
pub use input::stepper::StepperBuilder;
pub use input::text_field::TextFieldBuilder;
pub use input::toggle_button::ToggleButtonBuilder;
pub use input::toggle_switch::toggle_switch;
pub use input::volume_trackbar::volume_trackbar;

// ── Layout re-exports ─────────────────────────────────────────────────

pub use layout::button_grid::button_grid;
pub use layout::button_group::ButtonGroupBuilder;
pub use layout::card::{CardBuilder, CardKind};
pub use layout::collapsible::collapsible;
pub use layout::decorate::decorate;
pub use layout::dragger_row::dragger_row;
pub use layout::interactive_card::InteractiveCardBuilder;
pub use layout::list_item::ListItemBuilder;
pub use layout::multi_split::{DRAG_SIZE, MultiSplit};
pub use layout::reorderable_list::{self as reorderable_list, DragEvent};
pub use layout::scrollable_content::scrollable_content;
pub use layout::section_header::SectionHeaderBuilder;
pub use layout::split_section::split_section;
pub use layout::toolbar::{ToolbarItem, toolbar};

// ── Overlay re-exports ────────────────────────────────────────────────

pub use overlay::confirm_dialog::{ConfirmDialog, ConfirmDialogBuilder};
pub use overlay::context_menu::context_menu;
pub use overlay::dropdown_menu::DropdownMenuBuilder;
pub use overlay::form_modal::FormModalBuilder;
pub use overlay::modal_shell::{ModalKind, ModalShell, chart_modal, dashboard_modal};

// ── Primitive re-exports ──────────────────────────────────────────────

pub use primitives::badge::{BadgeKind, badge};
pub use primitives::icon_button::{IconButtonBuilder, icon_button, toolbar_icon};
pub use primitives::icons::{
    AZERET_MONO, AZERET_MONO_BYTES, ICONS_BYTES, ICONS_FONT, Icon, exchange_icon, icon_text,
};
pub use primitives::label::{body, colored, heading, label_text, mono, small, tiny, title};
pub use primitives::separator::{divider, flex_space, thick_divider, vertical_divider};
pub use primitives::truncated_text::truncated;
