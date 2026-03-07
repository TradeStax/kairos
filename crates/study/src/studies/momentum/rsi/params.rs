use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue, Visibility,
};
use data::SerializableColor;

/// Gray color used for the overbought/oversold reference levels.
pub(super) const LEVEL_COLOR: SerializableColor = SerializableColor {
    r: 0.7,
    g: 0.7,
    b: 0.7,
    a: 0.6,
};

/// Build the default parameter definitions for the RSI study.
pub(super) fn make_params() -> Vec<ParameterDef> {
    vec![
        ParameterDef {
            key: "period".into(),
            label: "Period".into(),
            description: "RSI lookback period".into(),
            kind: ParameterKind::Integer { min: 2, max: 100 },
            default: ParameterValue::Integer(14),
            tab: ParameterTab::Parameters,
            section: None,
            order: 0,
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
            default: ParameterValue::Float(70.0),
            tab: ParameterTab::Parameters,
            section: None,
            order: 1,
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
            default: ParameterValue::Float(30.0),
            tab: ParameterTab::Parameters,
            section: None,
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "color".into(),
            label: "Color".into(),
            description: "RSI line color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(SerializableColor {
                r: 1.0,
                g: 0.85,
                b: 0.2,
                a: 1.0,
            }),
            tab: ParameterTab::Style,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
    ]
}
