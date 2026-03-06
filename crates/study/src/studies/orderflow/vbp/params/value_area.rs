//! Value Area tab: VA toggle, lines, fill settings.

use crate::config::{
    DisplayFormat, LineStyleValue, ParameterDef, ParameterKind, ParameterSection, ParameterTab,
    ParameterValue, Visibility,
};

use super::{DEFAULT_VA_FILL_COLOR, DEFAULT_VAH_COLOR, DEFAULT_VAL_COLOR};

/// Value Area tab: VA toggle, lines, fill settings.
pub(super) fn build_value_area_tab_params(params: &mut Vec<ParameterDef>) {
    let va_section = Some(ParameterSection {
        label: "Value Area",
        order: 0,
    });
    params.push(ParameterDef {
        key: "va_show".into(),
        label: "Show Value Area".into(),
        description: "Enable value area features".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(true),
        tab: ParameterTab::ValueArea,
        section: va_section,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "va_show_highlight".into(),
        label: "Dim Outside VA".into(),
        description: "Dim bars outside value area".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(true),
        tab: ParameterTab::ValueArea,
        section: va_section,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show"),
    });

    // VA Lines section
    let va_lines = Some(ParameterSection {
        label: "Lines",
        order: 1,
    });
    params.push(ParameterDef {
        key: "va_vah_color".into(),
        label: "VAH Color".into(),
        description: "Value Area High line color".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_VAH_COLOR),
        tab: ParameterTab::ValueArea,
        section: va_lines,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show"),
    });
    params.push(ParameterDef {
        key: "va_vah_line_width".into(),
        label: "VAH Width".into(),
        description: "VAH line width".into(),
        kind: ParameterKind::Float {
            min: 0.5,
            max: 4.0,
            step: 0.5,
        },
        default: ParameterValue::Float(1.0),
        tab: ParameterTab::ValueArea,
        section: va_lines,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show"),
    });
    params.push(ParameterDef {
        key: "va_vah_line_style".into(),
        label: "VAH Style".into(),
        description: "VAH line style".into(),
        kind: ParameterKind::LineStyle,
        default: ParameterValue::LineStyle(LineStyleValue::Dashed),
        tab: ParameterTab::ValueArea,
        section: va_lines,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show"),
    });
    params.push(ParameterDef {
        key: "va_val_color".into(),
        label: "VAL Color".into(),
        description: "Value Area Low line color".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_VAL_COLOR),
        tab: ParameterTab::ValueArea,
        section: va_lines,
        order: 3,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show"),
    });
    params.push(ParameterDef {
        key: "va_val_line_width".into(),
        label: "VAL Width".into(),
        description: "VAL line width".into(),
        kind: ParameterKind::Float {
            min: 0.5,
            max: 4.0,
            step: 0.5,
        },
        default: ParameterValue::Float(1.0),
        tab: ParameterTab::ValueArea,
        section: va_lines,
        order: 4,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show"),
    });
    params.push(ParameterDef {
        key: "va_val_line_style".into(),
        label: "VAL Style".into(),
        description: "VAL line style".into(),
        kind: ParameterKind::LineStyle,
        default: ParameterValue::LineStyle(LineStyleValue::Dashed),
        tab: ParameterTab::ValueArea,
        section: va_lines,
        order: 5,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show"),
    });
    params.push(ParameterDef {
        key: "va_extend".into(),
        label: "Extend".into(),
        description: "Extend VA lines beyond profile".into(),
        kind: ParameterKind::Choice {
            options: &["None", "Left", "Right", "Both"],
        },
        default: ParameterValue::Choice("None".to_string()),
        tab: ParameterTab::ValueArea,
        section: va_lines,
        order: 6,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show"),
    });
    params.push(ParameterDef {
        key: "va_show_labels".into(),
        label: "Show Labels".into(),
        description: "Show price labels at VA boundaries".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::ValueArea,
        section: va_lines,
        order: 7,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show"),
    });

    // VA Fill section
    let va_fill = Some(ParameterSection {
        label: "Fill",
        order: 2,
    });
    params.push(ParameterDef {
        key: "va_show_fill".into(),
        label: "Show Fill".into(),
        description: "Fill between VAH and VAL".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::ValueArea,
        section: va_fill,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show"),
    });
    params.push(ParameterDef {
        key: "va_fill_color".into(),
        label: "Fill Color".into(),
        description: "VA fill color".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_VA_FILL_COLOR),
        tab: ParameterTab::ValueArea,
        section: va_fill,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show_fill"),
    });
    params.push(ParameterDef {
        key: "va_fill_opacity".into(),
        label: "Fill Opacity".into(),
        description: "VA fill opacity".into(),
        kind: ParameterKind::Float {
            min: 0.0,
            max: 0.5,
            step: 0.05,
        },
        default: ParameterValue::Float(0.15),
        tab: ParameterTab::ValueArea,
        section: va_fill,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show_fill"),
    });
}
