//! Drawing tools API stubs.
//!
//! Provides line.new(), box.new(), label.new() stubs for scripts.
//! Drawing tools integration is a future enhancement; currently these
//! are no-ops that return integer IDs.

use crate::error::ScriptError;
use rquickjs::{Ctx, Function, Object};
use std::cell::Cell;
use std::rc::Rc;

/// Install drawing tool stubs into the JS context.
pub fn install_drawing_stubs(ctx: &Ctx<'_>) -> Result<(), ScriptError> {
    let globals = ctx.globals();
    let next_id = Rc::new(Cell::new(0i32));

    // line.new(x1, y1, x2, y2, opts?) -> id
    let line_obj = Object::new(ctx.clone())?;
    {
        let id = next_id.clone();
        line_obj.set(
            "new",
            Function::new(
                ctx.clone(),
                move |_x1: f64, _y1: f64, _x2: f64, _y2: f64| -> i32 {
                    let cur = id.get();
                    id.set(cur + 1);
                    cur
                },
            ),
        )?;
    }
    globals.set("line", line_obj)?;

    // box.new(left, top, right, bottom, opts?) -> id
    let box_obj = Object::new(ctx.clone())?;
    {
        let id = next_id.clone();
        box_obj.set(
            "new",
            Function::new(
                ctx.clone(),
                move |_l: f64, _t: f64, _r: f64, _b: f64| -> i32 {
                    let cur = id.get();
                    id.set(cur + 1);
                    cur
                },
            ),
        )?;
    }
    globals.set("box", box_obj)?;

    // label.new(x, y, text, opts?) -> id
    let label_obj = Object::new(ctx.clone())?;
    {
        let id = next_id;
        label_obj.set(
            "new",
            Function::new(
                ctx.clone(),
                move |_x: f64, _y: f64, _text: String| -> i32 {
                    let cur = id.get();
                    id.set(cur + 1);
                    cur
                },
            ),
        )?;
    }
    globals.set("label", label_obj)?;

    Ok(())
}
