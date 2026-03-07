use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue, Visibility,
};
use crate::{BEARISH_COLOR, BULLISH_COLOR};
use data::SerializableColor;

/// Build the default parameter definitions for the MACD study.
///
/// Defines three period parameters (fast, slow, signal), two line
/// colors (MACD line, signal line), and two histogram colors
/// (positive/negative divergence).
pub(super) fn make_params() -> Vec<ParameterDef> {
    vec![
        ParameterDef {
            key: "fast_period".into(),
            label: "Fast Period".into(),
            description: "Fast EMA period".into(),
            kind: ParameterKind::Integer { min: 2, max: 100 },
            default: ParameterValue::Integer(12),
            tab: ParameterTab::Parameters,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "slow_period".into(),
            label: "Slow Period".into(),
            description: "Slow EMA period".into(),
            kind: ParameterKind::Integer { min: 2, max: 200 },
            default: ParameterValue::Integer(26),
            tab: ParameterTab::Parameters,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "signal_period".into(),
            label: "Signal Period".into(),
            description: "Signal line EMA period".into(),
            kind: ParameterKind::Integer { min: 2, max: 100 },
            default: ParameterValue::Integer(9),
            tab: ParameterTab::Parameters,
            section: None,
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "macd_color".into(),
            label: "MACD Color".into(),
            description: "MACD line color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(SerializableColor {
                r: 0.2,
                g: 0.6,
                b: 1.0,
                a: 1.0,
            }),
            tab: ParameterTab::Style,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "signal_color".into(),
            label: "Signal Color".into(),
            description: "Signal line color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(SerializableColor {
                r: 1.0,
                g: 0.6,
                b: 0.2,
                a: 1.0,
            }),
            tab: ParameterTab::Style,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "hist_positive_color".into(),
            label: "Histogram +".into(),
            description: "Histogram positive color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(BULLISH_COLOR.with_alpha(0.7)),
            tab: ParameterTab::Style,
            section: None,
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "hist_negative_color".into(),
            label: "Histogram -".into(),
            description: "Histogram negative color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(BEARISH_COLOR.with_alpha(0.7)),
            tab: ParameterTab::Style,
            section: None,
            order: 3,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
    ]
}
