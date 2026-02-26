//! Study parameter definitions, validation, and runtime storage.
//!
//! Each study declares its configurable parameters as [`ParameterDef`] values.
//! At runtime, the current parameter values live in a [`StudyConfig`] and are
//! validated against the definitions before being applied.
//!
//! - [`display`] — Formatting and conditional visibility rules for the UI.
//! - [`parameter`] — Parameter definitions, kinds, tabs, and sections.
//! - [`store`] — Runtime config storage with typed getters.
//! - [`value`] — Concrete parameter value types (int, float, color, etc.).

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

    #[test]
    fn test_float_param_rejects_nan() {
        let def = ParameterDef {
            key: "x".into(),
            label: "X".into(),
            description: "".into(),
            kind: ParameterKind::Float {
                min: 0.0,
                max: 1.0,
                step: 0.1,
            },
            default: ParameterValue::Float(0.5),
            tab: ParameterTab::Parameters,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        };
        assert!(def.validate(&ParameterValue::Float(f64::NAN)).is_err());
    }

    #[test]
    fn test_float_param_rejects_inf() {
        let def = ParameterDef {
            key: "x".into(),
            label: "X".into(),
            description: "".into(),
            kind: ParameterKind::Float {
                min: 0.0,
                max: 1.0,
                step: 0.1,
            },
            default: ParameterValue::Float(0.5),
            tab: ParameterTab::Parameters,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        };
        assert!(def.validate(&ParameterValue::Float(f64::INFINITY)).is_err());
        assert!(
            def.validate(&ParameterValue::Float(f64::NEG_INFINITY))
                .is_err()
        );
    }
}
