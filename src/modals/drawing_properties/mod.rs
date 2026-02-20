//! Drawing Properties Modal
//!
//! A modal dialog for editing all properties of a chart drawing.
//! Supports all 16 drawing types with type-specific sections.

mod helpers;
mod view;

use data::{
    DrawingId, DrawingStyle, DrawingTool, FibonacciConfig, LabelAlignment, LineStyle,
    SerializableColor, SerializableDrawing,
};
use palette::Hsva;

// ── State ─────────────────────────────────────────────────────────────

/// The drawing properties modal state.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawingPropertiesModal {
    pub(super) drawing_id: DrawingId,
    pub(super) tool: DrawingTool,
    // Editable style fields
    pub(super) stroke_color: SerializableColor,
    pub(super) stroke_width: f32,
    pub(super) line_style: LineStyle,
    pub(super) fill_color: Option<SerializableColor>,
    pub(super) fill_opacity: f32,
    pub(super) show_labels: bool,
    pub(super) label_alignment: LabelAlignment,
    pub(super) text: Option<String>,
    pub(super) fibonacci: Option<FibonacciConfig>,
    // Meta fields
    pub(super) locked: bool,
    pub(super) visible: bool,
    pub(super) label: Option<String>,
    // Snapshot & original for live preview + undo
    pub(super) before_snapshot: SerializableDrawing,
    pub(super) original: DrawingUpdate,
    // UI state
    pub(super) editing_stroke_color: Option<Hsva>,
    pub(super) editing_fill_color: Option<Hsva>,
    pub(super) hex_input_stroke: Option<String>,
    pub(super) hex_input_fill: Option<String>,
    pub(super) show_stroke_picker: bool,
    pub(super) show_fill_picker: bool,
}

impl DrawingPropertiesModal {
    /// Create a new properties modal from a drawing's current state.
    pub fn new(
        drawing_id: DrawingId,
        tool: DrawingTool,
        style: &DrawingStyle,
        locked: bool,
        visible: bool,
        label: Option<String>,
        snapshot: SerializableDrawing,
    ) -> Self {
        let original = DrawingUpdate {
            style: style.clone(),
            locked,
            visible,
            label: label.clone(),
        };
        Self {
            drawing_id,
            tool,
            stroke_color: style.stroke_color,
            stroke_width: style.stroke_width,
            line_style: style.line_style,
            fill_color: style.fill_color,
            fill_opacity: style.fill_opacity,
            show_labels: style.show_labels,
            label_alignment: style.label_alignment,
            text: style.text.clone(),
            fibonacci: style.fibonacci.clone(),
            locked,
            visible,
            label,
            before_snapshot: snapshot,
            original,
            editing_stroke_color: None,
            editing_fill_color: None,
            hex_input_stroke: None,
            hex_input_fill: None,
            show_stroke_picker: false,
            show_fill_picker: false,
        }
    }

    /// The drawing ID this modal is editing.
    pub fn drawing_id(&self) -> DrawingId {
        self.drawing_id
    }

    /// The full drawing snapshot captured before any edits (for undo).
    pub fn before_snapshot(&self) -> &SerializableDrawing {
        &self.before_snapshot
    }

    /// Build the `DrawingUpdate` from current modal state.
    pub fn build_update(&self) -> DrawingUpdate {
        DrawingUpdate {
            style: DrawingStyle {
                stroke_color: self.stroke_color,
                stroke_width: self.stroke_width,
                line_style: self.line_style,
                fill_color: self.fill_color,
                fill_opacity: self.fill_opacity,
                show_labels: self.show_labels,
                label_alignment: self.label_alignment,
                fibonacci: self.fibonacci.clone(),
                text: self.text.clone(),
            },
            locked: self.locked,
            visible: self.visible,
            label: self.label.clone(),
        }
    }

    fn has_fill(&self) -> bool {
        matches!(
            self.tool,
            DrawingTool::Rectangle | DrawingTool::Ellipse | DrawingTool::ParallelChannel
        )
    }

    fn has_fibonacci(&self) -> bool {
        matches!(
            self.tool,
            DrawingTool::FibRetracement | DrawingTool::FibExtension
        )
    }

    fn has_text(&self) -> bool {
        matches!(self.tool, DrawingTool::TextLabel)
    }

    fn has_labels(&self) -> bool {
        !matches!(self.tool, DrawingTool::TextLabel)
    }

    fn has_label_input(&self) -> bool {
        matches!(
            self.tool,
            DrawingTool::Line
                | DrawingTool::Ray
                | DrawingTool::ExtendedLine
                | DrawingTool::HorizontalLine
                | DrawingTool::VerticalLine
        )
    }

