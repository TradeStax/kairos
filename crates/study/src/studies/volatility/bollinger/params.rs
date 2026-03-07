use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue, Visibility,
};
use data::SerializableColor;

pub const DEFAULT_UPPER_COLOR: SerializableColor = SerializableColor {
    r: 0.2,
    g: 0.6,
    b: 1.0,
    a: 0.6,
};

pub const DEFAULT_MIDDLE_COLOR: SerializableColor = SerializableColor {
    r: 0.2,
    g: 0.6,
    b: 1.0,
    a: 1.0,
};

pub const DEFAULT_LOWER_COLOR: SerializableColor = SerializableColor {
    r: 0.2,
    g: 0.6,
    b: 1.0,
    a: 0.6,
};

pub fn make_params() -> Vec<ParameterDef> {
    vec![
        ParameterDef {
            key: "period".into(),
            label: "Period".into(),
            description: "Number of candles for the moving average".into(),
            kind: ParameterKind::Integer { min: 2, max: 500 },
            default: ParameterValue::Integer(20),
            tab: ParameterTab::Parameters,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "std_dev".into(),
            label: "Std Dev".into(),
            description: "Standard deviation multiplier for bands".into(),
            kind: ParameterKind::Float {
                min: 0.5,
                max: 5.0,
                step: 0.5,
            },
            default: ParameterValue::Float(2.0),
            tab: ParameterTab::Parameters,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "upper_color".into(),
            label: "Upper Color".into(),
            description: "Upper band color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(DEFAULT_UPPER_COLOR),
            tab: ParameterTab::Style,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "middle_color".into(),
            label: "Middle Color".into(),
            description: "Middle band (SMA) color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(DEFAULT_MIDDLE_COLOR),
            tab: ParameterTab::Style,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "lower_color".into(),
            label: "Lower Color".into(),
            description: "Lower band color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(DEFAULT_LOWER_COLOR),
            tab: ParameterTab::Style,
            section: None,
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "fill_opacity".into(),
            label: "Fill Opacity".into(),
            description: "Opacity of the band fill".into(),
            kind: ParameterKind::Float {
                min: 0.0,
                max: 1.0,
                step: 0.05,
            },
            default: ParameterValue::Float(0.1),
            tab: ParameterTab::Style,
            section: None,
            order: 3,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
    ]
}
