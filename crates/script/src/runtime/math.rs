//! Math utility functions exposed to JS.

use crate::error::ScriptError;
use rquickjs::{Ctx, Function, Object};

/// Install the `math` namespace on JS globals.
pub fn install_math(ctx: &Ctx<'_>) -> Result<(), ScriptError> {
    let globals = ctx.globals();
    let math = Object::new(ctx.clone())?;

    math.set("PI", std::f64::consts::PI)?;
    math.set("E", std::f64::consts::E)?;

    math.set(
        "sqrt",
        Function::new(ctx.clone(), |x: f64| -> f64 { x.sqrt() }),
    )?;
    math.set(
        "abs",
        Function::new(ctx.clone(), |x: f64| -> f64 { x.abs() }),
    )?;
    math.set(
        "pow",
        Function::new(ctx.clone(), |x: f64, y: f64| -> f64 { x.powf(y) }),
    )?;
    math.set(
        "log",
        Function::new(ctx.clone(), |x: f64| -> f64 { x.ln() }),
    )?;
    math.set(
        "log10",
        Function::new(ctx.clone(), |x: f64| -> f64 { x.log10() }),
    )?;
    math.set(
        "exp",
        Function::new(ctx.clone(), |x: f64| -> f64 { x.exp() }),
    )?;
    math.set(
        "min",
        Function::new(ctx.clone(), |a: f64, b: f64| -> f64 { a.min(b) }),
    )?;
    math.set(
        "max",
        Function::new(ctx.clone(), |a: f64, b: f64| -> f64 { a.max(b) }),
    )?;
    math.set(
        "round",
        Function::new(ctx.clone(), |x: f64| -> f64 { x.round() }),
    )?;
    math.set(
        "floor",
        Function::new(ctx.clone(), |x: f64| -> f64 { x.floor() }),
    )?;
    math.set(
        "ceil",
        Function::new(ctx.clone(), |x: f64| -> f64 { x.ceil() }),
    )?;
    math.set(
        "sign",
        Function::new(ctx.clone(), |x: f64| -> f64 { x.signum() }),
    )?;

    // avg(...values) - arithmetic mean of arguments
    math.set(
        "avg",
        Function::new(ctx.clone(), |values: Vec<f64>| -> f64 {
            if values.is_empty() {
                return f64::NAN;
            }
            let sum: f64 = values.iter().sum();
            sum / values.len() as f64
        }),
    )?;

    // sum(array) - sum of array elements
    math.set(
        "sum",
        Function::new(ctx.clone(), |values: Vec<f64>| -> f64 {
            values.iter().sum()
        }),
    )?;

    // stdev(array) - population standard deviation
    math.set(
        "stdev",
        Function::new(ctx.clone(), |values: Vec<f64>| -> f64 {
            if values.is_empty() {
                return f64::NAN;
            }
            let n = values.len() as f64;
            let mean = values.iter().sum::<f64>() / n;
            let variance =
                values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
            variance.sqrt()
        }),
    )?;

    // variance(array) - population variance
    math.set(
        "variance",
        Function::new(ctx.clone(), |values: Vec<f64>| -> f64 {
            if values.is_empty() {
                return f64::NAN;
            }
            let n = values.len() as f64;
            let mean = values.iter().sum::<f64>() / n;
            values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n
        }),
    )?;

    globals.set("math", math)?;
    Ok(())
}
