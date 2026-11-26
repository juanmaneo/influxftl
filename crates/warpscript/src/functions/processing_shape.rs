// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The Processing functions of shapes: `loadShape` reads an SVG document into a `PSHAPE`, and
//! `shape` draws one, at its own size or scaled to a given one.

use super::Entry;
use super::args::fail_with_cause;
use super::processing::{Params, give_back, parse_params};
use crate::error::{Class, Error, Result};
use crate::processing::shape::{Placement, Shape};
use crate::processing::svg;
use crate::stack::Stack;
use crate::value::Value;
use std::rc::Rc;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("Pshape", Entry::Stack(pshape)),
    ("PloadShape", Entry::Stack(ploadshape)),
];

/// `PloadShape`: read the SVG document on top of the stack into a shape.
fn ploadshape(stack: &mut Stack) -> Result<()> {
    let text = stack.pop()?.to_string();
    let shape = svg::parse(&text).map_err(|err| {
        fail_with_cause("PloadShape", "caught an exception while loading SVG.", err)
    })?;
    stack.push(Value::Shape(Rc::new(shape)));
    Ok(())
}

/// `Pshape(shape)`, `Pshape(shape, x, y)` or `Pshape(shape, x, y, width, height)`: draw the shape
/// at its origin, and at its own size or scaled to the one given.
fn pshape(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1, 3, 5])?;
    let shape = shape_arg(&params)?;
    let placement = match params.len() {
        1 => Placement {
            x: 0.0,
            y: 0.0,
            size: None,
        },
        3 => {
            let [x, y] = params.floats_window::<2>(1)?;
            Placement { x, y, size: None }
        }
        _ => {
            let [x, y, width, height] = params.floats_window::<4>(1)?;
            Placement {
                x,
                y,
                size: Some((width, height)),
            }
        }
    };
    params.graphics.borrow_mut().draw_shape(&shape, placement)?;
    give_back(stack, params.graphics);
    Ok(())
}

/// The shape given first above the graphics.
fn shape_arg(params: &Params) -> Result<Rc<Shape>> {
    params.first().and_then(|value| match value {
        Value::Shape(shape) => Ok(Rc::clone(shape)),
        other => Err(Error::class_cast(other.kind(), Class::Shape)),
    })
}
