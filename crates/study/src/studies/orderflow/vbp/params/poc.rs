//! POC tab: POC line and developing POC settings.

use crate::config::{
    DisplayFormat, LineStyleValue, ParameterDef, ParameterKind, ParameterSection, ParameterTab,
    ParameterValue, Visibility,
};

use super::{DEFAULT_DEV_POC_COLOR, DEFAULT_POC_COLOR};

/// POC tab: POC line and developing POC settings.
pub(super) fn build_poc_tab_params(params: &mut Vec<ParameterDef>) {
    let poc_line_section = Some(ParameterSection {
        label: "POC Line",
        order: 0,
    });
    params.push(ParameterDef {
        key: "poc_show".into(),
        label: "Show POC".into(),
        description: "Show Point of Control line".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(true),
        tab: ParameterTab::PocSettings,
        section: poc_line_section,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "poc_color".into(),
        label: "Color".into(),
        description: "POC line color".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_POC_COLOR),
        tab: ParameterTab::PocSettings,
        section: poc_line_section,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("poc_show"),
    });
    params.push(ParameterDef {
        key: "poc_line_width".into(),
        label: "Line Width".into(),
        description: "POC line width".into(),
        kind: ParameterKind::Float {
            min: 0.5,
            max: 4.0,
            step: 0.5,
        },
        default: ParameterValue::Float(1.5),
        tab: ParameterTab::PocSettings,
        section: poc_line_section,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("poc_show"),
    });
    params.push(ParameterDef {
        key: "poc_line_style".into(),
        label: "Line Style".into(),
        description: "POC line style".into(),
        kind: ParameterKind::LineStyle,
        default: ParameterValue::LineStyle(LineStyleValue::Solid),
        tab: ParameterTab::PocSettings,
        section: poc_line_section,
        order: 3,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("poc_show"),
    });
    params.push(ParameterDef {
        key: "poc_extend".into(),
        label: "Extend".into(),
        description: "Extend POC line beyond profile".into(),
        kind: ParameterKind::Choice {
            options: &["None", "Left", "Right", "Both"],
        },
        default: ParameterValue::Choice("None".to_string()),
        tab: ParameterTab::PocSettings,
        section: poc_line_section,
        order: 4,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("poc_show"),
    });
    params.push(ParameterDef {
        key: "poc_show_label".into(),
        label: "Show Label".into(),
        description: "Show price label at POC".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::PocSettings,
        section: poc_line_section,
        order: 5,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("poc_show"),
    });

    // Developing POC section
    let dev_poc_section = Some(ParameterSection {
        label: "Developing POC",
        order: 1,
    });
    params.push(ParameterDef {
        key: "poc_show_developing".into(),
        label: "Show Developing POC".into(),
        description: "Show developing POC line".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::PocSettings,
        section: dev_poc_section,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "poc_dev_color".into(),
        label: "Color".into(),
        description: "Developing POC line color".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_DEV_POC_COLOR),
        tab: ParameterTab::PocSettings,
        section: dev_poc_section,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("poc_show_developing"),
    });
    params.push(ParameterDef {
        key: "poc_dev_line_width".into(),
        label: "Line Width".into(),
        description: "Developing POC line width".into(),
        kind: ParameterKind::Float {
            min: 0.5,
            max: 4.0,
            step: 0.5,
        },
        default: ParameterValue::Float(1.0),
        tab: ParameterTab::PocSettings,
        section: dev_poc_section,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("poc_show_developing"),
    });
    params.push(ParameterDef {
        key: "poc_dev_line_style".into(),
        label: "Line Style".into(),
        description: "Developing POC line style".into(),
        kind: ParameterKind::LineStyle,
        default: ParameterValue::LineStyle(LineStyleValue::Dashed),
        tab: ParameterTab::PocSettings,
        section: dev_poc_section,
        order: 3,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("poc_show_developing"),
    });
}
