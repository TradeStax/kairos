use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue, Visibility,
};
use data::SerializableColor;

pub const DEFAULT_COLOR: SerializableColor = SerializableColor {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

pub fn make_params() -> Vec<ParameterDef> {
    vec![
        ParameterDef {
            key: "color".into(),
            label: "Color".into(),
            description: "OBV line color".into(),
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
    ]
}
