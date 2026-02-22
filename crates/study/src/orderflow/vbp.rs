//! Volume-by-Price (VBP) Study
//!
//! Renders horizontal volume distribution bars at each price level on the
//! chart background, supporting 5 visualization types, configurable time
//! periods, POC/Value Area overlays, and full color/style customization.
//!
//! Integrated features: POC line, developing POC, value area lines/fill,
//! HVN/LVN detection, and anchored VWAP with standard deviation bands.

use crate::config::{
    DisplayFormat, LineStyleValue, ParameterDef, ParameterKind,
    ParameterSection, ParameterTab, ParameterValue, StudyConfig,
    Visibility,
};
use crate::error::StudyError;
use crate::orderflow::profile_core;
use crate::output::{
    ExtendDirection, NodeDetectionMethod, ProfileSide, StudyOutput,
    VbpData, VbpGroupingMode, VbpLengthUnit, VbpNodeConfig,
    VbpPeriod, VbpPocConfig, VbpType, VbpValueAreaConfig,
    VbpVwapConfig,
};
use crate::traits::{Study, StudyCategory, StudyInput, StudyPlacement};
use data::SerializableColor;

/// Time-series point: (timestamp_ms, value).
type TimeSeries = Vec<(u64, f32)>;

const DEFAULT_VOLUME_COLOR: SerializableColor = SerializableColor {
    r: 0.95,
    g: 0.55,
    b: 0.15,
    a: 0.7,
};
const DEFAULT_BID_COLOR: SerializableColor = SerializableColor {
    r: 0.18,
    g: 0.72,
    b: 0.45,
    a: 0.7,
};
const DEFAULT_ASK_COLOR: SerializableColor = SerializableColor {
    r: 0.65,
    g: 0.20,
    b: 0.70,
    a: 0.7,
};
const DEFAULT_POC_COLOR: SerializableColor = SerializableColor {
    r: 1.0,
    g: 0.84,
    b: 0.0,
    a: 1.0,
};
const DEFAULT_DEV_POC_COLOR: SerializableColor = SerializableColor {
    r: 1.0,
    g: 0.84,
    b: 0.0,
    a: 0.5,
};
const DEFAULT_VAH_COLOR: SerializableColor = SerializableColor {
    r: 0.0,
    g: 0.7,
    b: 1.0,
    a: 0.8,
};
const DEFAULT_VAL_COLOR: SerializableColor = SerializableColor {
    r: 0.0,
    g: 0.7,
    b: 1.0,
    a: 0.8,
};
const DEFAULT_VA_FILL_COLOR: SerializableColor = SerializableColor {
    r: 0.0,
    g: 0.7,
    b: 1.0,
    a: 0.15,
};
const DEFAULT_HVN_COLOR: SerializableColor = SerializableColor {
    r: 0.0,
    g: 0.9,
    b: 0.4,
    a: 0.8,
};
const DEFAULT_LVN_COLOR: SerializableColor = SerializableColor {
    r: 0.9,
    g: 0.2,
    b: 0.2,
    a: 0.8,
};
const DEFAULT_VWAP_COLOR: SerializableColor = SerializableColor {
    r: 0.0,
    g: 0.9,
    b: 0.9,
    a: 1.0,
};
const DEFAULT_VWAP_BAND_COLOR: SerializableColor = SerializableColor {
    r: 0.0,
    g: 0.9,
    b: 0.9,
    a: 0.4,
};

pub struct VbpStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
    /// Fingerprint of the last computed input to skip redundant
    /// recomputation when the underlying data hasn't changed.
    /// (candle_count, first_candle_ts, last_candle_ts, trade_count)
    last_input_fingerprint: (usize, u64, u64, usize),
    /// Cached visible range from the last full computation.
    /// Skips recompute when pan hasn't moved >25% of the span.
    last_stable_range: Option<(u64, u64)>,
}

impl VbpStudy {
    pub fn new() -> Self {
        let params = Self::build_params();

        let mut config = StudyConfig::new("vbp");
        for p in &params {
            config.set(p.key.clone(), p.default.clone());
        }

        Self {
            config,
            output: StudyOutput::Empty,
            params,
            last_input_fingerprint: (0, 0, 0, 0),
            last_stable_range: None,
        }
    }

