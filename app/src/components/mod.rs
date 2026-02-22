// ── Component Library ─────────────────────────────────────────────────
//
// Reusable UI components for the Kairos application, organized
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
//   status_badge_themed   Theme-aware dot + label row
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

pub mod chrome;
pub mod display;
pub mod form;
pub mod input;
pub mod layout;
pub mod overlay;
pub mod primitives;

// ── Display re-exports ────────────────────────────────────────────────

pub use display::status_dot::status_badge_themed;

// ── Form re-exports ───────────────────────────────────────────────────


// ── Input re-exports ──────────────────────────────────────────────────


// ── Layout re-exports ─────────────────────────────────────────────────


// ── Overlay re-exports ────────────────────────────────────────────────


// ── Primitive re-exports ──────────────────────────────────────────────

