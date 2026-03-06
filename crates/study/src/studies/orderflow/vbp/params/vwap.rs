//! VWAP tab: anchored VWAP line and standard deviation bands.

use crate::config::{
    DisplayFormat, LineStyleValue, ParameterDef, ParameterKind, ParameterSection, ParameterTab,
    ParameterValue, Visibility,
};

use super::{DEFAULT_VWAP_BAND_COLOR, DEFAULT_VWAP_COLOR};

/// VWAP tab: anchored VWAP line and standard deviation bands.
pub(super) fn build_vwap_tab_params(params: &mut Vec<ParameterDef>) {
    let vwap_section = Some(ParameterSection {
        label: "VWAP Line",
        order: 0,
    });
    params.push(ParameterDef {
        key: "vwap_show".into(),
        label: "Show VWAP".into(),
        description: "Show anchored VWAP line".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::Vwap,
        section: vwap_section,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "vwap_color".into(),
        label: "Color".into(),
        description: "VWAP line color".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_VWAP_COLOR),
        tab: ParameterTab::Vwap,
        section: vwap_section,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("vwap_show"),
    });
    params.push(ParameterDef {
        key: "vwap_line_width".into(),
        label: "Line Width".into(),
        description: "VWAP line width".into(),
        kind: ParameterKind::Float {
            min: 0.5,
            max: 4.0,
            step: 0.5,
        },
        default: ParameterValue::Float(1.5),
        tab: ParameterTab::Vwap,
        section: vwap_section,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("vwap_show"),
    });
    params.push(ParameterDef {
        key: "vwap_line_style".into(),
        label: "Line Style".into(),
        description: "VWAP line style".into(),
        kind: ParameterKind::LineStyle,
        default: ParameterValue::LineStyle(LineStyleValue::Solid),
        tab: ParameterTab::Vwap,
        section: vwap_section,
        order: 3,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("vwap_show"),
    });
    params.push(ParameterDef {
        key: "vwap_show_label".into(),
        label: "Show Label".into(),
        description: "Show VWAP price label".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::Vwap,
        section: vwap_section,
        order: 4,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("vwap_show"),
    });

    // VWAP Bands section
    let vwap_bands = Some(ParameterSection {
        label: "Bands",
        order: 1,
    });
    params.push(ParameterDef {
        key: "vwap_show_bands".into(),
        label: "Show Bands".into(),
        description: "Show std dev bands around VWAP".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::Vwap,
        section: vwap_bands,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("vwap_show"),
    });
    params.push(ParameterDef {
        key: "vwap_band_multiplier".into(),
        label: "Multiplier".into(),
        description: "Std dev multiplier for bands".into(),
        kind: ParameterKind::Float {
            min: 1.0,
            max: 3.0,
            step: 0.5,
        },
        default: ParameterValue::Float(1.0),
        tab: ParameterTab::Vwap,
        section: vwap_bands,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("vwap_show_bands"),
    });
    params.push(ParameterDef {
        key: "vwap_band_color".into(),
        label: "Band Color".into(),
        description: "VWAP band color".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_VWAP_BAND_COLOR),
        tab: ParameterTab::Vwap,
        section: vwap_bands,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("vwap_show_bands"),
    });
    params.push(ParameterDef {
        key: "vwap_band_line_style".into(),
        label: "Band Style".into(),
        description: "VWAP band line style".into(),
        kind: ParameterKind::LineStyle,
        default: ParameterValue::LineStyle(LineStyleValue::Dashed),
        tab: ParameterTab::Vwap,
        section: vwap_bands,
        order: 3,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("vwap_show_bands"),
    });
    params.push(ParameterDef {
        key: "vwap_band_line_width".into(),
        label: "Band Width".into(),
        description: "VWAP band line width".into(),
        kind: ParameterKind::Float {
            min: 0.5,
            max: 4.0,
            step: 0.5,
        },
        default: ParameterValue::Float(1.0),
        tab: ParameterTab::Vwap,
        section: vwap_bands,
        order: 4,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("vwap_show_bands"),
    });
}
