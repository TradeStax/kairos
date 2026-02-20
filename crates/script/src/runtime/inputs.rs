//! Input system: two-phase execution for parameter declaration and reading.
//!
//! During the declaration pass, `input.*()` calls collect metadata.
//! During compute passes, they return current values from StudyConfig.

use crate::error::ScriptError;
use crate::manifest::{InputDeclaration, parse_category, slugify};
use data::SerializableColor;
use rquickjs::Ctx;
use std::cell::RefCell;
use std::rc::Rc;
use study::config::{ParameterKind, ParameterValue};
use study::traits::StudyCategory;

/// State collected during the declaration pass.
#[derive(Debug, Default)]
pub struct DeclarationState {
    pub name: Option<String>,
    pub overlay: bool,
    pub category: StudyCategory,
    pub inputs: Vec<InputDeclaration>,
}

/// Install declaration-pass runtime globals.
///
/// Returns a shared reference to the DeclarationState that gets populated
/// as the script executes indicator() and input.*() calls.
pub fn install_declaration_pass(
    ctx: &Ctx<'_>,
) -> Result<Rc<RefCell<DeclarationState>>, ScriptError> {
    let state = Rc::new(RefCell::new(DeclarationState::default()));
    let globals = ctx.globals();

    // indicator(name, { overlay, category })
    {
        let state_ref = state.clone();
        let indicator_fn = rquickjs::Function::new(
            ctx.clone(),
            move |name: String, opts: rquickjs::function::Opt<rquickjs::Object<'_>>| {
                let mut s = state_ref.borrow_mut();
                s.name = Some(name);
                if let Some(opts) = opts.0 {
                    if let Ok(overlay) = opts.get::<_, bool>("overlay") {
                        s.overlay = overlay;
                    }
                    if let Ok(cat) = opts.get::<_, String>("category") {
                        s.category = parse_category(&cat);
                    }
                }
            },
        )?;
        globals.set("indicator", indicator_fn)?;
    }

    // input object with methods
    let input_obj = rquickjs::Object::new(ctx.clone())?;

    // input.int(label, default, { min, max })
    {
        let state_ref = state.clone();
        let int_fn = rquickjs::Function::new(
            ctx.clone(),
            move |label: String,
                  default: i64,
                  opts: rquickjs::function::Opt<rquickjs::Object<'_>>|
                  -> i64 {
                let min = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, i64>("min").ok())
                    .unwrap_or(i64::MIN);
                let max = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, i64>("max").ok())
                    .unwrap_or(i64::MAX);

                let key = slugify(&label);
                state_ref.borrow_mut().inputs.push(InputDeclaration {
                    key,
                    label: label.clone(),
                    description: String::new(),
                    kind: ParameterKind::Integer { min, max },
                    default: ParameterValue::Integer(default),
                });
                default
            },
        )?;
        input_obj.set("int", int_fn)?;
    }

    // input.float(label, default, { min, max, step })
    {
        let state_ref = state.clone();
        let float_fn = rquickjs::Function::new(
            ctx.clone(),
            move |label: String,
                  default: f64,
                  opts: rquickjs::function::Opt<rquickjs::Object<'_>>|
                  -> f64 {
                let min = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, f64>("min").ok())
                    .unwrap_or(f64::MIN);
                let max = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, f64>("max").ok())
                    .unwrap_or(f64::MAX);
                let step = opts
                    .0
                    .as_ref()
                    .and_then(|o| o.get::<_, f64>("step").ok())
                    .unwrap_or(0.1);

                let key = slugify(&label);
                state_ref.borrow_mut().inputs.push(InputDeclaration {
                    key,
                    label: label.clone(),
                    description: String::new(),
                    kind: ParameterKind::Float { min, max, step },
                    default: ParameterValue::Float(default),
                });
                default
            },
        )?;
        input_obj.set("float", float_fn)?;
    }

    // input.bool(label, default)
    {
        let state_ref = state.clone();
        let bool_fn = rquickjs::Function::new(
            ctx.clone(),
            move |label: String, default: bool| -> bool {
                let key = slugify(&label);
                state_ref.borrow_mut().inputs.push(InputDeclaration {
                    key,
                    label: label.clone(),
                    description: String::new(),
                    kind: ParameterKind::Boolean,
                    default: ParameterValue::Boolean(default),
                });
                default
            },
        )?;
        input_obj.set("bool", bool_fn)?;
    }

    // input.color(label, defaultHex)
    {
        let state_ref = state.clone();
        let color_fn = rquickjs::Function::new(
            ctx.clone(),
            move |label: String, default_hex: String| -> String {
                let color = parse_hex_color(&default_hex);
                let key = slugify(&label);
                state_ref.borrow_mut().inputs.push(InputDeclaration {
                    key,
                    label: label.clone(),
                    description: String::new(),
                    kind: ParameterKind::Color,
                    default: ParameterValue::Color(color),
                });
                default_hex
            },
        )?;
        input_obj.set("color", color_fn)?;
    }

    // input.source(label, defaultSeries)
    // During declaration pass, we just record the parameter and return an empty array.
    {
        let state_ref = state.clone();
        let source_fn = rquickjs::Function::new(
            ctx.clone(),
            move |label: String, _default: Vec<f64>| -> Vec<f64> {
                let key = slugify(&label);
                state_ref.borrow_mut().inputs.push(InputDeclaration {
                    key,
                    label: label.clone(),
                    description: String::new(),
                    kind: ParameterKind::Choice {
                        options: &[
                            "Close", "Open", "High", "Low", "HL2", "HLC3",
                            "OHLC4",
                        ],
                    },
                    default: ParameterValue::Choice("close".to_string()),
                });

                // Return empty array (no data during declaration pass)
                vec![]
            },
        )?;
        input_obj.set("source", source_fn)?;
    }

    // input.choice(label, default, { options })
    {
        let state_ref = state.clone();
        let choice_fn = rquickjs::Function::new(
            ctx.clone(),
            move |label: String,
                  default: String,
                  _opts: rquickjs::function::Opt<rquickjs::Object<'_>>|
                  -> String {
                let key = slugify(&label);
                // We store as Choice with static options slice;
                // for scripts, use an empty options slice (dynamic)
                state_ref.borrow_mut().inputs.push(InputDeclaration {
                    key,
                    label: label.clone(),
                    description: String::new(),
                    kind: ParameterKind::Choice { options: &[] },
                    default: ParameterValue::Choice(default.clone()),
                });
                default
            },
        )?;
        input_obj.set("choice", choice_fn)?;
    }

    // input.lineStyle(label, default)
    {
        let state_ref = state.clone();
        let line_style_fn = rquickjs::Function::new(
            ctx.clone(),
            move |label: String, default: String| -> String {
                let style = parse_line_style(&default);
                let key = slugify(&label);
                state_ref.borrow_mut().inputs.push(InputDeclaration {
                    key,
                    label: label.clone(),
                    description: String::new(),
                    kind: ParameterKind::LineStyle,
                    default: ParameterValue::LineStyle(style),
                });
                default
            },
        )?;
        input_obj.set("lineStyle", line_style_fn)?;
    }

    globals.set("input", input_obj)?;

    Ok(state)
}

