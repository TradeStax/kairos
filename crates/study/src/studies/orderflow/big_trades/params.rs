//! Parameter definitions and runtime parameter structs for Big Trades.
//!
//! Contains both the `ParameterDef` array builder and the runtime
//! `ComputeParams` / `AbsorptionParams` structs extracted from
//! `StudyConfig` at the start of each compute pass.

use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue, Visibility,
};
use data::SerializableColor;

pub(super) const DEFAULT_DAYS_TO_LOAD: i64 = 1;
pub(super) const DEFAULT_FILTER_MIN: i64 = 50;
pub(super) const DEFAULT_FILTER_MAX: i64 = 0;
pub(super) const DEFAULT_AGGREGATION_WINDOW_MS: i64 = 40;

// Theme-matched colors (Kairos default palette)
#[allow(clippy::approx_constant)]
pub(super) const DEFAULT_BUY_COLOR: SerializableColor = SerializableColor {
    r: 0.318,
    g: 0.804,
    b: 0.627,
    a: 1.0,
};

pub(super) const DEFAULT_SELL_COLOR: SerializableColor = SerializableColor {
    r: 0.753,
    g: 0.314,
    b: 0.302,
    a: 1.0,
};

pub(super) const DEFAULT_TEXT_COLOR: SerializableColor = SerializableColor {
    r: 0.88,
    g: 0.88,
    b: 0.88,
    a: 0.9,
};

// Absorption zone default colors (semi-transparent)
#[allow(clippy::approx_constant)]
pub(super) const DEFAULT_ABSORPTION_BUY_COLOR: SerializableColor = SerializableColor {
    r: 0.318,
    g: 0.804,
    b: 0.627,
    a: 0.6,
};

pub(super) const DEFAULT_ABSORPTION_SELL_COLOR: SerializableColor = SerializableColor {
    r: 0.753,
    g: 0.314,
    b: 0.302,
    a: 0.6,
};

/// Snapshot of user-configurable parameters for a single compute pass.
pub(super) struct ComputeParams {
    pub filter_min: f64,
    pub filter_max: f64,
    pub window_ms: u64,
    pub buy_color: SerializableColor,
    pub sell_color: SerializableColor,
    pub show_text: bool,
    pub show_debug: bool,
}

/// Runtime absorption detection parameters.
pub(super) struct AbsorptionParams {
    pub enabled: bool,
    pub lambda_window: usize,
    pub lambda_smooth: usize,
    pub score_threshold: f64,
    pub volume_k: f64,
    pub confirm_window_ms: u64,
    pub buy_zone_color: SerializableColor,
    pub sell_zone_color: SerializableColor,
    pub zone_opacity: f32,
    pub show_zone_labels: bool,
}

