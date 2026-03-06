//! Data tab: location, VBP type, period, tick grouping, value area %.

use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterSection, ParameterTab, ParameterValue,
    Visibility,
};

/// Data tab: location, VBP type, period, tick grouping, value area %.
pub(super) fn build_data_tab_params(params: &mut Vec<ParameterDef>) {
    // Location section
    let location_section = Some(ParameterSection {
        label: "Location",
        order: 0,
    });
    params.push(ParameterDef {
        key: "display_location".into(),
        label: "Location".into(),
        description: "Where to render the volume profile".into(),
        kind: ParameterKind::Choice {
            options: &["In Chart", "Side Panel"],
        },
        default: ParameterValue::Choice("In Chart".to_string()),
        tab: ParameterTab::Parameters,
        section: location_section,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "side_panel_cumulative".into(),
        label: "Cumulative".into(),
        description: "Merge all periods into a single cumulative profile".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(true),
        tab: ParameterTab::Parameters,
        section: location_section,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenChoice {
            key: "display_location",
            equals: "Side Panel",
        },
    });

    params.push(ParameterDef {
        key: "vbp_type".into(),
        label: "VBP Type".into(),
        description: "Volume visualization type".into(),
        kind: ParameterKind::Choice {
            options: &[
                "Volume",
                "Bid/Ask Volume",
                "Delta",
                "Delta & Total Volume",
                "Delta Percentage",
            ],
        },
        default: ParameterValue::Choice("Volume".to_string()),
        tab: ParameterTab::Parameters,
        section: None,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });

    // Period section
    let period_section = Some(ParameterSection {
        label: "Period",
        order: 1,
    });
    params.push(ParameterDef {
        key: "period".into(),
        label: "Period".into(),
        description: "Time period for volume calculation".into(),
        kind: ParameterKind::Choice {
            options: &["Split", "Custom"],
        },
        default: ParameterValue::Choice("Split".to_string()),
        tab: ParameterTab::Parameters,
        section: period_section,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "split_interval".into(),
        label: "Split Interval".into(),
        description: "How to split the profile into segments".into(),
        kind: ParameterKind::Choice {
            options: &[
                "1 Day",
                "4 Hours",
                "2 Hours",
                "1 Hour",
                "30 Minutes",
                "15 Minutes",
                "Custom",
            ],
        },
        default: ParameterValue::Choice("1 Day".to_string()),
        tab: ParameterTab::Parameters,
        section: period_section,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenChoice {
            key: "period",
            equals: "Split",
        },
    });
    params.push(ParameterDef {
        key: "split_unit".into(),
        label: "Split Unit".into(),
        description: "Unit for custom split interval".into(),
        kind: ParameterKind::Choice {
            options: &["Days", "Hours", "Minutes", "Contracts"],
        },
        default: ParameterValue::Choice("Hours".to_string()),
        tab: ParameterTab::Parameters,
        section: period_section,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenChoice {
            key: "split_interval",
            equals: "Custom",
        },
    });
    params.push(ParameterDef {
        key: "split_value".into(),
        label: "Split Value".into(),
        description: "Number of units per split segment".into(),
        kind: ParameterKind::Integer { min: 1, max: 1000 },
        default: ParameterValue::Integer(1),
        tab: ParameterTab::Parameters,
        section: period_section,
        order: 3,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenChoice {
            key: "split_interval",
            equals: "Custom",
        },
    });
    params.push(ParameterDef {
        key: "max_profiles".into(),
        label: "Max Profiles".into(),
        description: "Maximum number of profile segments".into(),
        kind: ParameterKind::Integer { min: 1, max: 100 },
        default: ParameterValue::Integer(20),
        tab: ParameterTab::Parameters,
        section: period_section,
        order: 4,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenChoice {
            key: "period",
            equals: "Split",
        },
    });
    params.push(ParameterDef {
        key: "custom_start".into(),
        label: "Start Date/Time".into(),
        description: "Custom start (epoch millis)".into(),
        kind: ParameterKind::Integer {
            min: 0,
            max: i64::MAX,
        },
        default: ParameterValue::Integer(0),
        tab: ParameterTab::Parameters,
        section: period_section,
        order: 5,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenChoice {
            key: "period",
            equals: "Custom",
        },
    });
    params.push(ParameterDef {
        key: "custom_end".into(),
        label: "End Date/Time".into(),
        description: "Custom end (epoch millis)".into(),
        kind: ParameterKind::Integer {
            min: 0,
            max: i64::MAX,
        },
        default: ParameterValue::Integer(0),
        tab: ParameterTab::Parameters,
        section: period_section,
        order: 6,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenChoice {
            key: "period",
            equals: "Custom",
        },
    });

    // Tick Grouping section
    let grouping_section = Some(ParameterSection {
        label: "Tick Grouping",
        order: 1,
    });
    params.push(ParameterDef {
        key: "auto_grouping".into(),
        label: "Grouping".into(),
        description: "Automatic or Manual tick grouping".into(),
        kind: ParameterKind::Choice {
            options: &["Automatic", "Manual"],
        },
        default: ParameterValue::Choice("Automatic".to_string()),
        tab: ParameterTab::Parameters,
        section: grouping_section,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "auto_group_factor".into(),
        label: "Auto Group Factor".into(),
        description: "Tick size multiplier for automatic grouping".into(),
        kind: ParameterKind::Integer { min: 1, max: 100 },
        default: ParameterValue::Integer(1),
        tab: ParameterTab::Parameters,
        section: grouping_section,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenChoice {
            key: "auto_grouping",
            equals: "Automatic",
        },
    });
    params.push(ParameterDef {
        key: "manual_ticks".into(),
        label: "Manual Ticks".into(),
        description: "Number of ticks to group together".into(),
        kind: ParameterKind::Integer { min: 1, max: 100 },
        default: ParameterValue::Integer(1),
        tab: ParameterTab::Parameters,
        section: grouping_section,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenChoice {
            key: "auto_grouping",
            equals: "Manual",
        },
    });

    // Value area percentage (standalone)
    params.push(ParameterDef {
        key: "value_area_pct".into(),
        label: "Value Area %".into(),
        description: "Percentage of volume in Value Area".into(),
        kind: ParameterKind::Float {
            min: 0.5,
            max: 0.95,
            step: 0.05,
        },
        default: ParameterValue::Float(0.7),
        tab: ParameterTab::Parameters,
        section: None,
        order: 2,
        format: DisplayFormat::Percent,
        visible_when: Visibility::Always,
    });
}