/// Install compute-pass input globals that read from a StudyConfig.
pub fn install_compute_inputs(
    ctx: &Ctx<'_>,
    config: &study::config::StudyConfig,
    inputs: &[InputDeclaration],
) -> Result<(), ScriptError> {
    let globals = ctx.globals();

    // indicator() is a no-op during compute pass
    globals.set(
        "indicator",
        rquickjs::Function::new(ctx.clone(), |_name: String| {}),
    )?;

    let input_obj = rquickjs::Object::new(ctx.clone())?;

    // For each declared input, create a function that returns the current value
    // We pre-compute the values and capture them in closures
    let values: Vec<(String, ParameterValue)> = inputs
        .iter()
        .map(|inp| {
            let val = config
                .get(&inp.key)
                .cloned()
                .unwrap_or_else(|| inp.default.clone());
            (inp.key.clone(), val)
        })
        .collect();

    // input.int returns value from config
    let vals = values.clone();
    let int_fn = rquickjs::Function::new(
        ctx.clone(),
        move |label: String,
              default: i64,
              _opts: rquickjs::function::Opt<rquickjs::Object<'_>>|
              -> i64 {
            let key = slugify(&label);
            vals.iter()
                .find(|(k, _)| k == &key)
                .and_then(|(_, v)| match v {
                    ParameterValue::Integer(i) => Some(*i),
                    _ => None,
                })
                .unwrap_or(default)
        },
    )?;
    input_obj.set("int", int_fn)?;

    let vals = values.clone();
    let float_fn = rquickjs::Function::new(
        ctx.clone(),
        move |label: String,
              default: f64,
              _opts: rquickjs::function::Opt<rquickjs::Object<'_>>|
              -> f64 {
            let key = slugify(&label);
            vals.iter()
                .find(|(k, _)| k == &key)
                .and_then(|(_, v)| match v {
                    ParameterValue::Float(f) => Some(*f),
                    _ => None,
                })
                .unwrap_or(default)
        },
    )?;
    input_obj.set("float", float_fn)?;

    let vals = values.clone();
    let bool_fn = rquickjs::Function::new(
        ctx.clone(),
        move |label: String, default: bool| -> bool {
            let key = slugify(&label);
            vals.iter()
                .find(|(k, _)| k == &key)
                .and_then(|(_, v)| match v {
                    ParameterValue::Boolean(b) => Some(*b),
                    _ => None,
                })
                .unwrap_or(default)
        },
    )?;
    input_obj.set("bool", bool_fn)?;

    let vals = values.clone();
    let color_fn = rquickjs::Function::new(
        ctx.clone(),
        move |label: String, default_hex: String| -> String {
            let key = slugify(&label);
            vals.iter()
                .find(|(k, _)| k == &key)
                .and_then(|(_, v)| match v {
                    ParameterValue::Color(c) => Some(color_to_hex(c)),
                    _ => None,
                })
                .unwrap_or(default_hex)
        },
    )?;
    input_obj.set("color", color_fn)?;

    // source returns the actual global array (as Vec<f64>, auto-converted)
    let source_vals = values.clone();
    let source_fn = rquickjs::Function::new(
        ctx.clone(),
        move |ctx: Ctx<'_>,
              label: String,
              _default: Vec<f64>|
              -> Vec<f64> {
            let key = slugify(&label);
            let source_name = source_vals
                .iter()
                .find(|(k, _)| k == &key)
                .and_then(|(_, v)| match v {
                    ParameterValue::Choice(s) => Some(s.to_lowercase()),
                    _ => None,
                })
                .unwrap_or_else(|| "close".to_string());

            // Return the global array for the selected source
            let globals = ctx.globals();
            globals
                .get::<_, Vec<f64>>(&*source_name)
                .unwrap_or_default()
        },
    )?;
    input_obj.set("source", source_fn)?;

    let vals = values.clone();
    let choice_fn = rquickjs::Function::new(
        ctx.clone(),
        move |label: String,
              default: String,
              _opts: rquickjs::function::Opt<rquickjs::Object<'_>>|
              -> String {
            let key = slugify(&label);
            vals.iter()
                .find(|(k, _)| k == &key)
                .and_then(|(_, v)| match v {
                    ParameterValue::Choice(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or(default)
        },
    )?;
    input_obj.set("choice", choice_fn)?;

    let vals = values;
    let line_style_fn = rquickjs::Function::new(
        ctx.clone(),
        move |label: String, default: String| -> String {
            let key = slugify(&label);
            vals.iter()
                .find(|(k, _)| k == &key)
                .and_then(|(_, v)| match v {
                    ParameterValue::LineStyle(s) => Some(format!("{s}")),
                    _ => None,
                })
                .unwrap_or(default)
        },
    )?;
    input_obj.set("lineStyle", line_style_fn)?;

    globals.set("input", input_obj)?;

    Ok(())
}

/// Parse a hex color string (#RGB, #RRGGBB, #RRGGBBAA) into SerializableColor.
pub fn parse_hex_color(hex: &str) -> SerializableColor {
    let hex = hex.trim_start_matches('#');
    let (r, g, b, a) = match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).unwrap_or(0);
            (r, g, b, 255u8)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            (r, g, b, 255u8)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
            (r, g, b, a)
        }
        _ => (128, 128, 128, 255),
    };
    SerializableColor {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: a as f32 / 255.0,
    }
}

/// Convert SerializableColor to hex string.
pub fn color_to_hex(c: &SerializableColor) -> String {
    let r = (c.r * 255.0) as u8;
    let g = (c.g * 255.0) as u8;
    let b = (c.b * 255.0) as u8;
    let a = (c.a * 255.0) as u8;
    if a == 255 {
        format!("#{r:02X}{g:02X}{b:02X}")
    } else {
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }
}

fn parse_line_style(s: &str) -> study::config::LineStyleValue {
    match s.to_lowercase().as_str() {
        "dashed" => study::config::LineStyleValue::Dashed,
        "dotted" => study::config::LineStyleValue::Dotted,
        _ => study::config::LineStyleValue::Solid,
    }
}