    fn build_params() -> Vec<ParameterDef> {
        let mut params = Vec::with_capacity(70);

        // ── Data Tab (Parameters) ─────────────────────────────────
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
            order: 0,
        });
        params.push(ParameterDef {
            key: "period".into(),
            label: "Period".into(),
            description: "Time period for volume calculation".into(),
            kind: ParameterKind::Choice {
                options: &["Auto", "Length", "Custom"],
            },
            default: ParameterValue::Choice("Auto".to_string()),
            tab: ParameterTab::Parameters,
            section: period_section,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        });
        params.push(ParameterDef {
            key: "length_unit".into(),
            label: "Length Unit".into(),
            description: "Unit for length-based period".into(),
            kind: ParameterKind::Choice {
                options: &["Days", "Minutes", "Contracts"],
            },
            default: ParameterValue::Choice("Days".to_string()),
            tab: ParameterTab::Parameters,
            section: period_section,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenChoice {
                key: "period",
                equals: "Length",
            },
        });
        params.push(ParameterDef {
            key: "length_value".into(),
            label: "Length Value".into(),
            description: "Number of units for length-based period"
                .into(),
            kind: ParameterKind::Integer { min: 1, max: 1000 },
            default: ParameterValue::Integer(5),
            tab: ParameterTab::Parameters,
            section: period_section,
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenChoice {
                key: "period",
                equals: "Length",
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
            order: 3,
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
            order: 4,
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
            description: "Profile width as percentage of chart".into(),
            kind: ParameterKind::Float {
                min: 0.05,
                max: 0.5,
                step: 0.05,
            },
            default: ParameterValue::Float(0.25),
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
            tab: ParameterTab::Tab4,
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
            tab: ParameterTab::Tab4,
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
            tab: ParameterTab::Tab4,
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
            tab: ParameterTab::Tab4,
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
            tab: ParameterTab::Tab4,
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
            tab: ParameterTab::Tab4,
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
            tab: ParameterTab::Tab4,
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
            tab: ParameterTab::Tab4,
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
            tab: ParameterTab::Tab4,
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
            tab: ParameterTab::Tab4,
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
            tab: ParameterTab::Tab5,
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
            tab: ParameterTab::Tab5,
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
            tab: ParameterTab::Tab5,
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
            tab: ParameterTab::Tab5,
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
            tab: ParameterTab::Tab5,
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
            tab: ParameterTab::Tab5,
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
            tab: ParameterTab::Tab5,
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
            tab: ParameterTab::Tab5,
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
            tab: ParameterTab::Tab5,
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
            tab: ParameterTab::Tab5,
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
            tab: ParameterTab::Tab5,
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
            tab: ParameterTab::Tab5,
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
            tab: ParameterTab::Tab5,
            section: va_fill,
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("va_show_fill"),
        });

        // ── Peak & Valley Tab (Tab6) ──────────────────────────────
        let hvn_section = Some(ParameterSection {
            label: "HVN (High Volume Nodes)",
            order: 0,
        });
        params.push(ParameterDef {
            key: "hvn_show".into(),
            label: "Show HVN".into(),
            description: "Show high volume nodes".into(),
            kind: ParameterKind::Boolean,
            default: ParameterValue::Boolean(false),
            tab: ParameterTab::Tab6,
            section: hvn_section,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        });
        params.push(ParameterDef {
            key: "hvn_method".into(),
            label: "Method".into(),
            description: "HVN detection method".into(),
            kind: ParameterKind::Choice {
                options: &["Percentile", "Relative", "Std Dev"],
            },
            default: ParameterValue::Choice(
                "Percentile".to_string(),
            ),
            tab: ParameterTab::Tab6,
            section: hvn_section,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("hvn_show"),
        });
        params.push(ParameterDef {
            key: "hvn_threshold".into(),
            label: "Threshold".into(),
            description: "HVN detection threshold".into(),
            kind: ParameterKind::Float {
                min: 0.1,
                max: 1.0,
                step: 0.05,
            },
            default: ParameterValue::Float(0.85),
            tab: ParameterTab::Tab6,
            section: hvn_section,
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("hvn_show"),
        });
        params.push(ParameterDef {
            key: "hvn_color".into(),
            label: "Color".into(),
            description: "HVN line color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(DEFAULT_HVN_COLOR),
            tab: ParameterTab::Tab6,
            section: hvn_section,
            order: 3,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("hvn_show"),
        });
        params.push(ParameterDef {
            key: "hvn_line_style".into(),
            label: "Style".into(),
            description: "HVN line style".into(),
            kind: ParameterKind::LineStyle,
            default: ParameterValue::LineStyle(
                LineStyleValue::Dotted,
            ),
            tab: ParameterTab::Tab6,
            section: hvn_section,
            order: 4,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("hvn_show"),
        });
        params.push(ParameterDef {
            key: "hvn_line_width".into(),
            label: "Width".into(),
            description: "HVN line width".into(),
            kind: ParameterKind::Float {
                min: 0.5,
                max: 4.0,
                step: 0.5,
            },
            default: ParameterValue::Float(1.0),
            tab: ParameterTab::Tab6,
            section: hvn_section,
            order: 5,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("hvn_show"),
        });
        params.push(ParameterDef {
            key: "hvn_extend".into(),
            label: "Extend".into(),
            description: "Extend HVN lines".into(),
            kind: ParameterKind::Choice {
                options: &["None", "Left", "Right", "Both"],
            },
            default: ParameterValue::Choice("None".to_string()),
            tab: ParameterTab::Tab6,
            section: hvn_section,
            order: 6,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("hvn_show"),
        });
        params.push(ParameterDef {
            key: "hvn_show_labels".into(),
            label: "Show Labels".into(),
            description: "Show labels at HVN lines".into(),
            kind: ParameterKind::Boolean,
            default: ParameterValue::Boolean(false),
            tab: ParameterTab::Tab6,
            section: hvn_section,
            order: 7,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("hvn_show"),
        });

        // LVN section
        let lvn_section = Some(ParameterSection {
            label: "LVN (Low Volume Nodes)",
            order: 1,
        });
        params.push(ParameterDef {
            key: "lvn_show".into(),
            label: "Show LVN".into(),
            description: "Show low volume nodes".into(),
            kind: ParameterKind::Boolean,
            default: ParameterValue::Boolean(false),
            tab: ParameterTab::Tab6,
            section: lvn_section,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        });
        params.push(ParameterDef {
            key: "lvn_method".into(),
            label: "Method".into(),
            description: "LVN detection method".into(),
            kind: ParameterKind::Choice {
                options: &["Percentile", "Relative", "Std Dev"],
            },
            default: ParameterValue::Choice(
                "Percentile".to_string(),
            ),
            tab: ParameterTab::Tab6,
            section: lvn_section,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("lvn_show"),
        });
        params.push(ParameterDef {
            key: "lvn_threshold".into(),
            label: "Threshold".into(),
            description: "LVN detection threshold".into(),
            kind: ParameterKind::Float {
                min: 0.1,
                max: 1.0,
                step: 0.05,
            },
            default: ParameterValue::Float(0.15),
            tab: ParameterTab::Tab6,
            section: lvn_section,
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("lvn_show"),
        });
        params.push(ParameterDef {
            key: "lvn_color".into(),
            label: "Color".into(),
            description: "LVN line color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(DEFAULT_LVN_COLOR),
            tab: ParameterTab::Tab6,
            section: lvn_section,
            order: 3,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("lvn_show"),
        });
        params.push(ParameterDef {
            key: "lvn_line_style".into(),
            label: "Style".into(),
            description: "LVN line style".into(),
            kind: ParameterKind::LineStyle,
            default: ParameterValue::LineStyle(
                LineStyleValue::Dotted,
            ),
            tab: ParameterTab::Tab6,
            section: lvn_section,
            order: 4,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("lvn_show"),
        });
        params.push(ParameterDef {
            key: "lvn_line_width".into(),
            label: "Width".into(),
            description: "LVN line width".into(),
            kind: ParameterKind::Float {
                min: 0.5,
                max: 4.0,
                step: 0.5,
            },
            default: ParameterValue::Float(1.0),
            tab: ParameterTab::Tab6,
            section: lvn_section,
            order: 5,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("lvn_show"),
        });
        params.push(ParameterDef {
            key: "lvn_extend".into(),
            label: "Extend".into(),
            description: "Extend LVN lines".into(),
            kind: ParameterKind::Choice {
                options: &["None", "Left", "Right", "Both"],
            },
            default: ParameterValue::Choice("None".to_string()),
            tab: ParameterTab::Tab6,
            section: lvn_section,
            order: 6,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("lvn_show"),
        });
        params.push(ParameterDef {
            key: "lvn_show_labels".into(),
            label: "Show Labels".into(),
            description: "Show labels at LVN lines".into(),
            kind: ParameterKind::Boolean,
            default: ParameterValue::Boolean(false),
            tab: ParameterTab::Tab6,
            section: lvn_section,
            order: 7,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("lvn_show"),
        });

        // Detection section
        let det_section = Some(ParameterSection {
            label: "Detection",
            order: 2,
        });
        params.push(ParameterDef {
            key: "node_min_prominence".into(),
            label: "Min Prominence".into(),
            description: "Minimum prominence to qualify as a node"
                .into(),
            kind: ParameterKind::Float {
                min: 0.0,
                max: 1.0,
                step: 0.05,
            },
            default: ParameterValue::Float(0.0),
            tab: ParameterTab::Tab6,
            section: det_section,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
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
            tab: ParameterTab::Tab7,
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
            tab: ParameterTab::Tab7,
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
            tab: ParameterTab::Tab7,
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
            tab: ParameterTab::Tab7,
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
            tab: ParameterTab::Tab7,
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
            tab: ParameterTab::Tab7,
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
            tab: ParameterTab::Tab7,
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
            tab: ParameterTab::Tab7,
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
            tab: ParameterTab::Tab7,
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
            tab: ParameterTab::Tab7,
            section: vwap_bands,
            order: 4,
            format: DisplayFormat::Auto,
            visible_when: Visibility::WhenTrue("vwap_show_bands"),
        });

        params
    }

    fn parse_vbp_type(s: &str) -> VbpType {
        match s {
            "Bid/Ask Volume" => VbpType::BidAskVolume,
            "Delta" => VbpType::Delta,
            "Delta & Total Volume" => VbpType::DeltaAndTotalVolume,
            "Delta Percentage" => VbpType::DeltaPercentage,
            _ => VbpType::Volume,
        }
    }

    fn parse_period(s: &str) -> VbpPeriod {
        match s {
            "Length" => VbpPeriod::Length,
            "Custom" => VbpPeriod::Custom,
            _ => VbpPeriod::Auto,
        }
    }

    fn parse_length_unit(s: &str) -> VbpLengthUnit {
        match s {
            "Minutes" => VbpLengthUnit::Minutes,
            "Contracts" => VbpLengthUnit::Contracts,
            _ => VbpLengthUnit::Days,
        }
    }

    fn parse_side(s: &str) -> ProfileSide {
        match s {
            "Right" => ProfileSide::Right,
            _ => ProfileSide::Left,
        }
    }

    fn parse_extend(s: &str) -> ExtendDirection {
        match s {
            "Left" => ExtendDirection::Left,
            "Right" => ExtendDirection::Right,
            "Both" => ExtendDirection::Both,
            _ => ExtendDirection::None,
        }
    }

    fn parse_node_method(s: &str) -> NodeDetectionMethod {
        match s {
            "Relative" => NodeDetectionMethod::Relative,
            "Std Dev" => NodeDetectionMethod::StdDev,
            _ => NodeDetectionMethod::Percentile,
        }
    }

    /// Resolve which candle range to use based on period settings.
    fn resolve_candle_range<'a>(
        &self,
        candles: &'a [data::Candle],
        input: &StudyInput<'_>,
    ) -> &'a [data::Candle] {
        let period = Self::parse_period(
            self.config.get_choice("period", "Auto"),
        );

        match period {
            VbpPeriod::Auto => {
                if let Some((start, end)) = input.visible_range {
                    Self::slice_by_time(candles, start, end)
                } else {
                    candles
                }
            }
            VbpPeriod::Length => {
                if candles.is_empty() {
                    return candles;
                }
                let unit = Self::parse_length_unit(
                    self.config
                        .get_choice("length_unit", "Days"),
                );
                let value =
                    self.config.get_int("length_value", 5) as u64;
                let latest_ts = candles
                    .last()
                    .map(|c| c.time.to_millis())
                    .unwrap_or(0);

                match unit {
                    VbpLengthUnit::Days => {
                        let ms = value * 86_400_000;
                        let start = latest_ts.saturating_sub(ms);
                        Self::slice_by_time(
                            candles, start, latest_ts,
                        )
                    }
                    VbpLengthUnit::Minutes => {
                        let ms = value * 60_000;
                        let start = latest_ts.saturating_sub(ms);
                        Self::slice_by_time(
                            candles, start, latest_ts,
                        )
                    }
                    VbpLengthUnit::Contracts => {
                        let n = value as usize;
                        let start =
                            candles.len().saturating_sub(n);
                        &candles[start..]
                    }
                }
            }
            VbpPeriod::Custom => {
                let start =
                    self.config.get_int("custom_start", 0) as u64;
                let end =
                    self.config.get_int("custom_end", 0) as u64;
                if start == 0 && end == 0 {
                    candles
                } else {
                    Self::slice_by_time(candles, start, end)
                }
            }
        }
    }

    /// Binary-search slice of candles by timestamp range.
    fn slice_by_time(
        candles: &[data::Candle],
        start: u64,
        end: u64,
    ) -> &[data::Candle] {
        let start_idx = candles.partition_point(|c| {
            c.time.to_millis() < start
        });
        let end_idx =
            candles.partition_point(|c| c.time.to_millis() <= end);
        &candles[start_idx..end_idx]
    }

    /// Filter trades to the resolved candle time range.
    fn filter_trades<'a>(
        trades: &'a [data::Trade],
        candles: &[data::Candle],
    ) -> &'a [data::Trade] {
        if candles.is_empty() || trades.is_empty() {
            return &[];
        }
        let start = candles
            .first()
            .map(|c| c.time.to_millis())
            .unwrap_or(0);
        let end = candles
            .last()
            .map(|c| c.time.to_millis())
            .unwrap_or(0);

        let start_idx = trades
            .partition_point(|t| t.time.to_millis() < start);
        let end_idx =
            trades.partition_point(|t| t.time.to_millis() <= end);
        &trades[start_idx..end_idx]
    }

    /// Build POC config from current parameter values.
    fn build_poc_config(&self) -> VbpPocConfig {
        VbpPocConfig {
            show_poc: self.config.get_bool("poc_show", true),
            poc_color: self
                .config
                .get_color("poc_color", DEFAULT_POC_COLOR),
            poc_line_width: self
                .config
                .get_float("poc_line_width", 1.5)
                as f32,
            poc_line_style: self
                .config
                .get_line_style(
                    "poc_line_style",
                    LineStyleValue::Solid,
                ),
            poc_extend: Self::parse_extend(
                self.config.get_choice("poc_extend", "None"),
            ),
            show_poc_label: self
                .config
                .get_bool("poc_show_label", false),
            show_developing_poc: self
                .config
                .get_bool("poc_show_developing", false),
            developing_poc_color: self.config.get_color(
                "poc_dev_color",
                DEFAULT_DEV_POC_COLOR,
            ),
            developing_poc_line_width: self
                .config
                .get_float("poc_dev_line_width", 1.0)
                as f32,
            developing_poc_line_style: self
                .config
                .get_line_style(
                    "poc_dev_line_style",
                    LineStyleValue::Dashed,
                ),
        }
    }

    /// Build Value Area config from current parameter values.
    fn build_va_config(&self) -> VbpValueAreaConfig {
        VbpValueAreaConfig {
            show_value_area: self
                .config
                .get_bool("va_show", true),
            value_area_pct: self
                .config
                .get_float("value_area_pct", 0.7)
                as f32,
            show_va_highlight: self
                .config
                .get_bool("va_show_highlight", true),
            vah_color: self
                .config
                .get_color("va_vah_color", DEFAULT_VAH_COLOR),
            vah_line_width: self
                .config
                .get_float("va_vah_line_width", 1.0)
                as f32,
            vah_line_style: self
                .config
                .get_line_style(
                    "va_vah_line_style",
                    LineStyleValue::Dashed,
                ),
            val_color: self
                .config
                .get_color("va_val_color", DEFAULT_VAL_COLOR),
            val_line_width: self
                .config
                .get_float("va_val_line_width", 1.0)
                as f32,
            val_line_style: self
                .config
                .get_line_style(
                    "va_val_line_style",
                    LineStyleValue::Dashed,
                ),
            show_va_fill: self
                .config
                .get_bool("va_show_fill", false),
            va_fill_color: self
                .config
                .get_color("va_fill_color", DEFAULT_VA_FILL_COLOR),
            va_fill_opacity: self
                .config
                .get_float("va_fill_opacity", 0.15)
                as f32,
            va_extend: Self::parse_extend(
                self.config.get_choice("va_extend", "None"),
            ),
            show_va_labels: self
                .config
                .get_bool("va_show_labels", false),
        }
    }

    /// Build Node config from current parameter values.
    fn build_node_config(&self) -> VbpNodeConfig {
        VbpNodeConfig {
            show_hvn: self.config.get_bool("hvn_show", false),
            show_lvn: self.config.get_bool("lvn_show", false),
            hvn_method: Self::parse_node_method(
                self.config.get_choice("hvn_method", "Percentile"),
            ),
            hvn_threshold: self
                .config
                .get_float("hvn_threshold", 0.85)
                as f32,
            lvn_method: Self::parse_node_method(
                self.config.get_choice("lvn_method", "Percentile"),
            ),
            lvn_threshold: self
                .config
                .get_float("lvn_threshold", 0.15)
                as f32,
            min_prominence: self
                .config
                .get_float("node_min_prominence", 0.0)
                as f32,
            hvn_color: self
                .config
                .get_color("hvn_color", DEFAULT_HVN_COLOR),
            hvn_line_style: self
                .config
                .get_line_style(
                    "hvn_line_style",
                    LineStyleValue::Dotted,
                ),
            hvn_line_width: self
                .config
                .get_float("hvn_line_width", 1.0)
                as f32,
            hvn_extend: Self::parse_extend(
                self.config.get_choice("hvn_extend", "None"),
            ),
            lvn_color: self
                .config
                .get_color("lvn_color", DEFAULT_LVN_COLOR),
            lvn_line_style: self
                .config
                .get_line_style(
                    "lvn_line_style",
                    LineStyleValue::Dotted,
                ),
            lvn_line_width: self
                .config
                .get_float("lvn_line_width", 1.0)
                as f32,
            lvn_extend: Self::parse_extend(
                self.config.get_choice("lvn_extend", "None"),
            ),
            show_hvn_labels: self
                .config
                .get_bool("hvn_show_labels", false),
            show_lvn_labels: self
                .config
                .get_bool("lvn_show_labels", false),
        }
    }

    /// Build VWAP config from current parameter values.
    fn build_vwap_config(&self) -> VbpVwapConfig {
        VbpVwapConfig {
            show_vwap: self.config.get_bool("vwap_show", false),
            vwap_color: self
                .config
                .get_color("vwap_color", DEFAULT_VWAP_COLOR),
            vwap_line_width: self
                .config
                .get_float("vwap_line_width", 1.5)
                as f32,
            vwap_line_style: self
                .config
                .get_line_style(
                    "vwap_line_style",
                    LineStyleValue::Solid,
                ),
            show_vwap_label: self
                .config
                .get_bool("vwap_show_label", false),
            show_bands: self
                .config
                .get_bool("vwap_show_bands", false),
            band_multiplier: self
                .config
                .get_float("vwap_band_multiplier", 1.0)
                as f32,
            band_color: self.config.get_color(
                "vwap_band_color",
                DEFAULT_VWAP_BAND_COLOR,
            ),
            band_line_style: self
                .config
                .get_line_style(
                    "vwap_band_line_style",
                    LineStyleValue::Dashed,
                ),
            band_line_width: self
                .config
                .get_float("vwap_band_line_width", 1.0)
                as f32,
        }
    }

    /// Compute developing POC: walk candles chronologically,
    /// tracking running volume per price level and emitting the
    /// running POC price at each candle.
    fn compute_developing_poc(
        candle_slice: &[data::Candle],
        tick_size: data::Price,
        group_quantum: i64,
    ) -> Vec<(u64, i64)> {
        use std::collections::HashMap;

        let step = group_quantum.max(tick_size.units()).max(1);
        let cap = candle_slice
            .iter()
            .map(|c| {
                let lo =
                    c.low.round_to_tick(tick_size).units()
                        / step;
                let hi =
                    (c.high.round_to_tick(tick_size).units()
                        + step
                        - 1)
                        / step;
                (hi - lo + 1) as usize
            })
            .max()
            .unwrap_or(64);
        let mut volume_map: HashMap<i64, f64> =
            HashMap::with_capacity(cap * 2);
        let mut poc_price = 0i64;
        let mut poc_vol = 0.0f64;
        let mut result =
            Vec::with_capacity(candle_slice.len());

        for c in candle_slice {
            let low = (c.low.round_to_tick(tick_size).units()
                / step)
                * step;
            let high = ((c.high.round_to_tick(tick_size).units()
                + step
                - 1)
                / step)
                * step;
            let vol = c.volume() as f64;
            let n_levels = if high >= low {
                ((high - low) / step + 1) as f64
            } else {
                1.0
            };
            let vol_per = vol / n_levels;

            let mut p = low;
            while p <= high {
                let entry =
                    volume_map.entry(p).or_insert(0.0);
                *entry += vol_per;
                if *entry > poc_vol {
                    poc_vol = *entry;
                    poc_price = p;
                }
                p += step;
            }

            result.push((c.time.to_millis(), poc_price));
        }

        result
    }

    /// Compute anchored VWAP over the candle slice.
    fn compute_vwap(
        candle_slice: &[data::Candle],
        show_bands: bool,
        band_mult: f32,
    ) -> (TimeSeries, TimeSeries, TimeSeries) {
        let mut cum_tp_vol: f64 = 0.0;
        let mut cum_vol: f64 = 0.0;
        let mut cum_tp2_vol: f64 = 0.0;

        let n = candle_slice.len();
        let mut vwap_pts = Vec::with_capacity(n);
        let mut upper_pts = Vec::with_capacity(n);
        let mut lower_pts = Vec::with_capacity(n);

        for c in candle_slice {
            let tp = (c.high.to_f32() + c.low.to_f32()
                + c.close.to_f32()) as f64
                / 3.0;
            let vol = c.volume() as f64;
            let ts = c.time.to_millis();

            cum_tp_vol += tp * vol;
            cum_vol += vol;
            cum_tp2_vol += tp * tp * vol;

            if cum_vol > 0.0 {
                let vwap = cum_tp_vol / cum_vol;
                vwap_pts.push((ts, vwap as f32));

                if show_bands {
                    let variance =
                        (cum_tp2_vol / cum_vol) - (vwap * vwap);
                    let std_dev = if variance > 0.0 {
                        variance.sqrt()
                    } else {
                        0.0
                    };
                    let mult = band_mult as f64;
                    upper_pts.push((
                        ts,
                        (vwap + std_dev * mult) as f32,
                    ));
                    lower_pts.push((
                        ts,
                        (vwap - std_dev * mult) as f32,
                    ));
                }
            } else {
                vwap_pts.push((ts, tp as f32));
                if show_bands {
                    upper_pts.push((ts, tp as f32));
                    lower_pts.push((ts, tp as f32));
                }
            }
        }

        (vwap_pts, upper_pts, lower_pts)
    }
}

