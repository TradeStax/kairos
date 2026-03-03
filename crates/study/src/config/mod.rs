//! Study parameter definitions, validation, and runtime storage.
//!
//! Each study declares its configurable parameters as [`ParameterDef`] values.
//! At runtime, the current parameter values live in a [`StudyConfig`] and are
//! validated against the definitions before being applied.
//!
//! - `display` — Formatting and conditional visibility rules for the UI.
//! - `parameter` — Parameter definitions, kinds, tabs, and sections.
//! - `store` — Runtime config storage with typed getters.
//! - `value` — Concrete parameter value types (int, float, color, etc.).

mod display;
mod parameter;
mod store;
mod value;

pub use display::{DisplayFormat, Visibility};
pub use parameter::{ParameterDef, ParameterKind, ParameterSection, ParameterTab};
pub use store::StudyConfig;
pub use value::{LineStyleValue, ParameterValue};

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bool_config(key: &str, val: bool) -> StudyConfig {
        let mut c = StudyConfig::new("test");
        c.set(key, ParameterValue::Boolean(val));
        c
    }

    fn make_choice_config(key: &str, val: &str) -> StudyConfig {
        let mut c = StudyConfig::new("test");
        c.set(key, ParameterValue::Choice(val.to_string()));
        c
    }

    fn make_float_def(min: f64, max: f64) -> ParameterDef {
        ParameterDef {
            key: "x".into(),
            label: "X".into(),
            description: "".into(),
            kind: ParameterKind::Float {
                min,
                max,
                step: 0.1,
            },
            default: ParameterValue::Float(0.5),
            tab: ParameterTab::Parameters,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        }
    }

    fn make_int_def(min: i64, max: i64) -> ParameterDef {
        ParameterDef {
            key: "n".into(),
            label: "N".into(),
            description: "".into(),
            kind: ParameterKind::Integer { min, max },
            default: ParameterValue::Integer(10),
            tab: ParameterTab::Parameters,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        }
    }

    fn make_choice_def() -> ParameterDef {
        ParameterDef {
            key: "src".into(),
            label: "Source".into(),
            description: "".into(),
            kind: ParameterKind::Choice {
                options: &["Close", "Open", "HL2"],
            },
            default: ParameterValue::Choice("Close".to_string()),
            tab: ParameterTab::Parameters,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        }
    }

    // ── Visibility: WhenFalse ────────────────────────────────────

    #[test]
    fn test_when_false_missing_key_defaults_visible() {
        let config = StudyConfig::new("test");
        assert!(
            Visibility::WhenFalse("missing").is_visible(&config),
            "WhenFalse with missing key should be visible \
             (default false -> !false = true)"
        );
    }

    #[test]
    fn test_when_false_present_false_is_visible() {
        let config = make_bool_config("flag", false);
        assert!(
            Visibility::WhenFalse("flag").is_visible(&config),
            "WhenFalse with key=false should be visible"
        );
    }

    #[test]
    fn test_when_false_present_true_is_hidden() {
        let config = make_bool_config("flag", true);
        assert!(
            !Visibility::WhenFalse("flag").is_visible(&config),
            "WhenFalse with key=true should be hidden"
        );
    }

    // ── Visibility: WhenTrue ─────────────────────────────────────

    #[test]
    fn test_when_true_missing_key_defaults_hidden() {
        let config = StudyConfig::new("test");
        assert!(
            !Visibility::WhenTrue("missing").is_visible(&config),
            "WhenTrue with missing key should be hidden (default false)"
        );
    }

    #[test]
    fn test_when_true_present_true_is_visible() {
        let config = make_bool_config("flag", true);
        assert!(Visibility::WhenTrue("flag").is_visible(&config));
    }

    #[test]
    fn test_when_true_present_false_is_hidden() {
        let config = make_bool_config("flag", false);
        assert!(!Visibility::WhenTrue("flag").is_visible(&config));
    }

    // ── Visibility: Always ───────────────────────────────────────

    #[test]
    fn test_always_visible() {
        let config = StudyConfig::new("test");
        assert!(Visibility::Always.is_visible(&config));
    }

    // ── Visibility: WhenChoice / WhenNotChoice ───────────────────

    #[test]
    fn test_when_choice_matches() {
        let config = make_choice_config("mode", "Linear");
        let vis = Visibility::WhenChoice {
            key: "mode",
            equals: "Linear",
        };
        assert!(vis.is_visible(&config));
    }

    #[test]
    fn test_when_choice_does_not_match() {
        let config = make_choice_config("mode", "Log");
        let vis = Visibility::WhenChoice {
            key: "mode",
            equals: "Linear",
        };
        assert!(!vis.is_visible(&config));
    }

    #[test]
    fn test_when_choice_missing_key() {
        let config = StudyConfig::new("test");
        let vis = Visibility::WhenChoice {
            key: "mode",
            equals: "Linear",
        };
        // Missing key defaults to "" which != "Linear"
        assert!(!vis.is_visible(&config));
    }

    #[test]
    fn test_when_not_choice_matches() {
        let config = make_choice_config("mode", "Log");
        let vis = Visibility::WhenNotChoice {
            key: "mode",
            not_equals: "Linear",
        };
        assert!(vis.is_visible(&config));
    }

    #[test]
    fn test_when_not_choice_does_not_match() {
        let config = make_choice_config("mode", "Linear");
        let vis = Visibility::WhenNotChoice {
            key: "mode",
            not_equals: "Linear",
        };
        assert!(!vis.is_visible(&config));
    }

    // ── Parameter validation: Float ──────────────────────────────

    #[test]
    fn test_float_param_rejects_nan() {
        let def = make_float_def(0.0, 1.0);
        assert!(def.validate(&ParameterValue::Float(f64::NAN)).is_err());
    }

    #[test]
    fn test_float_param_rejects_inf() {
        let def = make_float_def(0.0, 1.0);
        assert!(def.validate(&ParameterValue::Float(f64::INFINITY)).is_err());
        assert!(
            def.validate(&ParameterValue::Float(f64::NEG_INFINITY))
                .is_err()
        );
    }

    #[test]
    fn test_float_param_accepts_within_bounds() {
        let def = make_float_def(0.0, 1.0);
        assert!(def.validate(&ParameterValue::Float(0.5)).is_ok());
        assert!(def.validate(&ParameterValue::Float(0.0)).is_ok());
        assert!(def.validate(&ParameterValue::Float(1.0)).is_ok());
    }

    #[test]
    fn test_float_param_rejects_below_min() {
        let def = make_float_def(0.0, 1.0);
        assert!(def.validate(&ParameterValue::Float(-0.1)).is_err());
    }

    #[test]
    fn test_float_param_rejects_above_max() {
        let def = make_float_def(0.0, 1.0);
        assert!(def.validate(&ParameterValue::Float(1.1)).is_err());
    }

    // ── Parameter validation: Integer ────────────────────────────

    #[test]
    fn test_int_param_accepts_within_bounds() {
        let def = make_int_def(1, 100);
        assert!(def.validate(&ParameterValue::Integer(1)).is_ok());
        assert!(def.validate(&ParameterValue::Integer(50)).is_ok());
        assert!(def.validate(&ParameterValue::Integer(100)).is_ok());
    }

    #[test]
    fn test_int_param_rejects_below_min() {
        let def = make_int_def(1, 100);
        assert!(def.validate(&ParameterValue::Integer(0)).is_err());
    }

    #[test]
    fn test_int_param_rejects_above_max() {
        let def = make_int_def(1, 100);
        assert!(def.validate(&ParameterValue::Integer(101)).is_err());
    }

    // ── Parameter validation: Choice ─────────────────────────────

    #[test]
    fn test_choice_param_accepts_valid_option() {
        let def = make_choice_def();
        assert!(
            def.validate(&ParameterValue::Choice("Close".to_string()))
                .is_ok()
        );
        assert!(
            def.validate(&ParameterValue::Choice("HL2".to_string()))
                .is_ok()
        );
    }

    #[test]
    fn test_choice_param_rejects_invalid_option() {
        let def = make_choice_def();
        assert!(
            def.validate(&ParameterValue::Choice("VWAP".to_string()))
                .is_err()
        );
    }

    // ── Parameter validation: Type mismatch ──────────────────────

    #[test]
    fn test_type_mismatch_float_for_int() {
        let def = make_int_def(1, 100);
        assert!(def.validate(&ParameterValue::Float(5.0)).is_err());
    }

    #[test]
    fn test_type_mismatch_int_for_float() {
        let def = make_float_def(0.0, 1.0);
        assert!(def.validate(&ParameterValue::Integer(1)).is_err());
    }

    #[test]
    fn test_type_mismatch_bool_for_color() {
        let def = ParameterDef {
            key: "c".into(),
            label: "Color".into(),
            description: "".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(data::SerializableColor::new(1.0, 1.0, 1.0, 1.0)),
            tab: ParameterTab::Style,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        };
        assert!(def.validate(&ParameterValue::Boolean(true)).is_err());
    }

    // ── Parameter validation: Boolean and Color pass ─────────────

    #[test]
    fn test_bool_param_accepts_bool() {
        let def = ParameterDef {
            key: "b".into(),
            label: "Flag".into(),
            description: "".into(),
            kind: ParameterKind::Boolean,
            default: ParameterValue::Boolean(false),
            tab: ParameterTab::Display,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        };
        assert!(def.validate(&ParameterValue::Boolean(true)).is_ok());
        assert!(def.validate(&ParameterValue::Boolean(false)).is_ok());
    }

    #[test]
    fn test_color_param_accepts_color() {
        let white = data::SerializableColor::new(1.0, 1.0, 1.0, 1.0);
        let def = ParameterDef {
            key: "c".into(),
            label: "Color".into(),
            description: "".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(white),
            tab: ParameterTab::Style,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        };
        assert!(def.validate(&ParameterValue::Color(white)).is_ok());
    }

    // ── StudyConfig store typed getters ───────────────────────────

    #[test]
    fn test_study_config_get_int() {
        let mut c = StudyConfig::new("test");
        c.set("period", ParameterValue::Integer(14));
        assert_eq!(c.get_int("period", 0), 14);
        // Wrong type returns default
        assert_eq!(c.get_int("missing", 20), 20);
    }

    #[test]
    fn test_study_config_get_float() {
        let mut c = StudyConfig::new("test");
        c.set("mult", ParameterValue::Float(2.5));
        assert!((c.get_float("mult", 0.0) - 2.5).abs() < 1e-10);
        assert!((c.get_float("missing", 1.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_study_config_get_bool() {
        let mut c = StudyConfig::new("test");
        c.set("show", ParameterValue::Boolean(true));
        assert!(c.get_bool("show", false));
        assert!(!c.get_bool("missing", false));
    }

    #[test]
    fn test_study_config_get_choice() {
        let mut c = StudyConfig::new("test");
        c.set("src", ParameterValue::Choice("Open".to_string()));
        assert_eq!(c.get_choice("src", "Close"), "Open");
        assert_eq!(c.get_choice("missing", "Close"), "Close");
    }

    #[test]
    fn test_study_config_get_line_style() {
        let mut c = StudyConfig::new("test");
        c.set("style", ParameterValue::LineStyle(LineStyleValue::Dashed));
        assert_eq!(
            c.get_line_style("style", LineStyleValue::Solid),
            LineStyleValue::Dashed
        );
        assert_eq!(
            c.get_line_style("missing", LineStyleValue::Solid),
            LineStyleValue::Solid
        );
    }

    #[test]
    fn test_study_config_get_color() {
        let mut c = StudyConfig::new("test");
        let red = data::SerializableColor::new(1.0, 0.0, 0.0, 1.0);
        let white = data::SerializableColor::new(1.0, 1.0, 1.0, 1.0);
        c.set("color", ParameterValue::Color(red));
        assert_eq!(c.get_color("color", white), red);
    }

    #[test]
    fn test_study_config_wrong_type_returns_default() {
        let mut c = StudyConfig::new("test");
        c.set("period", ParameterValue::Float(14.0));
        // Asking for int but stored as float => returns default
        assert_eq!(c.get_int("period", 20), 20);
    }

    // ── StudyConfig validate_and_set ─────────────────────────────

    #[test]
    fn test_validate_and_set_valid() {
        let params = vec![make_int_def(1, 100)];
        let mut c = StudyConfig::new("test");
        let result = c.validate_and_set("n", ParameterValue::Integer(50), &params);
        assert!(result.is_ok());
        assert_eq!(c.get_int("n", 0), 50);
    }

    #[test]
    fn test_validate_and_set_out_of_range() {
        let params = vec![make_int_def(1, 100)];
        let mut c = StudyConfig::new("test");
        let result = c.validate_and_set("n", ParameterValue::Integer(200), &params);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_and_set_unknown_key() {
        let params = vec![make_int_def(1, 100)];
        let mut c = StudyConfig::new("test");
        let result = c.validate_and_set("unknown", ParameterValue::Integer(5), &params);
        assert!(result.is_err());
    }

    // ── ParameterKind type_name ──────────────────────────────────

    #[test]
    fn test_parameter_kind_type_names() {
        assert_eq!(
            ParameterKind::Integer { min: 0, max: 100 }.type_name(),
            "integer"
        );
        assert_eq!(
            ParameterKind::Float {
                min: 0.0,
                max: 1.0,
                step: 0.1
            }
            .type_name(),
            "float"
        );
        assert_eq!(ParameterKind::Color.type_name(), "color");
        assert_eq!(ParameterKind::Boolean.type_name(), "boolean");
        assert_eq!(ParameterKind::Choice { options: &[] }.type_name(), "choice");
        assert_eq!(ParameterKind::LineStyle.type_name(), "line style");
    }
}
