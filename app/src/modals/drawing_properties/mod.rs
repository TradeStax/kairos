//! Drawing Properties Modal
//!
//! A modal dialog for editing all properties of a chart drawing.
//! Supports all 16 drawing types with type-specific sections.

mod helpers;
mod view;

use data::{
    CalcMode, DrawingId, DrawingStyle, DrawingTool, FibonacciConfig, FuturesTickerInfo,
    LabelAlignment, LineStyle, PositionCalcConfig, SerializableColor, SerializableDrawing,
    VbpDrawingConfig,
};
use palette::Hsva;
use study::Study as _;

// ── State ─────────────────────────────────────────────────────────────

/// Which color picker is currently open.
#[derive(Debug, Clone, PartialEq)]
pub enum PickerKind {
    LineColor,
    FillColor,
    TpColor,
    SlColor,
}

/// Active tab in the properties modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Style,
    Levels,       // Fibonacci only
    Position,     // Calculator only
    Labels,       // Calculator only
    Display,
    Vbp(study::ParameterTab), // Dynamic VBP tab
}

/// The drawing properties modal state.
///
/// # VBP State Sync Pattern
///
/// For VolumeProfile drawings, this modal holds three VBP-related
/// fields that must stay in sync:
///
/// - `vbp_config` — the source of truth for pending parameter
///   values. Mutated directly by `VbpParamChanged` /
///   `VbpColorChanged` / `VbpLineStyleChanged` messages.
/// - `vbp_params` — a snapshot of `VbpStudy::parameters()`
///   used by the view layer to iterate and render widgets.
///   Rebuilt from a temporary `VbpStudy` after config changes
///   via `refresh_vbp_params()`.
/// - `vbp_tabs` — the ordered list of visible parameter tabs,
///   derived from the study's `tab_labels()` and filtered to
///   exclude tabs that only contain hidden (period) keys.
///   Also rebuilt by `refresh_vbp_params()`.
///
/// After every VBP config mutation, call `refresh_vbp_params()`
/// to keep `vbp_params` and `vbp_tabs` consistent with
/// `vbp_config`.
#[derive(Debug, Clone)]
pub struct DrawingPropertiesModal {
    // NOTE: PartialEq implemented manually below (VBP fields
    // excluded)
    pub(super) drawing_id: DrawingId,
    pub(super) tool: DrawingTool,
    pub(super) active_tab: Tab,
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
    // Position calculator fields
    pub(super) position_calc: Option<PositionCalcConfig>,
    pub(super) editing_target_color: Option<Hsva>,
    pub(super) editing_stop_color: Option<Hsva>,
    pub(super) hex_input_target: Option<String>,
    pub(super) hex_input_stop: Option<String>,
    pub(super) ticker_info: Option<FuturesTickerInfo>,
    // Meta fields
    pub(super) locked: bool,
    pub(super) visible: bool,
    pub(super) label: Option<String>,
    // Snapshot & original for live preview + undo
    pub(super) before_snapshot: SerializableDrawing,
    pub(super) original: DrawingUpdate,
    // UI state — single active picker replaces four booleans
    pub(super) active_picker: Option<PickerKind>,
    pub(super) editing_stroke_color: Option<Hsva>,
    pub(super) editing_fill_color: Option<Hsva>,
    pub(super) hex_input_stroke: Option<String>,
    pub(super) hex_input_fill: Option<String>,
    // VBP drawing state (see doc comment above for sync pattern)
    pub(super) vbp_config: Option<study::StudyConfig>,
    pub(super) vbp_params: Option<Vec<study::ParameterDef>>,
    pub(super) vbp_tabs: Vec<study::ParameterTab>,
    pub(super) editing_vbp_color_key: Option<String>,
    pub(super) editing_vbp_color_hsva: Option<Hsva>,
}