impl Default for VbpStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for VbpStudy {
    fn id(&self) -> &str {
        "vbp"
    }

    fn name(&self) -> &str {
        "Volume by Price"
    }

    fn category(&self) -> StudyCategory {
        StudyCategory::OrderFlow
    }

    fn placement(&self) -> StudyPlacement {
        StudyPlacement::Background
    }

    fn parameters(&self) -> &[ParameterDef] {
        &self.params
    }

    fn config(&self) -> &StudyConfig {
        &self.config
    }

    fn config_mut(&mut self) -> &mut StudyConfig {
        &mut self.config
    }

    fn set_parameter(
        &mut self,
        key: &str,
        value: ParameterValue,
    ) -> Result<(), StudyError> {
        let params = self.parameters();
        let def = params
            .iter()
            .find(|p| p.key == key)
            .ok_or_else(|| StudyError::InvalidParameter {
                key: key.to_string(),
                reason: "unknown parameter".to_string(),
            })?;
        def.validate(&value).map_err(|reason| {
            StudyError::InvalidParameter {
                key: key.to_string(),
                reason,
            }
        })?;
        self.config.set(key, value);
        // Invalidate fingerprint so next compute() runs fully
        self.last_input_fingerprint = (0, 0, 0, 0);
        self.last_stable_range = None;
        Ok(())
    }