/// Build the full parameter definition list for BigTradesStudy.
pub(super) fn build_parameter_defs() -> Vec<ParameterDef> {
    vec![
        // ── Data Settings ────────────────────────────────
        ParameterDef {
            key: "days_to_load".into(),
            label: "Days to Load".into(),
            description: "Number of days of trade data to analyze".into(),
            kind: ParameterKind::Integer { min: 1, max: 30 },
            default: ParameterValue::Integer(DEFAULT_DAYS_TO_LOAD),
            tab: ParameterTab::Parameters,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "filter_min".into(),
            label: "Filter Min".into(),
            description: "Minimum contracts to display (0 = none)".into(),
            kind: ParameterKind::Integer { min: 0, max: 2000 },
            default: ParameterValue::Integer(DEFAULT_FILTER_MIN),
            tab: ParameterTab::Parameters,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "filter_max".into(),
            label: "Filter Max".into(),
            description: "Maximum contracts to display (0 = none)".into(),
            kind: ParameterKind::Integer { min: 0, max: 2000 },
            default: ParameterValue::Integer(DEFAULT_FILTER_MAX),
            tab: ParameterTab::Parameters,
            section: None,
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "aggregation_window_ms".into(),
            label: "Aggregation Window".into(),
            description: "Max ms gap between fills to merge".into(),
            kind: ParameterKind::Integer { min: 10, max: 500 },
            default: ParameterValue::Integer(DEFAULT_AGGREGATION_WINDOW_MS),
            tab: ParameterTab::Parameters,
            section: None,
            order: 3,
            format: DisplayFormat::Integer { suffix: " ms" },
            visible_when: Visibility::Always,
        },
        // ── Style / General ──────────────────────────────
        ParameterDef {
            key: "marker_shape".into(),
            label: "Marker Shape".into(),
            description: "Shape used for markers".into(),
            kind: ParameterKind::Choice {
                options: &["Circle", "Square", "Text Only"],
            },
            default: ParameterValue::Choice("Circle".to_string()),
            tab: ParameterTab::Style,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "hollow".into(),
            label: "Hollow Fill".into(),
            description: "Draw markers as outlines only".into(),
            kind: ParameterKind::Boolean,
            default: ParameterValue::Boolean(false),
            tab: ParameterTab::Style,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "show_text".into(),
            label: "Show Text".into(),
            description: "Show contract count text on markers".into(),
            kind: ParameterKind::Boolean,
            default: ParameterValue::Boolean(true),
            tab: ParameterTab::Display,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        // ── Style / Size ─────────────────────────────────
        ParameterDef {
            key: "min_size".into(),
            label: "Min Size".into(),
            description: "Minimum marker radius in pixels".into(),
            kind: ParameterKind::Float {
                min: 2.0,
                max: 60.0,
                step: 1.0,
            },
            default: ParameterValue::Float(8.0),
            tab: ParameterTab::Style,
            section: None,
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "max_size".into(),
            label: "Max Size".into(),
            description: "Maximum marker radius in pixels".into(),
            kind: ParameterKind::Float {
                min: 10.0,
                max: 100.0,
                step: 1.0,
            },
            default: ParameterValue::Float(36.0),
            tab: ParameterTab::Style,
            section: None,
            order: 3,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        // ── Style / Color ────────────────────────────────
        ParameterDef {
            key: "min_opacity".into(),
            label: "Min Opacity".into(),
            description: "Opacity for smallest markers".into(),
            kind: ParameterKind::Float {
                min: 0.0,
                max: 1.0,
                step: 0.05,
            },
            default: ParameterValue::Float(0.10),
            tab: ParameterTab::Style,
            section: None,
            order: 4,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "max_opacity".into(),
            label: "Max Opacity".into(),
            description: "Opacity for largest markers".into(),
            kind: ParameterKind::Float {
                min: 0.0,
                max: 1.0,
                step: 0.05,
            },
            default: ParameterValue::Float(0.60),
            tab: ParameterTab::Style,
            section: None,
            order: 5,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "buy_color".into(),
            label: "Buy Color".into(),
            description: "Color for buy (aggressor) markers".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(DEFAULT_BUY_COLOR),
            tab: ParameterTab::Style,
            section: None,
            order: 6,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "sell_color".into(),
            label: "Sell Color".into(),
            description: "Color for sell (aggressor) markers".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(DEFAULT_SELL_COLOR),
            tab: ParameterTab::Style,
            section: None,
            order: 7,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        // ── Style / Text ─────────────────────────────────
        ParameterDef {
            key: "text_size".into(),
            label: "Text Size".into(),
            description: "Font size for marker labels".into(),
            kind: ParameterKind::Float {
                min: 6.0,
                max: 20.0,
                step: 0.5,
            },
            default: ParameterValue::Float(10.0),
            tab: ParameterTab::Style,
            section: None,
            order: 8,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "text_color".into(),
            label: "Text Color".into(),
            description: "Color for marker label text".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(DEFAULT_TEXT_COLOR),
            tab: ParameterTab::Style,
            section: None,
            order: 9,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        // ── Debug ────────────────────────────────────────
        ParameterDef {
            key: "show_debug".into(),
            label: "Show Debug".into(),
            description: "Show debug annotations on markers".into(),
            kind: ParameterKind::Boolean,
            default: ParameterValue::Boolean(false),
            tab: ParameterTab::Display,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        // ── Absorption ───────────────────────────────────
        ParameterDef {
            key: "absorption_enabled".into(),
            label: "Enable".into(),
            description: "Detect absorption zones where large flow \
                          is absorbed without proportional price impact"
                .into(),
            kind: ParameterKind::Boolean,
            default: ParameterValue::Boolean(false),
            tab: ParameterTab::Absorption,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "absorption_lambda_window".into(),
            label: "Lambda History".into(),
            description: "Number of impact records to keep for lambda \
                          estimation"
                .into(),
            kind: ParameterKind::Integer { min: 10, max: 200 },
            default: ParameterValue::Integer(50),
            tab: ParameterTab::Absorption,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("absorption_enabled"),
        },
        ParameterDef {
            key: "absorption_lambda_smooth".into(),
            label: "Lambda Smoothing".into(),
            description: "EMA period for smoothing the price impact \
                          coefficient"
                .into(),
            kind: ParameterKind::Integer { min: 5, max: 100 },
            default: ParameterValue::Integer(20),
            tab: ParameterTab::Absorption,
            section: None,
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("absorption_enabled"),
        },
        ParameterDef {
            key: "absorption_score_threshold".into(),
            label: "Score Threshold".into(),
            description: "Actual/expected ratio below which absorption \
                          is detected (lower = stricter)"
                .into(),
            kind: ParameterKind::Float {
                min: 0.01,
                max: 1.0,
                step: 0.01,
            },
            default: ParameterValue::Float(0.25),
            tab: ParameterTab::Absorption,
            section: None,
            order: 3,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("absorption_enabled"),
        },
        ParameterDef {
            key: "absorption_volume_k".into(),
            label: "Volume Sensitivity".into(),
            description: "Std-dev multiplier for adaptive volume \
                          threshold (lower = more detections)"
                .into(),
            kind: ParameterKind::Float {
                min: 0.5,
                max: 5.0,
                step: 0.1,
            },
            default: ParameterValue::Float(2.0),
            tab: ParameterTab::Absorption,
            section: None,
            order: 4,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("absorption_enabled"),
        },
        ParameterDef {
            key: "absorption_confirm_ms".into(),
            label: "Confirmation Window".into(),
            description: "Max milliseconds to wait for price rejection \
                          confirmation"
                .into(),
            kind: ParameterKind::Integer {
                min: 5000,
                max: 60000,
            },
            default: ParameterValue::Integer(20000),
            tab: ParameterTab::Absorption,
            section: None,
            order: 5,
            format: DisplayFormat::Integer { suffix: " ms" },
            visible_when: Visibility::WhenTrue("absorption_enabled"),
        },
        ParameterDef {
            key: "absorption_buy_zone_color".into(),
            label: "Buy Zone Color".into(),
            description: "Color for buy-side absorption zones".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(DEFAULT_ABSORPTION_BUY_COLOR),
            tab: ParameterTab::Absorption,
            section: None,
            order: 6,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("absorption_enabled"),
        },
        ParameterDef {
            key: "absorption_sell_zone_color".into(),
            label: "Sell Zone Color".into(),
            description: "Color for sell-side absorption zones".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(DEFAULT_ABSORPTION_SELL_COLOR),
            tab: ParameterTab::Absorption,
            section: None,
            order: 7,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("absorption_enabled"),
        },
        ParameterDef {
            key: "absorption_zone_opacity".into(),
            label: "Zone Opacity".into(),
            description: "Base opacity for absorption zone fill".into(),
            kind: ParameterKind::Float {
                min: 0.05,
                max: 1.0,
                step: 0.05,
            },
            default: ParameterValue::Float(0.30),
            tab: ParameterTab::Absorption,
            section: None,
            order: 8,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("absorption_enabled"),
        },
        ParameterDef {
            key: "absorption_show_labels".into(),
            label: "Show Labels".into(),
            description: "Show volume label on absorption zones".into(),
            kind: ParameterKind::Boolean,
            default: ParameterValue::Boolean(true),
            tab: ParameterTab::Absorption,
            section: None,
            order: 9,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("absorption_enabled"),
        },
    ]
}