impl PartialEq for DrawingPropertiesModal {
    fn eq(&self, other: &Self) -> bool {
        self.drawing_id == other.drawing_id
            && self.tool == other.tool
            && self.active_tab == other.active_tab
    }
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
        ticker_info: Option<FuturesTickerInfo>,
    ) -> Self {
        let original = DrawingUpdate {
            style: style.clone(),
            locked,
            visible,
            label: label.clone(),
        };
        // Initialize VBP config from saved params.
        // Uses the study's tab_labels() for tab ordering
        // rather than re-discovering from parameter metadata.
        let (vbp_config, vbp_params, vbp_tabs) =
            if tool == DrawingTool::VolumeProfile {
                let mut tmp = study::orderflow::VbpStudy::new();
                if let Some(ref cfg) = style.vbp_config {
                    tmp.import_config(&cfg.params);
                }
                let params = tmp.parameters().to_vec();
                let config = tmp.config().clone();
                let tabs =
                    vbp_tabs_from_study(&tmp, &params);
                (Some(config), Some(params), tabs)
            } else {
                (None, None, Vec::new())
            };

        let initial_tab = if matches!(
            tool,
            DrawingTool::BuyCalculator | DrawingTool::SellCalculator
        ) {
            Tab::Position
        } else if tool == DrawingTool::VolumeProfile {
            vbp_tabs
                .first()
                .map(|t| Tab::Vbp(*t))
                .unwrap_or(Tab::Display)
        } else {
            Tab::Style
        };

        Self {
            drawing_id,
            tool,
            active_tab: initial_tab,
            stroke_color: style.stroke_color,
            stroke_width: style.stroke_width,
            line_style: style.line_style,
            fill_color: style.fill_color,
            fill_opacity: style.fill_opacity,
            show_labels: style.show_labels,
            label_alignment: style.label_alignment,
            text: style.text.clone(),
            fibonacci: style.fibonacci.clone(),
            position_calc: style.position_calc.clone(),
            editing_target_color: None,
            editing_stop_color: None,
            hex_input_target: None,
            hex_input_stop: None,
            ticker_info,
            locked,
            visible,
            label,
            before_snapshot: snapshot,
            original,
            active_picker: None,
            editing_stroke_color: None,
            editing_fill_color: None,
            hex_input_stroke: None,
            hex_input_fill: None,
            vbp_config,
            vbp_params,
            vbp_tabs,
            editing_vbp_color_key: None,
            editing_vbp_color_hsva: None,
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
        let vbp_config = self.vbp_config.as_ref().map(|config| {
            VbpDrawingConfig {
                params: serde_json::to_value(config).unwrap_or_default(),
            }
        });
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
                position_calc: self.position_calc.clone(),
                vbp_config,
            },
            locked: self.locked,
            visible: self.visible,
            label: self.label.clone(),
        }
    }

    /// Rebuild `vbp_params` and `vbp_tabs` from a temporary
    /// `VbpStudy` seeded with the current `vbp_config`.
    ///
    /// Call this after any mutation to `vbp_config` so that the
    /// view's parameter widgets and tab bar reflect the latest
    /// state (e.g. conditional visibility may change).
    fn refresh_vbp_params(&mut self) {
        let Some(ref config) = self.vbp_config else {
            return;
        };
        let exported =
            serde_json::to_value(config).unwrap_or_default();
        let mut tmp = study::orderflow::VbpStudy::new();
        tmp.import_config(&exported);
        let params = tmp.parameters().to_vec();
        let tabs = vbp_tabs_from_study(&tmp, &params);
        self.vbp_params = Some(params);
        self.vbp_tabs = tabs;
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

    fn has_position_calc(&self) -> bool {
        matches!(
            self.tool,
            DrawingTool::BuyCalculator | DrawingTool::SellCalculator
        )
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

    pub(super) fn available_tabs(&self) -> Vec<Tab> {
        if self.tool == DrawingTool::VolumeProfile {
            let mut tabs: Vec<Tab> =
                self.vbp_tabs.iter().map(|t| Tab::Vbp(*t)).collect();
            tabs.push(Tab::Display);
            tabs
        } else if self.has_position_calc() {
            vec![Tab::Position, Tab::Style, Tab::Labels]
        } else if self.has_fibonacci() {
            vec![Tab::Style, Tab::Levels, Tab::Display]
        } else {
            vec![Tab::Style, Tab::Display]
        }
    }

    pub fn update(&mut self, message: Message) -> Option<Action> {
        match message {
            Message::SwitchTab(tab) => {
                self.active_tab = tab;
                self.active_picker = None;
            }
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
            Message::CalcQuantityChanged(q) => {
                if let Some(ref mut calc) = self.position_calc {
                    calc.quantity = q.clamp(1, 999);
                }
            }
            Message::CalcTargetColorChanged(hsva) => {
                self.hex_input_target = None;
                self.editing_target_color = Some(hsva);
                if let Some(ref mut calc) = self.position_calc {
                    calc.target_color = data::config::theme::hsva_to_rgba(hsva);
                }
            }
            Message::CalcStopColorChanged(hsva) => {
                self.hex_input_stop = None;
                self.editing_stop_color = Some(hsva);
                if let Some(ref mut calc) = self.position_calc {
                    calc.stop_color = data::config::theme::hsva_to_rgba(hsva);
                }
            }
            Message::CalcTargetOpacityChanged(o) => {
                if let Some(ref mut calc) = self.position_calc {
                    calc.target_opacity = o;
                }
            }
            Message::CalcStopOpacityChanged(o) => {
                if let Some(ref mut calc) = self.position_calc {
                    calc.stop_opacity = o;
                }
            }
            Message::CalcLabelFontSizeChanged(s) => {
                if let Some(ref mut calc) = self.position_calc {
                    calc.label_font_size = s;
                }
            }
            Message::CalcShowTargetLabelToggled(v) => {
                if let Some(ref mut calc) = self.position_calc {
                    calc.show_target_label = v;
                }
            }
            Message::CalcShowEntryLabelToggled(v) => {
                if let Some(ref mut calc) = self.position_calc {
                    calc.show_entry_label = v;
                }
            }
            Message::CalcShowStopLabelToggled(v) => {
                if let Some(ref mut calc) = self.position_calc {
                    calc.show_stop_label = v;
                }
            }
            Message::CalcShowPnlToggled(v) => {
                if let Some(ref mut calc) = self.position_calc {
                    calc.show_pnl = v;
                }
            }
            Message::CalcShowTicksToggled(v) => {
                if let Some(ref mut calc) = self.position_calc {
                    calc.show_ticks = v;
                }
            }
            Message::CalcTargetModeChanged(mode) => {
                if let Some(ref mut calc) = self.position_calc {
                    calc.target_mode = mode;
                    if mode != CalcMode::Free && calc.target_value == 0.0 {
                        calc.target_value = match mode {
                            CalcMode::Ticks => 10.0,
                            CalcMode::Money => 500.0,
                            CalcMode::Free => 0.0,
                        };
                    }
                }
            }
            Message::CalcStopModeChanged(mode) => {
                if let Some(ref mut calc) = self.position_calc {
                    calc.stop_mode = mode;
                    if mode != CalcMode::Free && calc.stop_value == 0.0 {
                        calc.stop_value = match mode {
                            CalcMode::Ticks => 10.0,
                            CalcMode::Money => 500.0,
                            CalcMode::Free => 0.0,
                        };
                    }
                }
            }
            Message::CalcTargetValueChanged(s) => {
                if let Some(ref mut calc) = self.position_calc
                    && let Ok(v) = s.parse::<f64>()
                {
                    calc.target_value = v.max(0.0);
                }
            }
            Message::CalcStopValueChanged(s) => {
                if let Some(ref mut calc) = self.position_calc
                    && let Ok(v) = s.parse::<f64>()
                {
                    calc.stop_value = v.max(0.0);
                }
            }
            Message::CalcTargetHexInput(input) => {
                if let Some(ref mut calc) = self.position_calc
                    && let Some(rgba) =
                        data::config::theme::hex_to_rgba_safe(&input)
                {
                    calc.target_color = rgba;
                    self.editing_target_color =
                        Some(data::config::theme::rgba_to_hsva(rgba));
                }
                self.hex_input_target = Some(input);
            }
            Message::CalcStopHexInput(input) => {
                if let Some(ref mut calc) = self.position_calc
                    && let Some(rgba) =
                        data::config::theme::hex_to_rgba_safe(&input)
                {
                    calc.stop_color = rgba;
                    self.editing_stop_color =
                        Some(data::config::theme::rgba_to_hsva(rgba));
                }
                self.hex_input_stop = Some(input);
            }
            Message::CalcResetColorsToDefault => {
                if let Some(ref mut calc) = self.position_calc {
                    calc.target_color = PositionCalcConfig::DEFAULT_TARGET_COLOR;
                    calc.stop_color = PositionCalcConfig::DEFAULT_STOP_COLOR;
                    self.editing_target_color = None;
                    self.editing_stop_color = None;
                    self.hex_input_target = None;
                    self.hex_input_stop = None;
                }
            }
            Message::ToggleTargetColorPicker => {
                self.active_picker = if self.active_picker == Some(PickerKind::TpColor) {
                    None
                } else {
                    Some(PickerKind::TpColor)
                };
            }
            Message::ToggleStopColorPicker => {
                self.active_picker = if self.active_picker == Some(PickerKind::SlColor) {
                    None
                } else {
                    Some(PickerKind::SlColor)
                };
            }
            Message::ToggleStrokePicker => {
                self.active_picker = if self.active_picker == Some(PickerKind::LineColor) {
                    None
                } else {
                    Some(PickerKind::LineColor)
                };
            }
            Message::ToggleFillPicker => {
                self.active_picker = if self.active_picker == Some(PickerKind::FillColor) {
                    None
                } else {
                    Some(PickerKind::FillColor)
                };
            }
            Message::DismissColorPicker => {
                self.active_picker = None;
            }
            Message::VbpParamChanged(key, value) => {
                if let Some(ref mut config) = self.vbp_config {
                    config.set(key, value);
                    self.refresh_vbp_params();
                }
            }
            Message::VbpColorChanged(key, hsva) => {
                self.editing_vbp_color_hsva = Some(hsva);
                let rgba = data::config::theme::hsva_to_rgba(hsva);
                if let Some(ref mut config) = self.vbp_config {
                    config.set(
                        key,
                        study::ParameterValue::Color(rgba),
                    );
                    self.refresh_vbp_params();
                }
            }
            Message::VbpEditColor(key) => {
                if self.editing_vbp_color_key.as_deref() == Some(&key)
                {
                    // Toggle off
                    self.editing_vbp_color_key = None;
                    self.editing_vbp_color_hsva = None;
                } else {
                    self.editing_vbp_color_key = Some(key);
                    self.editing_vbp_color_hsva = None;
                }
            }
            Message::VbpLineStyleChanged(key, value) => {
                if let Some(ref mut config) = self.vbp_config {
                    config.set(
                        key,
                        study::ParameterValue::LineStyle(value),
                    );
                    self.refresh_vbp_params();
                }
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
    // Tab navigation
    SwitchTab(Tab),
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
    // Position calculator
    CalcQuantityChanged(u32),
    CalcTargetModeChanged(CalcMode),
    CalcStopModeChanged(CalcMode),
    CalcTargetValueChanged(String),
    CalcStopValueChanged(String),
    CalcTargetColorChanged(Hsva),
    CalcStopColorChanged(Hsva),
    CalcTargetHexInput(String),
    CalcStopHexInput(String),
    CalcTargetOpacityChanged(f32),
    CalcStopOpacityChanged(f32),
    CalcLabelFontSizeChanged(f32),
    CalcShowTargetLabelToggled(bool),
    CalcShowEntryLabelToggled(bool),
    CalcShowStopLabelToggled(bool),
    CalcShowPnlToggled(bool),
    CalcShowTicksToggled(bool),
    CalcResetColorsToDefault,
    ToggleTargetColorPicker,
    ToggleStopColorPicker,
    // Color picker
    ToggleStrokePicker,
    ToggleFillPicker,
    DismissColorPicker,
    // VBP
    VbpParamChanged(String, study::ParameterValue),
    VbpColorChanged(String, Hsva),
    VbpEditColor(String),
    VbpLineStyleChanged(String, study::config::LineStyleValue),
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

/// Period-related keys are controlled by the drawing's anchors;
/// hide them from the properties modal.
const HIDDEN_KEYS: &[&str] = &[
    "period",
    "length_unit",
    "length_value",
    "custom_start",
    "custom_end",
];

/// Build the ordered list of visible VBP tabs.
///
/// Uses the study's `tab_labels()` for canonical ordering, then
/// filters out any tab whose only parameters are hidden (period)
/// keys.
fn vbp_tabs_from_study(
    study: &dyn study::Study,
    params: &[study::ParameterDef],
) -> Vec<study::ParameterTab> {
    // Collect tabs that have at least one non-hidden parameter
    let has_visible_param =
        |tab: study::ParameterTab| -> bool {
            params.iter().any(|p| {
                p.tab == tab
                    && !HIDDEN_KEYS
                        .contains(&p.key.as_str())
            })
        };

    // Prefer study-defined tab order when available
    if let Some(labels) = study.tab_labels() {
        labels
            .iter()
            .map(|(_, tab)| *tab)
            .filter(|t| has_visible_param(*t))
            .collect()
    } else {
        // Fallback: discover from parameter metadata
        let mut tabs: Vec<study::ParameterTab> = Vec::new();
        for p in params {
            if HIDDEN_KEYS.contains(&p.key.as_str()) {
                continue;
            }
            if !tabs.contains(&p.tab) {
                tabs.push(p.tab);
            }
        }
        tabs
    }
}