    fn tab_labels(
        &self,
    ) -> Option<&[(&'static str, ParameterTab)]> {
        static LABELS: &[(&str, ParameterTab)] = &[
            ("Data", ParameterTab::Parameters),
            ("Style", ParameterTab::Style),
            ("POC", ParameterTab::Tab4),
            ("Value Area", ParameterTab::Tab5),
            ("Peak & Valley", ParameterTab::Tab6),
            ("VWAP", ParameterTab::Tab7),
        ];
        Some(LABELS)
    }

    fn compute(
        &mut self,
        input: &StudyInput,
    ) -> Result<(), StudyError> {
        if input.candles.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        // Stable-range check for Auto period: skip recompute
        // if visible range hasn't moved >25% of previous span.
        let period = Self::parse_period(
            self.config.get_choice("period", "Auto"),
        );
        if matches!(period, VbpPeriod::Auto) {
            if let Some((start, end)) = input.visible_range {
                if let Some((prev_start, prev_end)) =
                    self.last_stable_range
                {
                    let prev_span =
                        prev_end.saturating_sub(prev_start);
                    let threshold = prev_span / 4;
                    let start_shift = (start as i64
                        - prev_start as i64)
                        .unsigned_abs();
                    let end_shift = (end as i64
                        - prev_end as i64)
                        .unsigned_abs();
                    if start_shift < threshold
                        && end_shift < threshold
                        && !matches!(
                            self.output,
                            StudyOutput::Empty
                        )
                    {
                        return Ok(());
                    }
                }
                self.last_stable_range = Some((start, end));
            }
        }

        let candle_slice =
            self.resolve_candle_range(input.candles, input);

        if candle_slice.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        // Filter trades once for both fingerprint and profile.
        let filtered_trades = input
            .trades
            .map(|t| Self::filter_trades(t, candle_slice));
        let trade_count =
            filtered_trades.map(|t| t.len()).unwrap_or(0);

        // Build a fingerprint of the actual input data.
        let first_ts = candle_slice
            .first()
            .map(|c| c.time.to_millis())
            .unwrap_or(0);
        let last_ts = candle_slice
            .last()
            .map(|c| c.time.to_millis())
            .unwrap_or(0);
        let fingerprint =
            (candle_slice.len(), first_ts, last_ts, trade_count);

        if fingerprint == self.last_input_fingerprint
            && !matches!(self.output, StudyOutput::Empty)
        {
            return Ok(());
        }
        self.last_input_fingerprint = fingerprint;

        // Tick grouping
        let tick_units = input.tick_size.units().max(1);
        let is_automatic = self
            .config
            .get_choice("auto_grouping", "Automatic")
            != "Manual";

        let group_quantum = if is_automatic {
            tick_units
        } else {
            let manual =
                self.config.get_int("manual_ticks", 1).max(1);
            tick_units * manual
        };

        // Build profile: prefer trades if available
        let levels = match filtered_trades {
            Some(filtered) if !filtered.is_empty() => {
                profile_core::build_profile_from_trades(
                    filtered,
                    input.tick_size,
                    group_quantum,
                )
            }
            _ => profile_core::build_profile_from_candles(
                candle_slice,
                input.tick_size,
                group_quantum,
            ),
        };

        let grouping_mode = if is_automatic {
            let factor = self
                .config
                .get_int("auto_group_factor", 1)
                .max(1);
            VbpGroupingMode::Automatic { factor }
        } else {
            VbpGroupingMode::Manual
        };

        if levels.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        // Read config values
        let vbp_type = Self::parse_vbp_type(
            self.config.get_choice("vbp_type", "Volume"),
        );
        let side = Self::parse_side(
            self.config.get_choice("alignment", "Left"),
        );
        let width_pct =
            self.config.get_float("width_pct", 0.25) as f32;
        let opacity =
            self.config.get_float("opacity", 0.7) as f32;
        let volume_color = self
            .config
            .get_color("volume_color", DEFAULT_VOLUME_COLOR);
        let bid_color =
            self.config.get_color("bid_color", DEFAULT_BID_COLOR);
        let ask_color =
            self.config.get_color("ask_color", DEFAULT_ASK_COLOR);

        // Build nested configs
        let poc_config = self.build_poc_config();
        let va_config = self.build_va_config();
        let node_config = self.build_node_config();
        let vwap_config = self.build_vwap_config();

        // Compute POC and Value Area
        let poc = profile_core::find_poc_index(&levels);
        let value_area = if va_config.show_value_area {
            poc.and_then(|idx| {
                profile_core::calculate_value_area(
                    &levels,
                    idx,
                    va_config.value_area_pct as f64,
                )
            })
        } else {
            None
        };

        // Compute time range from candle slice
        let time_range = {
            let start = candle_slice
                .first()
                .map(|c| c.time.to_millis())
                .unwrap_or(0);
            let end = candle_slice
                .last()
                .map(|c| c.time.to_millis())
                .unwrap_or(0);
            Some((start, end))
        };

        // Developing POC
        let developing_poc_points =
            if poc_config.show_developing_poc {
                Self::compute_developing_poc(
                    candle_slice,
                    input.tick_size,
                    group_quantum,
                )
            } else {
                Vec::new()
            };

        // HVN/LVN detection
        let (hvn_nodes, lvn_nodes) =
            if node_config.show_hvn || node_config.show_lvn {
                profile_core::detect_volume_nodes(
                    &levels,
                    node_config.hvn_method,
                    node_config.hvn_threshold,
                    node_config.lvn_method,
                    node_config.lvn_threshold,
                    node_config.min_prominence,
                )
            } else {
                (Vec::new(), Vec::new())
            };

        // Anchored VWAP
        let (vwap_points, vwap_upper_points, vwap_lower_points) =
            if vwap_config.show_vwap {
                Self::compute_vwap(
                    candle_slice,
                    vwap_config.show_bands,
                    vwap_config.band_multiplier,
                )
            } else {
                (Vec::new(), Vec::new(), Vec::new())
            };

        self.output = StudyOutput::Vbp(VbpData {
            vbp_type,
            side,
            levels,
            quantum: group_quantum,
            poc: if poc_config.show_poc { poc } else { None },
            value_area,
            time_range,
            volume_color,
            bid_color,
            ask_color,
            width_pct,
            opacity,
            poc_config,
            va_config,
            node_config,
            vwap_config,
            developing_poc_points,
            hvn_nodes,
            lvn_nodes,
            vwap_points,
            vwap_upper_points,
            vwap_lower_points,
            grouping_mode,
            resolved_cache: std::sync::Mutex::new(None),
        });

        Ok(())
    }

    fn output(&self) -> &StudyOutput {
        &self.output
    }

    fn reset(&mut self) {
        self.output = StudyOutput::Empty;
        self.last_input_fingerprint = (0, 0, 0, 0);
        self.last_stable_range = None;
    }

    fn clone_study(&self) -> Box<dyn Study> {
        Box::new(Self {
            config: self.config.clone(),
            output: self.output.clone(),
            params: self.params.clone(),
            last_input_fingerprint: (0, 0, 0, 0),
            last_stable_range: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::{
        Candle, ChartBasis, Price, Timeframe, Timestamp, Volume,
    };

    fn make_candle(
        time: u64,
        open: f32,
        high: f32,
        low: f32,
        close: f32,
        buy_vol: f64,
        sell_vol: f64,
    ) -> Candle {
        Candle::new(
            Timestamp::from_millis(time),
            Price::from_f32(open),
            Price::from_f32(high),
            Price::from_f32(low),
            Price::from_f32(close),
            Volume(buy_vol),
            Volume(sell_vol),
        )
        .expect("test: valid candle")
    }

    fn make_input(candles: &[Candle]) -> StudyInput<'_> {
        StudyInput {
            candles,
            trades: None,
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(1.0),
            visible_range: None,
        }
    }

    #[test]
    fn test_vbp_compute_default() {
        let mut study = VbpStudy::new();
        let candles = vec![
            make_candle(
                1000, 100.0, 102.0, 99.0, 101.0, 100.0, 50.0,
            ),
            make_candle(
                2000, 101.0, 103.0, 100.0, 102.0, 80.0, 60.0,
            ),
        ];

        let input = make_input(&candles);
        study.compute(&input).unwrap();

        match &study.output {
            StudyOutput::Vbp(data) => {
                assert!(!data.levels.is_empty());
                assert!(data.poc.is_some());
                assert_eq!(data.vbp_type, VbpType::Volume);
                assert_eq!(data.side, ProfileSide::Left);
            }
            _ => panic!("Expected Vbp output"),
        }
    }

    #[test]
    fn test_vbp_empty_candles() {
        let mut study = VbpStudy::new();
        let candles: Vec<Candle> = vec![];
        let input = make_input(&candles);
        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_vbp_type_parsing() {
        assert_eq!(
            VbpStudy::parse_vbp_type("Volume"),
            VbpType::Volume
        );
        assert_eq!(
            VbpStudy::parse_vbp_type("Bid/Ask Volume"),
            VbpType::BidAskVolume
        );
        assert_eq!(
            VbpStudy::parse_vbp_type("Delta"),
            VbpType::Delta
        );
        assert_eq!(
            VbpStudy::parse_vbp_type("Delta & Total Volume"),
            VbpType::DeltaAndTotalVolume
        );
        assert_eq!(
            VbpStudy::parse_vbp_type("Delta Percentage"),
            VbpType::DeltaPercentage
        );
    }

    #[test]
    fn test_vbp_period_length() {
        let mut study = VbpStudy::new();
        study
            .set_parameter(
                "period",
                ParameterValue::Choice("Length".to_string()),
            )
            .unwrap();
        study
            .set_parameter(
                "length_unit",
                ParameterValue::Choice("Days".to_string()),
            )
            .unwrap();
        study
            .set_parameter(
                "length_value",
                ParameterValue::Integer(2),
            )
            .unwrap();

        let day_ms = 86_400_000u64;
        let candles = vec![
            make_candle(
                day_ms, 100.0, 102.0, 99.0, 101.0, 50.0, 50.0,
            ),
            make_candle(
                day_ms * 2,
                101.0,
                103.0,
                100.0,
                102.0,
                50.0,
                50.0,
            ),
            make_candle(
                day_ms * 3,
                102.0,
                104.0,
                101.0,
                103.0,
                50.0,
                50.0,
            ),
        ];

        let input = StudyInput {
            candles: &candles,
            trades: None,
            basis: ChartBasis::Time(Timeframe::D1),
            tick_size: Price::from_f32(1.0),
            visible_range: None,
        };

        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Vbp(_)));
    }

    #[test]
    fn test_vbp_value_area_disabled() {
        let mut study = VbpStudy::new();
        study
            .set_parameter(
                "va_show",
                ParameterValue::Boolean(false),
            )
            .unwrap();

        let candles = vec![make_candle(
            1000, 100.0, 102.0, 99.0, 101.0, 100.0, 50.0,
        )];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        match &study.output {
            StudyOutput::Vbp(data) => {
                assert!(data.value_area.is_none());
            }
            _ => panic!("Expected Vbp output"),
        }
    }

    #[test]
    fn test_vbp_new_tab_params() {
        let study = VbpStudy::new();
        let params = study.parameters();

        // Check we have params on all 4 new tabs
        let has_tab4 = params
            .iter()
            .any(|p| p.tab == ParameterTab::Tab4);
        let has_tab5 = params
            .iter()
            .any(|p| p.tab == ParameterTab::Tab5);
        let has_tab6 = params
            .iter()
            .any(|p| p.tab == ParameterTab::Tab6);
        let has_tab7 = params
            .iter()
            .any(|p| p.tab == ParameterTab::Tab7);

        assert!(has_tab4, "Missing POC tab params");
        assert!(has_tab5, "Missing Value Area tab params");
        assert!(has_tab6, "Missing Peak & Valley tab params");
        assert!(has_tab7, "Missing VWAP tab params");
    }

    #[test]
    fn test_vbp_developing_poc() {
        let mut study = VbpStudy::new();
        study
            .set_parameter(
                "poc_show_developing",
                ParameterValue::Boolean(true),
            )
            .unwrap();

        let candles = vec![
            make_candle(
                1000, 100.0, 102.0, 99.0, 101.0, 100.0, 50.0,
            ),
            make_candle(
                2000, 101.0, 103.0, 100.0, 102.0, 80.0, 60.0,
            ),
            make_candle(
                3000, 102.0, 104.0, 101.0, 103.0, 120.0, 80.0,
            ),
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        match &study.output {
            StudyOutput::Vbp(data) => {
                assert_eq!(
                    data.developing_poc_points.len(),
                    3,
                    "developing POC should have one point per candle"
                );
                // Each point should have a valid timestamp
                for (ts, price) in &data.developing_poc_points {
                    assert!(*ts > 0);
                    assert!(*price > 0);
                }
            }
            _ => panic!("Expected Vbp output"),
        }
    }

    #[test]
    fn test_vbp_vwap_computation() {
        let mut study = VbpStudy::new();
        study
            .set_parameter(
                "vwap_show",
                ParameterValue::Boolean(true),
            )
            .unwrap();
        study
            .set_parameter(
                "vwap_show_bands",
                ParameterValue::Boolean(true),
            )
            .unwrap();

        let candles = vec![
            make_candle(
                1000, 100.0, 102.0, 98.0, 100.0, 50.0, 50.0,
            ),
            make_candle(
                2000, 100.0, 104.0, 99.0, 103.0, 80.0, 40.0,
            ),
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        match &study.output {
            StudyOutput::Vbp(data) => {
                assert_eq!(data.vwap_points.len(), 2);
                assert_eq!(data.vwap_upper_points.len(), 2);
                assert_eq!(data.vwap_lower_points.len(), 2);
                // VWAP should be between candle low and high
                for (_, price) in &data.vwap_points {
                    assert!(*price > 90.0 && *price < 110.0);
                }
                // Upper >= VWAP >= Lower
                for i in 0..2 {
                    assert!(
                        data.vwap_upper_points[i].1
                            >= data.vwap_points[i].1
                    );
                    assert!(
                        data.vwap_lower_points[i].1
                            <= data.vwap_points[i].1
                    );
                }
            }
            _ => panic!("Expected Vbp output"),
        }
    }

    #[test]
    fn test_vbp_hvn_lvn_integration() {
        let mut study = VbpStudy::new();
        study
            .set_parameter(
                "hvn_show",
                ParameterValue::Boolean(true),
            )
            .unwrap();
        study
            .set_parameter(
                "lvn_show",
                ParameterValue::Boolean(true),
            )
            .unwrap();
        study
            .set_parameter(
                "hvn_method",
                ParameterValue::Choice("Relative".to_string()),
            )
            .unwrap();
        study
            .set_parameter(
                "hvn_threshold",
                ParameterValue::Float(0.5),
            )
            .unwrap();
        study
            .set_parameter(
                "lvn_method",
                ParameterValue::Choice("Relative".to_string()),
            )
            .unwrap();
        study
            .set_parameter(
                "lvn_threshold",
                ParameterValue::Float(0.2),
            )
            .unwrap();

        // Create candles that produce a profile with clear peaks
        // and valleys (need enough levels)
        let candles = vec![
            make_candle(
                1000, 100.0, 110.0, 90.0, 105.0, 200.0, 100.0,
            ),
            make_candle(
                2000, 105.0, 115.0, 95.0, 110.0, 50.0, 30.0,
            ),
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        match &study.output {
            StudyOutput::Vbp(data) => {
                // With these candles we should get some levels
                assert!(!data.levels.is_empty());
                // Nodes computed (may or may not find any)
                assert!(data.node_config.show_hvn);
                assert!(data.node_config.show_lvn);
            }
            _ => panic!("Expected Vbp output"),
        }
    }

    #[test]
    fn test_vbp_fingerprint_invalidation() {
        let mut study = VbpStudy::new();
        let candles = vec![make_candle(
            1000, 100.0, 102.0, 99.0, 101.0, 100.0, 50.0,
        )];
        let input = make_input(&candles);

        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Vbp(_)));

        // Change a parameter — should invalidate fingerprint
        study
            .set_parameter(
                "poc_show",
                ParameterValue::Boolean(false),
            )
            .unwrap();
        assert_eq!(
            study.last_input_fingerprint,
            (0, 0, 0, 0)
        );

        // Recompute should work
        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Vbp(_)));
    }

    #[test]
    fn test_vbp_clone_with_new_fields() {
        let mut study = VbpStudy::new();
        study
            .set_parameter(
                "poc_show_developing",
                ParameterValue::Boolean(true),
            )
            .unwrap();
        study
            .set_parameter(
                "vwap_show",
                ParameterValue::Boolean(true),
            )
            .unwrap();
        study
            .set_parameter(
                "hvn_show",
                ParameterValue::Boolean(true),
            )
            .unwrap();

        let candles = vec![
            make_candle(
                1000, 100.0, 102.0, 99.0, 101.0, 100.0, 50.0,
            ),
            make_candle(
                2000, 101.0, 103.0, 100.0, 102.0, 80.0, 60.0,
            ),
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let cloned = study.clone_study();
        match cloned.output() {
            StudyOutput::Vbp(data) => {
                assert!(!data.levels.is_empty());
                assert!(!data.developing_poc_points.is_empty());
                assert!(!data.vwap_points.is_empty());
            }
            _ => panic!("Expected Vbp output from clone"),
        }
    }
}
