//! Style tab: colors, width, opacity, alignment.

use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterSection, ParameterTab, ParameterValue,
    Visibility,
};

use super::{DEFAULT_ASK_COLOR, DEFAULT_BID_COLOR, DEFAULT_VOLUME_COLOR};

/// Style tab: colors, width, opacity, alignment.
pub(super) fn build_style_tab_params(params: &mut Vec<ParameterDef>) {
    let color_section = Some(ParameterSection {
        label: "Colors",
        order: 0,
    });
    params.push(ParameterDef {
        key: "volume_color".into(),
        label: "Volume Color".into(),
        description: "Color for total volume bars".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_VOLUME_COLOR),
        tab: ParameterTab::Style,
        section: color_section,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "bid_color".into(),
        label: "Bid Color".into(),
        description: "Color for buy volume".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_BID_COLOR),
        tab: ParameterTab::Style,
        section: color_section,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "ask_color".into(),
        label: "Ask Color".into(),
        description: "Color for sell volume".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_ASK_COLOR),
        tab: ParameterTab::Style,
        section: color_section,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "width_pct".into(),
        label: "Width %".into(),
        description: "Profile width as fraction of segment".into(),
        kind: ParameterKind::Float {
            min: 0.1,
            max: 1.0,
            step: 0.05,
        },
        default: ParameterValue::Float(0.7),
        tab: ParameterTab::Style,
        section: None,
        order: 4,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "opacity".into(),
        label: "Opacity".into(),
        description: "Bar opacity".into(),
        kind: ParameterKind::Float {
            min: 0.1,
            max: 1.0,
            step: 0.05,
        },
        default: ParameterValue::Float(0.7),
        tab: ParameterTab::Style,
        section: None,
        order: 5,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "alignment".into(),
        label: "Alignment".into(),
        description: "Side of chart for bars".into(),
        kind: ParameterKind::Choice {
            options: &["Left", "Right"],
        },
        default: ParameterValue::Choice("Left".to_string()),
        tab: ParameterTab::Style,
        section: None,
        order: 6,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
}
