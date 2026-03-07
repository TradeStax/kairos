use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue, Visibility,
};
use crate::{BEARISH_COLOR, BULLISH_COLOR};
use data::SerializableColor;

pub const DEFAULT_THRESHOLD: f64 = 3.0;
pub const DEFAULT_HIT_DECAY: f64 = 0.5;

pub const DEFAULT_BUY_COLOR: SerializableColor = BULLISH_COLOR.with_alpha(0.6);
pub const DEFAULT_SELL_COLOR: SerializableColor = BEARISH_COLOR.with_alpha(0.6);

/// Levels with opacity below this are invisible and dropped.
pub const MIN_OPACITY: f32 = 0.03;

/// Hard cap on emitted levels to bound renderer draw calls.
/// When exceeded, oldest (leftmost) levels are discarded.
pub const MAX_OUTPUT_LEVELS: usize = 1500;

pub fn make_params() -> Vec<ParameterDef> {
    vec![
        ParameterDef {
            key: "threshold".into(),
            label: "Threshold".into(),
            description: "Imbalance ratio threshold".into(),
            kind: ParameterKind::Float {
                min: 1.0,
                max: 10.0,
                step: 0.5,
            },
            default: ParameterValue::Float(DEFAULT_THRESHOLD),
            tab: ParameterTab::Parameters,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "buy_color".into(),
            label: "Buy Color".into(),
            description: "Color for buy imbalances".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(DEFAULT_BUY_COLOR),
            tab: ParameterTab::Style,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "sell_color".into(),
            label: "Sell Color".into(),
            description: "Color for sell imbalances".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(DEFAULT_SELL_COLOR),
            tab: ParameterTab::Style,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "ignore_zeros".into(),
            label: "Ignore Zeros".into(),
            description: "Skip levels with zero volume".into(),
            kind: ParameterKind::Boolean,
            default: ParameterValue::Boolean(true),
            tab: ParameterTab::Parameters,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "hit_decay".into(),
            label: "Hit Decay".into(),
            description: "Opacity multiplier per price hit".into(),
            kind: ParameterKind::Float {
                min: 0.1,
                max: 1.0,
                step: 0.1,
            },
            default: ParameterValue::Float(DEFAULT_HIT_DECAY),
            tab: ParameterTab::Parameters,
            section: None,
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
    ]
}
