// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The Processing shape functions that draw one primitive: a point, a line, a triangle, a
//! quadrilateral, a rectangle, an ellipse, an arc, and the `box` and `sphere` of the 3D
//! renderer.
//!
//! A 3D context is drawn in the plane: the `z` coordinates are checked for presence as Warp 10
//! checks them, but not drawn (see `DIVERGENCES.md`). `box` and `sphere` draw nothing, as they
//! do in a 2D context in Processing.

use super::Entry;
use super::processing::{draw_and_return, give_back, mode_at, parse_params};
use crate::error::processing::ProcessingError;
use crate::error::{Error, Result};
use crate::processing::primitive::{self, ArcMode, place};
use crate::stack::Stack;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("Ppoint", Entry::Stack(ppoint)),
    ("Pline", Entry::Stack(pline)),
    ("Ptriangle", Entry::Stack(ptriangle)),
    ("Pquad", Entry::Stack(pquad)),
    ("Prect", Entry::Stack(prect)),
    ("Pellipse", Entry::Stack(pellipse)),
    ("Parc", Entry::Stack(parc)),
    ("Pbox", Entry::Stack(pbox)),
    ("Psphere", Entry::Stack(psphere)),
];

/// The keywords an arc takes as its mode.
const ARC_MODES: &[(&str, ArcMode)] = &[
    ("CHORD", ArcMode::Chord),
    ("PIE", ArcMode::Pie),
    ("OPEN", ArcMode::Open),
];

/// The failure of a shape given a value it does not take, with Warp 10's wording.
fn refused(function: &'static str, message: &'static str) -> Error {
    ProcessingError::Refused { function, message }.into()
}

/// `Ppoint(x, y)` or `Ppoint(x, y, z)`: a point of the stroke weight.
fn ppoint(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[2, 3])?;
    if params.graphics.borrow().three_d && params.len() != 3 {
        return Err(refused("Ppoint", " missing z-coordinate."));
    }
    let [x, y] = params.floats_window::<2>(0)?;
    draw_and_return(stack, params, &primitive::point(x, y)?)
}

/// `Pline(x1, y1, x2, y2)` or `Pline(x1, y1, z1, x2, y2, z2)`: a straight stroke.
fn pline(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[4, 6])?;
    if params.graphics.borrow().three_d && params.len() != 6 {
        return Err(refused("Pline", " missing z-coordinates."));
    }
    let [x1, y1, x2, y2] = params.floats_window::<4>(0)?;
    draw_and_return(stack, params, &primitive::line([x1, y1], [x2, y2])?)
}

/// `Ptriangle(x1, y1, x2, y2, x3, y3)`.
fn ptriangle(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[6])?;
    let [ax, ay, bx, by, cx, cy] = params.floats::<6>()?;
    draw_and_return(
        stack,
        params,
        &primitive::triangle([[ax, ay], [bx, by], [cx, cy]])?,
    )
}

/// `Pquad(x1, y1, …, x4, y4)`: the four corners, in order.
fn pquad(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[8])?;
    let [ax, ay, bx, by, cx, cy, dx, dy] = params.floats::<8>()?;
    draw_and_return(
        stack,
        params,
        &primitive::quad([[ax, ay], [bx, by], [cx, cy], [dx, dy]])?,
    )
}

/// `Prect`: a rectangle placed by the `rectMode` of the style, with one radius for all its
/// corners (five values), or one radius per corner (eight values).
fn prect(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[4, 5, 8])?;
    let mode = params.graphics.borrow().style.rect_mode;
    let values = params.float_list()?;
    let (corners, radii) = match values[..] {
        [a, b, c, d] => ([a, b, c, d], [0.0; 4]),
        [a, b, c, d, radius] => ([a, b, c, d], [radius; 4]),
        [a, b, c, d, tl, tr, br, bl] => ([a, b, c, d], [tl, tr, br, bl]),
        _ => return Err(params.arity()),
    };
    let [ox, oy, ex, ey] = corners;
    let (x, width) = place(mode, ox, ex)?;
    let (y, height) = place(mode, oy, ey)?;
    draw_and_return(stack, params, &primitive::rect(x, y, width, height, radii)?)
}

/// `Pellipse(a, b, c, d)`: an ellipse placed by the `ellipseMode` of the style.
fn pellipse(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[4])?;
    let mode = params.graphics.borrow().style.ellipse_mode;
    let [ox, oy, ex, ey] = params.floats::<4>()?;
    let (x, width) = place(mode, ox, ex)?;
    let (y, height) = place(mode, oy, ey)?;
    draw_and_return(stack, params, &primitive::ellipse(x, y, width, height)?)
}

/// `Parc(a, b, c, d, start, stop)` or `Parc(a, b, c, d, start, stop, mode)`: an arc of the
/// ellipse the `ellipseMode` places. The mode is `PIE` when it is not given.
fn parc(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[6, 7])?;
    let mode = params.graphics.borrow().style.ellipse_mode;
    let [ox, oy, ex, ey, start, stop] = params.floats_window::<6>(0)?;
    let arc_mode = if params.len() == 7 {
        mode_at(
            &params,
            6,
            "Parc",
            " invalid mode, expected 'CHORD', 'PIE' or 'OPEN'.",
            ARC_MODES,
        )?
    } else {
        ArcMode::Pie
    };
    let (x, width) = place(mode, ox, ex)?;
    let (y, height) = place(mode, oy, ey)?;
    let outline = primitive::arc([x, y, width, height], start, stop, arc_mode)?;
    draw_and_return(stack, params, &outline)
}

/// `Pbox(size)` or `Pbox(width, height, depth)`: a box of the 3D renderer. A 2D context draws
/// nothing, as Processing's does, and a 3D box is not drawn yet (see `DIVERGENCES.md`).
fn pbox(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1, 3])?;
    params.float_list()?;
    give_back(stack, params.graphics);
    Ok(())
}

/// `Psphere(radius)`: a sphere of the 3D renderer, drawn as [`pbox`] is: nothing yet.
fn psphere(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1])?;
    params.float_list()?;
    give_back(stack, params.graphics);
    Ok(())
}
