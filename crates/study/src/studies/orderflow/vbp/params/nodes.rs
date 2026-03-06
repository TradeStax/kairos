//! Peak & Valley tab: detection, HVN/LVN zones, peak/valley lines,
//! developing peak/valley.

use crate::config::{
    DisplayFormat, LineStyleValue, ParameterDef, ParameterKind, ParameterSection, ParameterTab,
    ParameterValue, Visibility,
};

use super::{
    DEFAULT_DEV_PEAK_COLOR, DEFAULT_DEV_VALLEY_COLOR, DEFAULT_HVN_ZONE_COLOR,
    DEFAULT_LVN_ZONE_COLOR, DEFAULT_PEAK_COLOR, DEFAULT_VALLEY_COLOR,
};

/// Peak & Valley tab: detection, HVN/LVN zones, peak/valley lines,
/// developing peak/valley.
pub(super) fn build_nodes_tab_params(params: &mut Vec<ParameterDef>) {
    // Detection section (always visible)
    let det_section = Some(ParameterSection {
        label: "Detection",
        order: 0,
    });
    params.push(ParameterDef {
        key: "node_hvn_method".into(),
        label: "HVN Method".into(),
        description: "HVN detection method".into(),
        kind: ParameterKind::Choice {
            options: &["Percentile", "Relative", "Std Dev"],
        },
        default: ParameterValue::Choice("Percentile".to_string()),
        tab: ParameterTab::Nodes,
        section: det_section,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "node_hvn_threshold".into(),
        label: "HVN Threshold".into(),
        description: "HVN detection threshold".into(),
        kind: ParameterKind::Float {
            min: 0.1,
            max: 1.0,
            step: 0.05,
        },
        default: ParameterValue::Float(0.85),
        tab: ParameterTab::Nodes,
        section: det_section,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "node_lvn_method".into(),
        label: "LVN Method".into(),
        description: "LVN detection method".into(),
        kind: ParameterKind::Choice {
            options: &["Percentile", "Relative", "Std Dev"],
        },
        default: ParameterValue::Choice("Percentile".to_string()),
        tab: ParameterTab::Nodes,
        section: det_section,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "node_lvn_threshold".into(),
        label: "LVN Threshold".into(),
        description: "LVN detection threshold".into(),
        kind: ParameterKind::Float {
            min: 0.1,
            max: 1.0,
            step: 0.05,
        },
        default: ParameterValue::Float(0.15),
        tab: ParameterTab::Nodes,
        section: det_section,
        order: 3,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "node_min_prominence".into(),
        label: "Min Prominence".into(),
        description: "Minimum prominence to qualify".into(),
        kind: ParameterKind::Float {
            min: 0.0,
            max: 1.0,
            step: 0.05,
        },
        default: ParameterValue::Float(0.15),
        tab: ParameterTab::Nodes,
        section: det_section,
        order: 4,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });

    // Section 1: HVN Zones
    let hvn_zone_section = Some(ParameterSection {
        label: "HVN Zones",
        order: 1,
    });
    params.push(ParameterDef {
        key: "hvn_zone_show".into(),
        label: "Show HVN Zones".into(),
        description: "Show high volume zone shading".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::Nodes,
        section: hvn_zone_section,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "hvn_zone_color".into(),
        label: "Color".into(),
        description: "HVN zone fill color".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_HVN_ZONE_COLOR),
        tab: ParameterTab::Nodes,
        section: hvn_zone_section,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("hvn_zone_show"),
    });
    params.push(ParameterDef {
        key: "hvn_zone_opacity".into(),
        label: "Opacity".into(),
        description: "HVN zone fill opacity".into(),
        kind: ParameterKind::Float {
            min: 0.02,
            max: 0.3,
            step: 0.02,
        },
        default: ParameterValue::Float(0.08),
        tab: ParameterTab::Nodes,
        section: hvn_zone_section,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("hvn_zone_show"),
    });

    // Section 2: Peak Line
    let peak_section = Some(ParameterSection {
        label: "Peak Line",
        order: 2,
    });
    params.push(ParameterDef {
        key: "peak_show".into(),
        label: "Show Peak".into(),
        description: "Show dominant peak line".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::Nodes,
        section: peak_section,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "peak_color".into(),
        label: "Color".into(),
        description: "Peak line color".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_PEAK_COLOR),
        tab: ParameterTab::Nodes,
        section: peak_section,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("peak_show"),
    });
    params.push(ParameterDef {
        key: "peak_line_style".into(),
        label: "Style".into(),
        description: "Peak line style".into(),
        kind: ParameterKind::LineStyle,
        default: ParameterValue::LineStyle(LineStyleValue::Solid),
        tab: ParameterTab::Nodes,
        section: peak_section,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("peak_show"),
    });
    params.push(ParameterDef {
        key: "peak_line_width".into(),
        label: "Width".into(),
        description: "Peak line width".into(),
        kind: ParameterKind::Float {
            min: 0.5,
            max: 4.0,
            step: 0.5,
        },
        default: ParameterValue::Float(1.5),
        tab: ParameterTab::Nodes,
        section: peak_section,
        order: 3,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("peak_show"),
    });
    params.push(ParameterDef {
        key: "peak_extend".into(),
        label: "Extend".into(),
        description: "Extend peak line".into(),
        kind: ParameterKind::Choice {
            options: &["None", "Left", "Right", "Both"],
        },
        default: ParameterValue::Choice("None".to_string()),
        tab: ParameterTab::Nodes,
        section: peak_section,
        order: 4,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("peak_show"),
    });
    params.push(ParameterDef {
        key: "peak_show_label".into(),
        label: "Show Label".into(),
        description: "Show label at peak line".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::Nodes,
        section: peak_section,
        order: 5,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("peak_show"),
    });

    // Section 3: Developing Peak
    let dev_peak_section = Some(ParameterSection {
        label: "Developing Peak",
        order: 3,
    });
    params.push(ParameterDef {
        key: "dev_peak_show".into(),
        label: "Show Developing Peak".into(),
        description: "Show developing peak polyline".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::Nodes,
        section: dev_peak_section,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "dev_peak_color".into(),
        label: "Color".into(),
        description: "Developing peak color".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_DEV_PEAK_COLOR),
        tab: ParameterTab::Nodes,
        section: dev_peak_section,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("dev_peak_show"),
    });
    params.push(ParameterDef {
        key: "dev_peak_line_width".into(),
        label: "Width".into(),
        description: "Developing peak line width".into(),
        kind: ParameterKind::Float {
            min: 0.5,
            max: 4.0,
            step: 0.5,
        },
        default: ParameterValue::Float(1.0),
        tab: ParameterTab::Nodes,
        section: dev_peak_section,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("dev_peak_show"),
    });
    params.push(ParameterDef {
        key: "dev_peak_line_style".into(),
        label: "Style".into(),
        description: "Developing peak line style".into(),
        kind: ParameterKind::LineStyle,
        default: ParameterValue::LineStyle(LineStyleValue::Dashed),
        tab: ParameterTab::Nodes,
        section: dev_peak_section,
        order: 3,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("dev_peak_show"),
    });

    // Section 4: LVN Zones
    let lvn_zone_section = Some(ParameterSection {
        label: "LVN Zones",
        order: 4,
    });
    params.push(ParameterDef {
        key: "lvn_zone_show".into(),
        label: "Show LVN Zones".into(),
        description: "Show low volume zone shading".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::Nodes,
        section: lvn_zone_section,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "lvn_zone_color".into(),
        label: "Color".into(),
        description: "LVN zone fill color".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_LVN_ZONE_COLOR),
        tab: ParameterTab::Nodes,
        section: lvn_zone_section,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("lvn_zone_show"),
    });
    params.push(ParameterDef {
        key: "lvn_zone_opacity".into(),
        label: "Opacity".into(),
        description: "LVN zone fill opacity".into(),
        kind: ParameterKind::Float {
            min: 0.02,
            max: 0.3,
            step: 0.02,
        },
        default: ParameterValue::Float(0.08),
        tab: ParameterTab::Nodes,
        section: lvn_zone_section,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("lvn_zone_show"),
    });

    // Section 5: Valley Line
    let valley_section = Some(ParameterSection {
        label: "Valley Line",
        order: 5,
    });
    params.push(ParameterDef {
        key: "valley_show".into(),
        label: "Show Valley".into(),
        description: "Show deepest valley line".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::Nodes,
        section: valley_section,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "valley_color".into(),
        label: "Color".into(),
        description: "Valley line color".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_VALLEY_COLOR),
        tab: ParameterTab::Nodes,
        section: valley_section,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("valley_show"),
    });
    params.push(ParameterDef {
        key: "valley_line_style".into(),
        label: "Style".into(),
        description: "Valley line style".into(),
        kind: ParameterKind::LineStyle,
        default: ParameterValue::LineStyle(LineStyleValue::Solid),
        tab: ParameterTab::Nodes,
        section: valley_section,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("valley_show"),
    });
    params.push(ParameterDef {
        key: "valley_line_width".into(),
        label: "Width".into(),
        description: "Valley line width".into(),
        kind: ParameterKind::Float {
            min: 0.5,
            max: 4.0,
            step: 0.5,
        },
        default: ParameterValue::Float(1.5),
        tab: ParameterTab::Nodes,
        section: valley_section,
        order: 3,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("valley_show"),
    });
    params.push(ParameterDef {
        key: "valley_extend".into(),
        label: "Extend".into(),
        description: "Extend valley line".into(),
        kind: ParameterKind::Choice {
            options: &["None", "Left", "Right", "Both"],
        },
        default: ParameterValue::Choice("None".to_string()),
        tab: ParameterTab::Nodes,
        section: valley_section,
        order: 4,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("valley_show"),
    });
    params.push(ParameterDef {
        key: "valley_show_label".into(),
        label: "Show Label".into(),
        description: "Show label at valley line".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::Nodes,
        section: valley_section,
        order: 5,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("valley_show"),
    });

    // Section 6: Developing Valley
    let dev_valley_section = Some(ParameterSection {
        label: "Developing Valley",
        order: 6,
    });
    params.push(ParameterDef {
        key: "dev_valley_show".into(),
        label: "Show Developing Valley".into(),
        description: "Show developing valley polyline".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::Nodes,
        section: dev_valley_section,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "dev_valley_color".into(),
        label: "Color".into(),
        description: "Developing valley color".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_DEV_VALLEY_COLOR),
        tab: ParameterTab::Nodes,
        section: dev_valley_section,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("dev_valley_show"),
    });
    params.push(ParameterDef {
        key: "dev_valley_line_width".into(),
        label: "Width".into(),
        description: "Developing valley line width".into(),
        kind: ParameterKind::Float {
            min: 0.5,
            max: 4.0,
            step: 0.5,
        },
        default: ParameterValue::Float(1.0),
        tab: ParameterTab::Nodes,
        section: dev_valley_section,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("dev_valley_show"),
    });
    params.push(ParameterDef {
        key: "dev_valley_line_style".into(),
        label: "Style".into(),
        description: "Developing valley line style".into(),
        kind: ParameterKind::LineStyle,
        default: ParameterValue::LineStyle(LineStyleValue::Dashed),
        tab: ParameterTab::Nodes,
        section: dev_valley_section,
        order: 3,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("dev_valley_show"),
    });
}
