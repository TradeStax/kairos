use crate::BULLISH_COLOR;
use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterSection, ParameterTab, ParameterValue,
    Visibility,
};
use data::SerializableColor;

pub(super) const DEFAULT_BUCKET_SECONDS: i64 = 10;
pub(super) const DEFAULT_FILTER_MIN: i64 = 1;
pub(super) const DEFAULT_FILTER_MAX: i64 = 0;
pub(super) const DEFAULT_STDDEV_FILTER: f64 = 2.0;

pub(super) const DEFAULT_BUY_COLOR: SerializableColor = BULLISH_COLOR;

/// Default sell color — purple #8C52AF.
pub(super) const DEFAULT_SELL_COLOR: SerializableColor =
    SerializableColor::from_rgb8_const(140, 82, 175);

pub(super) const DEFAULT_BODY_OPACITY: f64 = 0.5;
pub(super) const DEFAULT_BORDER_OPACITY: f64 = 1.0;

pub(super) fn make_params() -> Vec<ParameterDef> {
    vec![
        // ── Data Settings (order: 0) ──────────────────
        ParameterDef {
            key: "input_data".into(),
            label: "Input Data".into(),
            description: "Measure volume or trade count per bucket".into(),
            kind: ParameterKind::Choice {
                options: &["Volume", "Trades"],
            },
            default: ParameterValue::Choice("Volume".into()),
            tab: ParameterTab::Parameters,
            section: Some(ParameterSection {
                label: "Data Settings",
                order: 0,
            }),
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "filter_min".into(),
            label: "Filter Min".into(),
            description: "Min trade size to include (0 = none)".into(),
            kind: ParameterKind::Integer { min: 0, max: 10000 },
            default: ParameterValue::Integer(DEFAULT_FILTER_MIN),
            tab: ParameterTab::Parameters,
            section: Some(ParameterSection {
                label: "Data Settings",
                order: 0,
            }),
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "filter_max".into(),
            label: "Filter Max".into(),
            description: "Max trade size to include (0 = none)".into(),
            kind: ParameterKind::Integer { min: 0, max: 10000 },
            default: ParameterValue::Integer(DEFAULT_FILTER_MAX),
            tab: ParameterTab::Parameters,
            section: Some(ParameterSection {
                label: "Data Settings",
                order: 0,
            }),
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        // ── Mode (order: 1) ───────────────────────────
        ParameterDef {
            key: "display_value".into(),
            label: "Display Value".into(),
            description: "Which side of activity to display".into(),
            kind: ParameterKind::Choice {
                options: &["Total", "Buy", "Sell", "Delta"],
            },
            default: ParameterValue::Choice("Total".into()),
            tab: ParameterTab::Parameters,
            section: Some(ParameterSection {
                label: "Mode",
                order: 1,
            }),
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "bucket_seconds".into(),
            label: "Bucket Seconds".into(),
            description: "Bucket time window in seconds".into(),
            kind: ParameterKind::Integer { min: 1, max: 120 },
            default: ParameterValue::Integer(DEFAULT_BUCKET_SECONDS),
            tab: ParameterTab::Parameters,
            section: Some(ParameterSection {
                label: "Mode",
                order: 1,
            }),
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        // ── Filter (order: 2) ─────────────────────────
        ParameterDef {
            key: "filter_mode".into(),
            label: "Filter Mode".into(),
            description: "Outlier filtering mode".into(),
            kind: ParameterKind::Choice {
                options: &["None", "Automatic"],
            },
            default: ParameterValue::Choice("Automatic".into()),
            tab: ParameterTab::Parameters,
            section: Some(ParameterSection {
                label: "Filter",
                order: 2,
            }),
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "stddev_filter".into(),
            label: "StdDev Multiplier".into(),
            description: "Cap at mean + mult × stddev".into(),
            kind: ParameterKind::Float {
                min: 0.5,
                max: 5.0,
                step: 0.1,
            },
            default: ParameterValue::Float(DEFAULT_STDDEV_FILTER),
            tab: ParameterTab::Parameters,
            section: Some(ParameterSection {
                label: "Filter",
                order: 2,
            }),
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenChoice {
                key: "filter_mode",
                equals: "Automatic",
            },
        },
        // ── Style ─────────────────────────────────────
        ParameterDef {
            key: "buy_color".into(),
            label: "Buy Color".into(),
            description: "Color for buy-dominant candles".into(),
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
            description: "Color for sell-dominant candles".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(DEFAULT_SELL_COLOR),
            tab: ParameterTab::Style,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "body_opacity".into(),
            label: "Body Opacity".into(),
            description: "Opacity of the candle body fill".into(),
            kind: ParameterKind::Float {
                min: 0.0,
                max: 1.0,
                step: 0.05,
            },
            default: ParameterValue::Float(DEFAULT_BODY_OPACITY),
            tab: ParameterTab::Style,
            section: None,
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "border_opacity".into(),
            label: "Border Opacity".into(),
            description: "Opacity of the candle wick and outline".into(),
            kind: ParameterKind::Float {
                min: 0.0,
                max: 1.0,
                step: 0.05,
            },
            default: ParameterValue::Float(DEFAULT_BORDER_OPACITY),
            tab: ParameterTab::Style,
            section: None,
            order: 3,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
    ]
}
