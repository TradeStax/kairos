use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue, Visibility,
};
use data::SerializableColor;

pub(super) const DEFAULT_K_COLOR: SerializableColor = SerializableColor {
    r: 0.2,
    g: 0.6,
    b: 1.0,
    a: 1.0,
};

pub(super) const DEFAULT_D_COLOR: SerializableColor = SerializableColor {
    r: 1.0,
    g: 0.4,
    b: 0.4,
    a: 1.0,
};

/// Build the default parameter definitions for the Stochastic study.
pub(super) fn make_params() -> Vec<ParameterDef> {
    vec![
        ParameterDef {
            key: "k_period".into(),
            label: "%K Period".into(),
            description: "Lookback period for %K calculation".into(),
            kind: ParameterKind::Integer { min: 5, max: 50 },
            default: ParameterValue::Integer(14),
            tab: ParameterTab::Parameters,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "d_period".into(),
            label: "%D Period".into(),
            description: "Smoothing period for %D (signal line)".into(),
            kind: ParameterKind::Integer { min: 1, max: 20 },
            default: ParameterValue::Integer(3),
            tab: ParameterTab::Parameters,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "smooth".into(),
            label: "Smooth".into(),
            description: "Smoothing period for %K".into(),
            kind: ParameterKind::Integer { min: 1, max: 10 },
            default: ParameterValue::Integer(3),
            tab: ParameterTab::Parameters,
            section: None,
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "overbought".into(),
            label: "Overbought".into(),
            description: "Overbought level".into(),
            kind: ParameterKind::Float {
                min: 50.0,
                max: 100.0,
                step: 5.0,
            },
            default: ParameterValue::Float(80.0),
            tab: ParameterTab::Parameters,
            section: None,
            order: 3,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "oversold".into(),
            label: "Oversold".into(),
            description: "Oversold level".into(),
            kind: ParameterKind::Float {
                min: 0.0,
                max: 50.0,
                step: 5.0,
            },
            default: ParameterValue::Float(20.0),
            tab: ParameterTab::Parameters,
            section: None,
            order: 4,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "k_color".into(),
            label: "%K Color".into(),
            description: "%K line color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(DEFAULT_K_COLOR),
            tab: ParameterTab::Style,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "d_color".into(),
            label: "%D Color".into(),
            description: "%D line color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(DEFAULT_D_COLOR),
            tab: ParameterTab::Style,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
    ]
}
