//! Drawing Properties Modal
//!
//! A modal dialog for editing all properties of a chart drawing.
//! Supports all 16 drawing types with type-specific sections.

use data::{
    DrawingId, DrawingStyle, DrawingTool, FibonacciConfig, LabelAlignment, LineStyle,
    SerializableColor, SerializableDrawing,
};
use iced::{
    Alignment, Color, Element, Length,
    widget::{
        button, center, column, container, mouse_area, opaque, pick_list, row, space, stack, text,
        text_input,
    },
};
use palette::Hsva;

use crate::components::form::form_section::FormSectionBuilder;
use crate::components::input::checkbox_field::CheckboxFieldBuilder;
use crate::components::input::color_picker::color_picker;
use crate::components::input::slider_field::SliderFieldBuilder;
use crate::components::primitives::icon_button::icon_button;
use crate::components::primitives::icons::Icon;
use crate::style::{self, tokens};

// ── State ─────────────────────────────────────────────────────────────

/// The drawing properties modal state.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawingPropertiesModal {
    drawing_id: DrawingId,
    tool: DrawingTool,
    // Editable style fields
    stroke_color: SerializableColor,
    stroke_width: f32,
    line_style: LineStyle,
    fill_color: Option<SerializableColor>,
    fill_opacity: f32,
    show_labels: bool,
    label_alignment: LabelAlignment,
    text: Option<String>,
    fibonacci: Option<FibonacciConfig>,
    // Meta fields
    locked: bool,
    visible: bool,
    label: Option<String>,
    // Snapshot & original for live preview + undo
    before_snapshot: SerializableDrawing,
    original: DrawingUpdate,
    // UI state
    editing_stroke_color: Option<Hsva>,
    editing_fill_color: Option<Hsva>,
    hex_input_stroke: Option<String>,
    hex_input_fill: Option<String>,
    show_stroke_picker: bool,
    show_fill_picker: bool,
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
                let color = data::config::theme::from_hsva(hsva);
                self.stroke_color = SerializableColor::from(color);
            }
            Message::StrokeHexInput(input) => {
                if let Some(color) = data::config::theme::hex_to_color(&input) {
                    self.stroke_color = SerializableColor::from(color);
                    self.editing_stroke_color = Some(data::config::theme::to_hsva(color));
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
                let color = data::config::theme::from_hsva(hsva);
                self.fill_color = Some(SerializableColor::from(color));
            }
            Message::FillHexInput(input) => {
                if let Some(color) = data::config::theme::hex_to_color(&input) {
                    self.fill_color = Some(SerializableColor::from(color));
                    self.editing_fill_color = Some(data::config::theme::to_hsva(color));
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
                        let color = data::config::theme::from_hsva(hsva);
                        level.color = SerializableColor::from(color);
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

    // ── View ───────────────────────────────────────────────────────

    pub fn view(&self) -> Element<'_, Message> {
        let header = self.header();

        let mut body = column![].spacing(tokens::spacing::LG);
        body = body.push(self.appearance_section());

        if self.has_fill() {
            body = body.push(self.fill_section());
        }

        if self.has_text() {
            body = body.push(self.text_section());
        }

        if self.has_fibonacci() {
            body = body.push(self.fibonacci_section());
        }

        body = body.push(self.options_section());

        let footer = self.footer();

        let inner = column![
            header,
            iced::widget::scrollable(body).style(style::scroll_bar),
            footer,
        ]
        .spacing(tokens::spacing::LG)
        .width(Length::Fill);

        // Color picker popup overlay
        let content: Element<'_, Message> = if self.show_stroke_picker || self.show_fill_picker {
            let popup = self.color_picker_popup();
            stack![
                mouse_area(inner).on_press(Message::DismissColorPicker),
                center(opaque(popup)),
            ]
            .into()
        } else {
            inner.into()
        };

        container(content)
            .padding(tokens::spacing::XL)
            .max_width(440.0)
            .max_height(560.0)
            .style(style::dashboard_modal)
            .into()
    }

    /// Title bar with drawing name, lock toggle, and close button.
    fn header(&self) -> Element<'_, Message> {
        let lock_icon = if self.locked {
            Icon::Locked
        } else {
            Icon::Unlocked
        };
        row![
            text(format!("{} Properties", self.tool)).size(tokens::text::HEADING),
            space::horizontal(),
            icon_button(lock_icon)
                .size(14)
                .padding(tokens::spacing::XS)
                .on_press(Message::LockedToggled(!self.locked)),
            icon_button(Icon::Close)
                .size(14)
                .padding(tokens::spacing::XS)
                .on_press(Message::Close),
        ]
        .spacing(tokens::spacing::XS)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
    }

    /// Color swatch + hex + style dropdown, width slider.
    fn appearance_section(&self) -> Element<'_, Message> {
        let stroke_iced: iced::Color = self.stroke_color.into();
        let hex_stroke = self
            .hex_input_stroke
            .as_deref()
            .unwrap_or(data::config::theme::color_to_hex(stroke_iced).as_str())
            .to_string();
        let is_hex_valid = self.hex_input_stroke.is_none()
            || self
                .hex_input_stroke
                .as_deref()
                .and_then(data::config::theme::hex_to_color)
                .is_some();

        // Color swatch + hex input + style dropdown in one row
        let color_style_row: Element<'_, Message> = row![
            text("Color").size(tokens::text::LABEL),
            color_swatch(
                stroke_iced,
                self.show_stroke_picker,
                Message::ToggleStrokePicker,
            ),
            hex_text_input(&hex_stroke, is_hex_valid, Message::StrokeHexInput,),
            space::horizontal(),
            text("Style").size(tokens::text::LABEL),
            pick_list(
                LineStyle::ALL.to_vec(),
                Some(self.line_style),
                Message::LineStyleChanged,
            )
            .width(100),
        ]
        .spacing(tokens::spacing::MD)
        .align_y(Alignment::Center)
        .into();

        let width_slider = SliderFieldBuilder::new(
            "Width",
            0.5..=5.0,
            self.stroke_width,
            Message::StrokeWidthChanged,
        )
        .step(0.5)
        .format(|v| format!("{v:.1}px"));

        FormSectionBuilder::new("Appearance")
            .push(color_style_row)
            .push(width_slider)
            .into()
    }

    /// Fill toggle, color swatch, opacity (shapes only).
    fn fill_section(&self) -> Element<'_, Message> {
        let fill_enabled = self.fill_color.is_some();

        let mut section = FormSectionBuilder::new("Fill").with_top_divider(true);

        if !fill_enabled {
            section = section.push(CheckboxFieldBuilder::new(
                "Enable Fill",
                false,
                Message::FillEnabled,
            ));
        } else {
            let fill_c = self
                .fill_color
                .unwrap_or(SerializableColor::new(0.3, 0.6, 1.0, 1.0));
            let fill_iced: iced::Color = fill_c.into();
            let hex_fill = self
                .hex_input_fill
                .as_deref()
                .unwrap_or(data::config::theme::color_to_hex(fill_iced).as_str())
                .to_string();
            let is_hex_valid = self.hex_input_fill.is_none()
                || self
                    .hex_input_fill
                    .as_deref()
                    .and_then(data::config::theme::hex_to_color)
                    .is_some();

            // Enable toggle + color swatch + hex
            let fill_row: Element<'_, Message> = row![
                iced::widget::checkbox(fill_enabled)
                    .label("Fill")
                    .on_toggle(Message::FillEnabled),
                space::horizontal(),
                color_swatch(fill_iced, self.show_fill_picker, Message::ToggleFillPicker,),
                hex_text_input(&hex_fill, is_hex_valid, Message::FillHexInput,),
            ]
            .spacing(tokens::spacing::MD)
            .align_y(Alignment::Center)
            .into();

            section = section.push(fill_row);

            section = section.push(
                SliderFieldBuilder::new(
                    "Opacity",
                    0.0..=1.0f32,
                    self.fill_opacity,
                    Message::FillOpacityChanged,
                )
                .step(0.05)
                .format(|v| format!("{:.0}%", v * 100.0)),
            );
        }

        section.into()
    }

    fn text_section(&self) -> Element<'_, Message> {
        let current_text = self.text.as_deref().unwrap_or("");

        FormSectionBuilder::new("Text")
            .with_top_divider(true)
            .push(
                text_input("Enter text...", current_text)
                    .on_input(Message::TextChanged)
                    .width(Length::Fill),
            )
            .into()
    }

    /// Fibonacci options + two-column level grid.
    fn fibonacci_section(&self) -> Element<'_, Message> {
        let fib = self.fibonacci.as_ref().cloned().unwrap_or_default();

        // Two-column option toggles
        let options_row: Element<'_, Message> = row![
            container(CheckboxFieldBuilder::new(
                "Show Prices",
                fib.show_prices,
                Message::FibShowPricesToggled,
            ))
            .width(Length::FillPortion(1)),
            container(CheckboxFieldBuilder::new(
                "Show %",
                fib.show_percentages,
                Message::FibShowPercentagesToggled,
            ))
            .width(Length::FillPortion(1)),
        ]
        .spacing(tokens::spacing::LG)
        .into();

        let extend_row = CheckboxFieldBuilder::new(
            "Extend Lines",
            fib.extend_lines,
            Message::FibExtendLinesToggled,
        );

        // Two-column level grid
        let levels = &fib.levels;
        let mid = (levels.len() + 1) / 2;
        let mut left_col = column![].spacing(tokens::spacing::XS);
        let mut right_col = column![].spacing(tokens::spacing::XS);

        for (idx, level) in levels.iter().enumerate() {
            let level_color: iced::Color = level.color.into();
            let level_label = level.label.clone();
            let level_visible = level.visible;

            let level_row: Element<'_, Message> = row![
                iced::widget::checkbox(level_visible)
                    .on_toggle(move |v| { Message::FibLevelVisibilityToggled(idx, v) }),
                text(level_label).size(tokens::text::BODY).width(50),
                container(iced::widget::Space::new().width(14).height(14))
                    .style(move |_theme: &iced::Theme| {
                        container::Style {
                            background: Some(level_color.into()),
                            border: iced::Border {
                                radius: tokens::radius::SM.into(),
                                width: tokens::border::THIN,
                                color: iced::Color::WHITE.scale_alpha(0.2),
                            },
                            ..container::Style::default()
                        }
                    })
                    .width(14)
                    .height(14),
            ]
            .spacing(tokens::spacing::SM)
            .align_y(Alignment::Center)
            .into();

            if idx < mid {
                left_col = left_col.push(level_row);
            } else {
                right_col = right_col.push(level_row);
            }
        }

        let levels_grid: Element<'_, Message> = column![
            text("Levels").size(tokens::text::LABEL),
            row![
                left_col.width(Length::FillPortion(1)),
                right_col.width(Length::FillPortion(1)),
            ]
            .spacing(tokens::spacing::MD),
        ]
        .spacing(tokens::spacing::SM)
        .into();

        FormSectionBuilder::new("Fibonacci")
            .with_top_divider(true)
            .push(options_row)
            .push(extend_row)
            .push(levels_grid)
            .into()
    }

    /// Options list — label on left, control on right.
    fn options_section(&self) -> Element<'_, Message> {
        let mut section = FormSectionBuilder::new("Options");

        if self.has_labels() {
            section = section.push(option_row(
                "Show Labels",
                iced::widget::checkbox(self.show_labels).on_toggle(Message::ShowLabelsToggled),
            ));
        }

        section = section.push(option_row(
            "Visible",
            iced::widget::checkbox(self.visible).on_toggle(Message::VisibleToggled),
        ));

        if self.has_label_input() {
            // Two-column label row: text input + alignment dropdown
            let label_value = self.label.as_deref().unwrap_or("");
            let label_row: Element<'_, Message> = row![
                text("Label").size(tokens::text::BODY).width(50),
                text_input("Optional label...", label_value)
                    .on_input(Message::LabelChanged)
                    .width(Length::Fill),
                pick_list(
                    LabelAlignment::ALL.to_vec(),
                    Some(self.label_alignment),
                    Message::LabelAlignmentChanged,
                )
                .width(80),
            ]
            .spacing(tokens::spacing::MD)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .into();

            section = section.push(label_row);
        }

        section.into()
    }

    fn footer(&self) -> Element<'_, Message> {
        row![
            space::horizontal(),
            button(text("Cancel").size(tokens::text::BODY))
                .on_press(Message::Close)
                .padding([tokens::spacing::SM, tokens::spacing::XL])
                .style(style::button::secondary),
            button(text("Apply").size(tokens::text::BODY))
                .on_press(Message::Apply)
                .padding([tokens::spacing::SM, tokens::spacing::XL])
                .style(style::button::primary),
        ]
        .spacing(tokens::spacing::MD)
        .align_y(Alignment::Center)
        .into()
    }

    /// Floating color picker popup overlay.
    fn color_picker_popup(&self) -> Element<'_, Message> {
        if self.show_stroke_picker {
            self.stroke_picker_popup()
        } else {
            self.fill_picker_popup()
        }
    }

    fn stroke_picker_popup(&self) -> Element<'_, Message> {
        let stroke_iced: iced::Color = self.stroke_color.into();
        let hsva = self
            .editing_stroke_color
            .unwrap_or_else(|| data::config::theme::to_hsva(stroke_iced));

        picker_popup(hsva, Message::StrokeColorChanged)
    }

    fn fill_picker_popup(&self) -> Element<'_, Message> {
        let fill_c = self
            .fill_color
            .unwrap_or(SerializableColor::new(0.3, 0.6, 1.0, 1.0));
        let fill_iced: iced::Color = fill_c.into();
        let hsva = self
            .editing_fill_color
            .unwrap_or_else(|| data::config::theme::to_hsva(fill_iced));

        picker_popup(hsva, Message::FillColorChanged)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

