use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue, Visibility,
};
use crate::{BEARISH_COLOR, BULLISH_COLOR};
use data::SerializableColor;

pub const DEFAULT_POS_COLOR: SerializableColor = BULLISH_COLOR;

pub const DEFAULT_NEG_COLOR: SerializableColor = BEARISH_COLOR;

pub const DEFAULT_OPACITY: f64 = 0.8;

pub fn make_params() -> Vec<ParameterDef> {
    vec![
        ParameterDef {
            key: "positive_color".into(),
            label: "Positive Color".into(),
            description: "Color for positive delta bars".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(DEFAULT_POS_COLOR),
            tab: ParameterTab::Style,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "negative_color".into(),
            label: "Negative Color".into(),
            description: "Color for negative delta bars".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(DEFAULT_NEG_COLOR),
            tab: ParameterTab::Style,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "opacity".into(),
            label: "Opacity".into(),
            description: "Bar opacity".into(),
            kind: ParameterKind::Float {
                min: 0.0,
                max: 1.0,
                step: 0.05,
            },
            default: ParameterValue::Float(DEFAULT_OPACITY),
            tab: ParameterTab::Style,
            section: None,
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
    ]
}
