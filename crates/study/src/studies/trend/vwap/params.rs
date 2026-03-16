use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue, Visibility,
};
use data::SerializableColor;

pub const DEFAULT_COLOR: SerializableColor = SerializableColor {
    r: 0.0,
    g: 0.9,
    b: 0.9,
    a: 1.0,
};

pub const BAND_COLOR: SerializableColor = SerializableColor {
    r: 0.0,
    g: 0.9,
    b: 0.9,
    a: 0.4,
};

pub fn make_params() -> Vec<ParameterDef> {
    vec![
        ParameterDef {
            key: "color".into(),
            label: "Color".into(),
            description: "VWAP line color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(DEFAULT_COLOR),
            tab: ParameterTab::Style,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "width".into(),
            label: "Width".into(),
            description: "Line width".into(),
            kind: ParameterKind::Float {
                min: 0.5,
                max: 5.0,
                step: 0.5,
            },
            default: ParameterValue::Float(1.5),
            tab: ParameterTab::Style,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "show_bands".into(),
            label: "Show Bands".into(),
            description: "Show standard deviation bands".into(),
            kind: ParameterKind::Boolean,
            default: ParameterValue::Boolean(false),
            tab: ParameterTab::Display,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "band_multiplier".into(),
            label: "Band Multiplier".into(),
            description: "Standard deviation multiplier for bands".into(),
            kind: ParameterKind::Float {
                min: 1.0,
                max: 3.0,
                step: 0.5,
            },
            default: ParameterValue::Float(1.0),
            tab: ParameterTab::Parameters,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("show_bands"),
        },
    ]
}