/// Option row: label text on the left, control on the right.
fn option_row<'a>(
    label: &'a str,
    control: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    row![
        text(label).size(tokens::text::BODY),
        space::horizontal(),
        control.into(),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

/// Small color swatch button that toggles a color picker popup.
fn color_swatch<'a>(color: Color, is_active: bool, on_press: Message) -> Element<'a, Message> {
    button(space::horizontal().width(22).height(22))
        .style(move |_theme, _status| button::Style {
            background: Some(color.into()),
            border: iced::border::rounded(3)
                .width(if is_active { 2.0 } else { 1.0 })
                .color(if is_active {
                    Color::WHITE
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.3)
                }),
            ..button::Style::default()
        })
        .padding(0)
        .on_press(on_press)
        .into()
}

/// Hex color text input with validation styling.
fn hex_text_input<'a>(
    hex_value: &str,
    is_valid: bool,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    text_input("", hex_value)
        .on_input(on_input)
        .width(80)
        .style(move |theme: &iced::Theme, status| {
            let palette = theme.extended_palette();
            iced::widget::text_input::Style {
                border: iced::Border {
                    color: if is_valid {
                        palette.background.strong.color
                    } else {
                        palette.danger.base.color
                    },
                    width: tokens::border::THIN,
                    radius: tokens::radius::SM.into(),
                },
                ..iced::widget::text_input::default(theme, status)
            }
        })
        .into()
}

/// Compact square color picker popup.
fn picker_popup<'a>(
    hsva: Hsva,
    on_color: impl Fn(Hsva) -> Message + Clone + 'a,
) -> Element<'a, Message> {
    container(color_picker(hsva, on_color, 180.0))
        .padding(tokens::spacing::SM)
        .style(style::dropdown_container)
        .into()
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