    pub fn update(&mut self, message: Message) -> Option<Action> {
        match message {
            Message::StrokeColorChanged(hsva) => {
                self.hex_input_stroke = None;
                self.editing_stroke_color = Some(hsva);
                self.stroke_color = data::config::theme::hsva_to_rgba(hsva);
            }
            Message::StrokeHexInput(input) => {
                if let Some(rgba) = data::config::theme::hex_to_rgba_safe(&input) {
                    self.stroke_color = rgba;
                    self.editing_stroke_color = Some(data::config::theme::rgba_to_hsva(rgba));
                }
                self.hex_input_stroke = Some(input);
            }
            Message::StrokeWidthChanged(w) => {
                self.stroke_width = w;
            }
            Message::LineStyleChanged(ls) => {
                self.line_style = ls;
            }
            Message::FillEnabled(enabled) => {
                if enabled && self.fill_color.is_none() {
                    self.fill_color = Some(SerializableColor::new(0.3, 0.6, 1.0, 1.0));
                } else if !enabled {
                    self.fill_color = None;
                }
            }
            Message::FillColorChanged(hsva) => {
                self.hex_input_fill = None;
                self.editing_fill_color = Some(hsva);
                self.fill_color = Some(data::config::theme::hsva_to_rgba(hsva));
            }
            Message::FillHexInput(input) => {
                if let Some(rgba) = data::config::theme::hex_to_rgba_safe(&input) {
                    self.fill_color = Some(rgba);
                    self.editing_fill_color = Some(data::config::theme::rgba_to_hsva(rgba));
                }
                self.hex_input_fill = Some(input);
            }
            Message::FillOpacityChanged(o) => {
                self.fill_opacity = o;
            }
            Message::ShowLabelsToggled(v) => {
                self.show_labels = v;
            }
            Message::LabelAlignmentChanged(a) => {
                self.label_alignment = a;
            }
            Message::TextChanged(t) => {
                self.text = Some(t);
            }
            Message::LockedToggled(v) => {
                self.locked = v;
            }
            Message::VisibleToggled(v) => {
                self.visible = v;
            }
            Message::LabelChanged(l) => {
                self.label = if l.is_empty() { None } else { Some(l) };
            }
            Message::FibShowPricesToggled(v) => {
                if let Some(ref mut fib) = self.fibonacci {
                    fib.show_prices = v;
                }
            }
            Message::FibShowPercentagesToggled(v) => {
                if let Some(ref mut fib) = self.fibonacci {
                    fib.show_percentages = v;
                }
            }
            Message::FibExtendLinesToggled(v) => {
                if let Some(ref mut fib) = self.fibonacci {
                    fib.extend_lines = v;
                }
            }
            Message::FibLevelVisibilityToggled(idx, v) => {
                if let Some(ref mut fib) = self.fibonacci {
                    if let Some(level) = fib.levels.get_mut(idx) {
                        level.visible = v;
                    }
                }
            }
            Message::FibLevelColorChanged(idx, hsva) => {
                if let Some(ref mut fib) = self.fibonacci {
                    if let Some(level) = fib.levels.get_mut(idx) {
                        level.color = data::config::theme::hsva_to_rgba(hsva);
                    }
                }
            }
            Message::ToggleStrokePicker => {
                self.show_stroke_picker = !self.show_stroke_picker;
                self.show_fill_picker = false;
            }
            Message::ToggleFillPicker => {
                self.show_fill_picker = !self.show_fill_picker;
                self.show_stroke_picker = false;
            }
            Message::DismissColorPicker => {
                self.show_stroke_picker = false;
                self.show_fill_picker = false;
            }
            Message::Apply => {
                let update = self.build_update();
                return Some(Action::Applied(self.drawing_id, update));
            }
            Message::Close => {
                return Some(Action::Cancelled(self.drawing_id, self.original.clone()));
            }
        }
        None
    }
}

// ── Messages & Actions ────────────────────────────────────────────────

/// Messages for the drawing properties modal.
#[derive(Debug, Clone)]
pub enum Message {
    // Style
    StrokeColorChanged(Hsva),
    StrokeHexInput(String),
    StrokeWidthChanged(f32),
    LineStyleChanged(LineStyle),
    FillEnabled(bool),
    FillColorChanged(Hsva),
    FillHexInput(String),
    FillOpacityChanged(f32),
    ShowLabelsToggled(bool),
    LabelAlignmentChanged(LabelAlignment),
    TextChanged(String),
    // Meta
    LockedToggled(bool),
    VisibleToggled(bool),
    LabelChanged(String),
    // Fibonacci
    FibShowPricesToggled(bool),
    FibShowPercentagesToggled(bool),
    FibExtendLinesToggled(bool),
    FibLevelVisibilityToggled(usize, bool),
    FibLevelColorChanged(usize, Hsva),
    // Color picker
    ToggleStrokePicker,
    ToggleFillPicker,
    DismissColorPicker,
    // Actions
    Apply,
    Close,
}

/// Actions produced by the modal for the parent to handle.
pub enum Action {
    /// Apply changes and close the modal.
    Applied(DrawingId, DrawingUpdate),
    /// Cancel edits — carries original state for revert.
    Cancelled(DrawingId, DrawingUpdate),
}

/// All editable properties to apply back to a drawing.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawingUpdate {
    pub style: DrawingStyle,
    pub locked: bool,
    pub visible: bool,
    pub label: Option<String>,
}
