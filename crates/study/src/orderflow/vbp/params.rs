//! VBP parameter definitions and default color constants.
//!
//! Contains the full set of `ParameterDef` arrays for all VBP
//! tabs (Data, Style, POC, Value Area, Peak & Valley, VWAP)
//! and the default color constants used across the study.

use crate::config::{
    DisplayFormat, LineStyleValue, ParameterDef, ParameterKind,
    ParameterSection, ParameterTab, ParameterValue, Visibility,
};
use crate::{BEARISH_COLOR, BULLISH_COLOR};
use data::SerializableColor;

pub(super) const DEFAULT_VOLUME_COLOR: SerializableColor =
    SerializableColor {
        r: 0.95,
        g: 0.55,
        b: 0.15,
        a: 0.7,
    };
pub(super) const DEFAULT_BID_COLOR: SerializableColor =
    BEARISH_COLOR.with_alpha(0.7);
pub(super) const DEFAULT_ASK_COLOR: SerializableColor =
    BULLISH_COLOR.with_alpha(0.7);
pub(super) const DEFAULT_POC_COLOR: SerializableColor =
    SerializableColor {
        r: 1.0,
        g: 0.84,
        b: 0.0,
        a: 1.0,
    };
pub(super) const DEFAULT_DEV_POC_COLOR: SerializableColor =
    SerializableColor {
        r: 1.0,
        g: 0.84,
        b: 0.0,
        a: 0.5,
    };
pub(super) const DEFAULT_VAH_COLOR: SerializableColor =
    SerializableColor {
        r: 0.0,
        g: 0.7,
        b: 1.0,
        a: 0.8,
    };
pub(super) const DEFAULT_VAL_COLOR: SerializableColor =
    SerializableColor {
        r: 0.0,
        g: 0.7,
        b: 1.0,
        a: 0.8,
    };
pub(super) const DEFAULT_VA_FILL_COLOR: SerializableColor =
    SerializableColor {
        r: 0.0,
        g: 0.7,
        b: 1.0,
        a: 0.15,
    };
pub(super) const DEFAULT_PEAK_COLOR: SerializableColor =
    BULLISH_COLOR.with_alpha(0.8);
pub(super) const DEFAULT_DEV_PEAK_COLOR: SerializableColor =
    BULLISH_COLOR.with_alpha(0.5);
pub(super) const DEFAULT_HVN_ZONE_COLOR: SerializableColor =
    BULLISH_COLOR.with_alpha(0.5);
pub(super) const DEFAULT_VALLEY_COLOR: SerializableColor =
    BEARISH_COLOR.with_alpha(0.8);
pub(super) const DEFAULT_DEV_VALLEY_COLOR: SerializableColor =
    BEARISH_COLOR.with_alpha(0.5);
pub(super) const DEFAULT_LVN_ZONE_COLOR: SerializableColor =
    BEARISH_COLOR.with_alpha(0.5);
pub(super) const DEFAULT_VWAP_COLOR: SerializableColor =
    SerializableColor {
        r: 0.0,
        g: 0.9,
        b: 0.9,
        a: 1.0,
    };
pub(super) const DEFAULT_VWAP_BAND_COLOR: SerializableColor =
    SerializableColor {
        r: 0.0,
        g: 0.9,
        b: 0.9,
        a: 0.4,
    };

/// Build the full parameter definition list for VbpStudy.
pub(super) fn build_params() -> Vec<ParameterDef> {
    let mut params = Vec::with_capacity(72);

    // ── Data Tab (Parameters) ─────────────────────────────────

    // Location section (before Period)
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
        description: "How to split the profile into segments"
            .into(),
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
            options: &[
                "Days", "Hours", "Minutes", "Contracts",
            ],
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
        default: ParameterValue::Choice(
            "Automatic".to_string(),
        ),
        tab: ParameterTab::Parameters,
        section: grouping_section,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "auto_group_factor".into(),
        label: "Auto Group Factor".into(),
        description:
            "Tick size multiplier for automatic grouping".into(),
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

    // Value area percentage (Data tab, standalone)
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

    // ── Style Tab ─────────────────────────────────────────────
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
        description: "Profile width as fraction of segment"
            .into(),
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

    // ── POC Tab (Tab4) ────────────────────────────────────────
    let poc_line_section = Some(ParameterSection {
        label: "POC Line",
        order: 0,
    });
    params.push(ParameterDef {
        key: "poc_show".into(),
        label: "Show POC".into(),
        description: "Show Point of Control line".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(true),
        tab: ParameterTab::PocSettings,
        section: poc_line_section,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "poc_color".into(),
        label: "Color".into(),
        description: "POC line color".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_POC_COLOR),
        tab: ParameterTab::PocSettings,
        section: poc_line_section,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("poc_show"),
    });
    params.push(ParameterDef {
        key: "poc_line_width".into(),
        label: "Line Width".into(),
        description: "POC line width".into(),
        kind: ParameterKind::Float {
            min: 0.5,
            max: 4.0,
            step: 0.5,
        },
        default: ParameterValue::Float(1.5),
        tab: ParameterTab::PocSettings,
        section: poc_line_section,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("poc_show"),
    });
    params.push(ParameterDef {
        key: "poc_line_style".into(),
        label: "Line Style".into(),
        description: "POC line style".into(),
        kind: ParameterKind::LineStyle,
        default: ParameterValue::LineStyle(
            LineStyleValue::Solid,
        ),
        tab: ParameterTab::PocSettings,
        section: poc_line_section,
        order: 3,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("poc_show"),
    });
    params.push(ParameterDef {
        key: "poc_extend".into(),
        label: "Extend".into(),
        description: "Extend POC line beyond profile".into(),
        kind: ParameterKind::Choice {
            options: &["None", "Left", "Right", "Both"],
        },
        default: ParameterValue::Choice("None".to_string()),
        tab: ParameterTab::PocSettings,
        section: poc_line_section,
        order: 4,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("poc_show"),
    });
    params.push(ParameterDef {
        key: "poc_show_label".into(),
        label: "Show Label".into(),
        description: "Show price label at POC".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::PocSettings,
        section: poc_line_section,
        order: 5,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("poc_show"),
    });

    // Developing POC section
    let dev_poc_section = Some(ParameterSection {
        label: "Developing POC",
        order: 1,
    });
    params.push(ParameterDef {
        key: "poc_show_developing".into(),
        label: "Show Developing POC".into(),
        description: "Show developing POC line".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::PocSettings,
        section: dev_poc_section,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "poc_dev_color".into(),
        label: "Color".into(),
        description: "Developing POC line color".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_DEV_POC_COLOR),
        tab: ParameterTab::PocSettings,
        section: dev_poc_section,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue(
            "poc_show_developing",
        ),
    });
    params.push(ParameterDef {
        key: "poc_dev_line_width".into(),
        label: "Line Width".into(),
        description: "Developing POC line width".into(),
        kind: ParameterKind::Float {
            min: 0.5,
            max: 4.0,
            step: 0.5,
        },
        default: ParameterValue::Float(1.0),
        tab: ParameterTab::PocSettings,
        section: dev_poc_section,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue(
            "poc_show_developing",
        ),
    });
    params.push(ParameterDef {
        key: "poc_dev_line_style".into(),
        label: "Line Style".into(),
        description: "Developing POC line style".into(),
        kind: ParameterKind::LineStyle,
        default: ParameterValue::LineStyle(
            LineStyleValue::Dashed,
        ),
        tab: ParameterTab::PocSettings,
        section: dev_poc_section,
        order: 3,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue(
            "poc_show_developing",
        ),
    });

    // ── Value Area Tab (Tab5) ─────────────────────────────────
    let va_section = Some(ParameterSection {
        label: "Value Area",
        order: 0,
    });
    params.push(ParameterDef {
        key: "va_show".into(),
        label: "Show Value Area".into(),
        description: "Enable value area features".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(true),
        tab: ParameterTab::ValueArea,
        section: va_section,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "va_show_highlight".into(),
        label: "Dim Outside VA".into(),
        description: "Dim bars outside value area".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(true),
        tab: ParameterTab::ValueArea,
        section: va_section,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show"),
    });

    // VA Lines section
    let va_lines = Some(ParameterSection {
        label: "Lines",
        order: 1,
    });
    params.push(ParameterDef {
        key: "va_vah_color".into(),
        label: "VAH Color".into(),
        description: "Value Area High line color".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_VAH_COLOR),
        tab: ParameterTab::ValueArea,
        section: va_lines,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show"),
    });
    params.push(ParameterDef {
        key: "va_vah_line_width".into(),
        label: "VAH Width".into(),
        description: "VAH line width".into(),
        kind: ParameterKind::Float {
            min: 0.5,
            max: 4.0,
            step: 0.5,
        },
        default: ParameterValue::Float(1.0),
        tab: ParameterTab::ValueArea,
        section: va_lines,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show"),
    });
    params.push(ParameterDef {
        key: "va_vah_line_style".into(),
        label: "VAH Style".into(),
        description: "VAH line style".into(),
        kind: ParameterKind::LineStyle,
        default: ParameterValue::LineStyle(
            LineStyleValue::Dashed,
        ),
        tab: ParameterTab::ValueArea,
        section: va_lines,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show"),
    });
    params.push(ParameterDef {
        key: "va_val_color".into(),
        label: "VAL Color".into(),
        description: "Value Area Low line color".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_VAL_COLOR),
        tab: ParameterTab::ValueArea,
        section: va_lines,
        order: 3,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show"),
    });
    params.push(ParameterDef {
        key: "va_val_line_width".into(),
        label: "VAL Width".into(),
        description: "VAL line width".into(),
        kind: ParameterKind::Float {
            min: 0.5,
            max: 4.0,
            step: 0.5,
        },
        default: ParameterValue::Float(1.0),
        tab: ParameterTab::ValueArea,
        section: va_lines,
        order: 4,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show"),
    });
    params.push(ParameterDef {
        key: "va_val_line_style".into(),
        label: "VAL Style".into(),
        description: "VAL line style".into(),
        kind: ParameterKind::LineStyle,
        default: ParameterValue::LineStyle(
            LineStyleValue::Dashed,
        ),
        tab: ParameterTab::ValueArea,
        section: va_lines,
        order: 5,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show"),
    });
    params.push(ParameterDef {
        key: "va_extend".into(),
        label: "Extend".into(),
        description: "Extend VA lines beyond profile".into(),
        kind: ParameterKind::Choice {
            options: &["None", "Left", "Right", "Both"],
        },
        default: ParameterValue::Choice("None".to_string()),
        tab: ParameterTab::ValueArea,
        section: va_lines,
        order: 6,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show"),
    });
    params.push(ParameterDef {
        key: "va_show_labels".into(),
        label: "Show Labels".into(),
        description: "Show price labels at VA boundaries"
            .into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::ValueArea,
        section: va_lines,
        order: 7,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show"),
    });

    // VA Fill section
    let va_fill = Some(ParameterSection {
        label: "Fill",
        order: 2,
    });
    params.push(ParameterDef {
        key: "va_show_fill".into(),
        label: "Show Fill".into(),
        description: "Fill between VAH and VAL".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::ValueArea,
        section: va_fill,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show"),
    });
    params.push(ParameterDef {
        key: "va_fill_color".into(),
        label: "Fill Color".into(),
        description: "VA fill color".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_VA_FILL_COLOR),
        tab: ParameterTab::ValueArea,
        section: va_fill,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show_fill"),
    });
    params.push(ParameterDef {
        key: "va_fill_opacity".into(),
        label: "Fill Opacity".into(),
        description: "VA fill opacity".into(),
        kind: ParameterKind::Float {
            min: 0.0,
            max: 0.5,
            step: 0.05,
        },
        default: ParameterValue::Float(0.15),
        tab: ParameterTab::ValueArea,
        section: va_fill,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("va_show_fill"),
    });

    // ── Peak & Valley Tab (Tab6) ──────────────────────────────

    // Section 0: Detection (always visible)
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
        default: ParameterValue::Choice(
            "Percentile".to_string(),
        ),
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
        default: ParameterValue::Choice(
            "Percentile".to_string(),
        ),
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
        default: ParameterValue::Color(
            DEFAULT_HVN_ZONE_COLOR,
        ),
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
        default: ParameterValue::LineStyle(
            LineStyleValue::Solid,
        ),
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
        default: ParameterValue::Color(
            DEFAULT_DEV_PEAK_COLOR,
        ),
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
        default: ParameterValue::LineStyle(
            LineStyleValue::Dashed,
        ),
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
        default: ParameterValue::Color(
            DEFAULT_LVN_ZONE_COLOR,
        ),
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
        default: ParameterValue::LineStyle(
            LineStyleValue::Solid,
        ),
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
        default: ParameterValue::Color(
            DEFAULT_DEV_VALLEY_COLOR,
        ),
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
        default: ParameterValue::LineStyle(
            LineStyleValue::Dashed,
        ),
        tab: ParameterTab::Nodes,
        section: dev_valley_section,
        order: 3,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("dev_valley_show"),
    });

    // ── VWAP Tab (Tab7) ───────────────────────────────────────
    let vwap_section = Some(ParameterSection {
        label: "VWAP Line",
        order: 0,
    });
    params.push(ParameterDef {
        key: "vwap_show".into(),
        label: "Show VWAP".into(),
        description: "Show anchored VWAP line".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::Vwap,
        section: vwap_section,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::Always,
    });
    params.push(ParameterDef {
        key: "vwap_color".into(),
        label: "Color".into(),
        description: "VWAP line color".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(DEFAULT_VWAP_COLOR),
        tab: ParameterTab::Vwap,
        section: vwap_section,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("vwap_show"),
    });
    params.push(ParameterDef {
        key: "vwap_line_width".into(),
        label: "Line Width".into(),
        description: "VWAP line width".into(),
        kind: ParameterKind::Float {
            min: 0.5,
            max: 4.0,
            step: 0.5,
        },
        default: ParameterValue::Float(1.5),
        tab: ParameterTab::Vwap,
        section: vwap_section,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("vwap_show"),
    });
    params.push(ParameterDef {
        key: "vwap_line_style".into(),
        label: "Line Style".into(),
        description: "VWAP line style".into(),
        kind: ParameterKind::LineStyle,
        default: ParameterValue::LineStyle(
            LineStyleValue::Solid,
        ),
        tab: ParameterTab::Vwap,
        section: vwap_section,
        order: 3,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("vwap_show"),
    });
    params.push(ParameterDef {
        key: "vwap_show_label".into(),
        label: "Show Label".into(),
        description: "Show VWAP price label".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::Vwap,
        section: vwap_section,
        order: 4,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("vwap_show"),
    });

    // VWAP Bands section
    let vwap_bands = Some(ParameterSection {
        label: "Bands",
        order: 1,
    });
    params.push(ParameterDef {
        key: "vwap_show_bands".into(),
        label: "Show Bands".into(),
        description: "Show std dev bands around VWAP".into(),
        kind: ParameterKind::Boolean,
        default: ParameterValue::Boolean(false),
        tab: ParameterTab::Vwap,
        section: vwap_bands,
        order: 0,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("vwap_show"),
    });
    params.push(ParameterDef {
        key: "vwap_band_multiplier".into(),
        label: "Multiplier".into(),
        description: "Std dev multiplier for bands".into(),
        kind: ParameterKind::Float {
            min: 1.0,
            max: 3.0,
            step: 0.5,
        },
        default: ParameterValue::Float(1.0),
        tab: ParameterTab::Vwap,
        section: vwap_bands,
        order: 1,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("vwap_show_bands"),
    });
    params.push(ParameterDef {
        key: "vwap_band_color".into(),
        label: "Band Color".into(),
        description: "VWAP band color".into(),
        kind: ParameterKind::Color,
        default: ParameterValue::Color(
            DEFAULT_VWAP_BAND_COLOR,
        ),
        tab: ParameterTab::Vwap,
        section: vwap_bands,
        order: 2,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("vwap_show_bands"),
    });
    params.push(ParameterDef {
        key: "vwap_band_line_style".into(),
        label: "Band Style".into(),
        description: "VWAP band line style".into(),
        kind: ParameterKind::LineStyle,
        default: ParameterValue::LineStyle(
            LineStyleValue::Dashed,
        ),
        tab: ParameterTab::Vwap,
        section: vwap_bands,
        order: 3,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("vwap_show_bands"),
    });
    params.push(ParameterDef {
        key: "vwap_band_line_width".into(),
        label: "Band Width".into(),
        description: "VWAP band line width".into(),
        kind: ParameterKind::Float {
            min: 0.5,
            max: 4.0,
            step: 0.5,
        },
        default: ParameterValue::Float(1.0),
        tab: ParameterTab::Vwap,
        section: vwap_bands,
        order: 4,
        format: DisplayFormat::Auto,
        visible_when: Visibility::WhenTrue("vwap_show_bands"),
    });

    params
}
