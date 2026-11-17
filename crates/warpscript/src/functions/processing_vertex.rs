// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The Processing functions that build a shape vertex by vertex: `beginShape` and `endShape`,
//! `vertex`, the contours, and the Bézier, quadratic and curve vertices.
//!
//! A vertex outside a shape is ignored, as Processing ignores it. A Bézier or curve vertex is
//! refused when the shape is not a `POLYGON`, since Processing only takes those in a polygon.

use super::Entry;
use super::processing::{Params, give_back, mode_at, parse_params};
use crate::error::Result;
use crate::error::processing::ProcessingError;
use crate::processing::curve::steps;
use crate::processing::recording::{Kind, Recording};
use crate::stack::Stack;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("PbeginShape", Entry::Stack(pbeginshape)),
    ("PendShape", Entry::Stack(pendshape)),
    ("Pvertex", Entry::Stack(pvertex)),
    ("PbeginContour", Entry::Stack(pbegincontour)),
    ("PendContour", Entry::Stack(pendcontour)),
    ("PbezierVertex", Entry::Stack(pbeziervertex)),
    ("PquadraticVertex", Entry::Stack(pquadraticvertex)),
    ("PcurveVertex", Entry::Stack(pcurvevertex)),
];

/// The kinds of shape `beginShape` takes.
const SHAPE_KINDS: &[(&str, Kind)] = &[
    ("POLYGON", Kind::Polygon),
    ("POINTS", Kind::Points),
    ("LINES", Kind::Lines),
    ("TRIANGLES", Kind::Triangles),
    ("TRIANGLE_STRIP", Kind::TriangleStrip),
    ("TRIANGLE_FAN", Kind::TriangleFan),
    ("QUADS", Kind::Quads),
    ("QUAD_STRIP", Kind::QuadStrip),
];

/// The modes `endShape` takes: whether the shape is closed.
const END_MODES: &[(&str, bool)] = &[("OPEN", false), ("CLOSE", true)];

/// Warp 10's message for a Bézier or quadratic vertex outside a polygon.
const BEZIER_NEEDS_POLYGON: &str =
    "beginShape() or beginShape(POLYGON) must be used before bezierVertex() or quadraticVertex()";

/// Warp 10's message for a curve vertex outside a polygon.
const CURVE_NEEDS_POLYGON: &str =
    "You must use beginShape() or beginShape(POLYGON) before curveVertex()";

/// Run `act` on the shape being recorded. Without a shape begun, there is nothing to record.
fn with_shape(params: &Params, act: impl FnOnce(&mut Recording) -> Result<()>) -> Result<()> {
    match params.graphics.borrow_mut().recording.as_mut() {
        Some(recording) => act(recording),
        None => Ok(()),
    }
}

/// Run `act` on the polygon being recorded. Any other shape, or none, is refused with `message`.
fn with_polygon(
    params: &Params,
    message: &'static str,
    act: impl FnOnce(&mut Recording) -> Result<()>,
) -> Result<()> {
    match params.graphics.borrow_mut().recording.as_mut() {
        Some(recording) if recording.kind() == Kind::Polygon => act(recording),
        _ => Err(ProcessingError::ShapeState { message }.into()),
    }
}

/// `beginShape()` or `beginShape(kind)`: the vertices after it make a shape of that kind.
fn pbeginshape(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[0, 1])?;
    let kind = if params.is_empty() {
        Kind::Polygon
    } else {
        mode_at(
            &params,
            0,
            "PbeginShape",
            ": invalid mode, should be 'POLYGON', 'POINTS', 'LINES', 'TRIANGLES', 'TRIANGLE_STRIP', 'TRIANGLE_FAN', 'QUADS' or 'QUAD_STRIP'.",
            SHAPE_KINDS,
        )?
    };
    params.graphics.borrow_mut().recording = Some(Recording::begin(kind));
    give_back(stack, params.graphics);
    Ok(())
}

/// `endShape()` or `endShape(OPEN | CLOSE)`: the shape begun is drawn. A shape is open unless
/// `CLOSE` is given, as Processing's is.
fn pendshape(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[0, 1])?;
    let close = if params.is_empty() {
        false
    } else {
        mode_at(
            &params,
            0,
            "PendShape",
            ": invalid mode, should be 'OPEN' or 'CLOSE'.",
            END_MODES,
        )?
    };
    let recorded = params.graphics.borrow_mut().recording.take();
    if let Some(recording) = recorded {
        let outline = recording.finish(close);
        params.graphics.borrow_mut().draw(&outline)?;
    }
    give_back(stack, params.graphics);
    Ok(())
}

/// `vertex(x, y)`, or with more values the z and texture coordinates, of which only `x` and `y`
/// are drawn.
fn pvertex(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[2, 3, 4, 5])?;
    let values = params.float_list()?;
    let [x, y, ..] = values[..] else {
        return Err(params.arity());
    };
    with_shape(&params, |recording| {
        recording.vertex([x, y]);
        Ok(())
    })?;
    give_back(stack, params.graphics);
    Ok(())
}

/// `beginContour()`: the vertices after it are a hole in the polygon. It only applies to a
/// polygon.
fn pbegincontour(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[0])?;
    with_shape(&params, |recording| {
        if recording.kind() == Kind::Polygon {
            recording.begin_contour();
        }
        Ok(())
    })?;
    give_back(stack, params.graphics);
    Ok(())
}

/// `endContour()`: the hole begun is closed.
fn pendcontour(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[0])?;
    with_shape(&params, |recording| {
        recording.end_contour();
        Ok(())
    })?;
    give_back(stack, params.graphics);
    Ok(())
}

/// `bezierVertex(x2, y2, x3, y3, x4, y4)`, or with the z coordinates of a 3D shape, a cubic
/// Bézier from the last vertex through the two control points to the end point.
// The 3D form takes the same points as the 2D one, with their z coordinates skipped, so its arm
// has the body of the 2D arm.
#[allow(clippy::match_same_arms)]
fn pbeziervertex(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[6, 9])?;
    let detail = steps(params.graphics.borrow().style.bezier_detail);
    let values = params.float_list()?;
    let (controls, end) = match values[..] {
        [x2, y2, x3, y3, x4, y4] => ([[x2, y2], [x3, y3]], [x4, y4]),
        [x2, y2, _, x3, y3, _, x4, y4, _] => ([[x2, y2], [x3, y3]], [x4, y4]),
        _ => return Err(params.arity()),
    };
    with_polygon(&params, BEZIER_NEEDS_POLYGON, |recording| {
        recording.bezier_vertex(controls, end, detail)
    })?;
    give_back(stack, params.graphics);
    Ok(())
}

/// `quadraticVertex(cx, cy, x3, y3)`, or with the z coordinates of a 3D shape, a quadratic
/// Bézier from the last vertex through the control point to the end point.
// As for `bezierVertex`, the 3D form drops the z coordinates of the 2D one.
#[allow(clippy::match_same_arms)]
fn pquadraticvertex(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[4, 6])?;
    let detail = steps(params.graphics.borrow().style.bezier_detail);
    let values = params.float_list()?;
    let (control, end) = match values[..] {
        [cx, cy, x, y] => ([cx, cy], [x, y]),
        [cx, cy, _, x, y, _] => ([cx, cy], [x, y]),
        _ => return Err(params.arity()),
    };
    with_polygon(&params, BEZIER_NEEDS_POLYGON, |recording| {
        recording.quadratic_vertex(control, end, detail)
    })?;
    give_back(stack, params.graphics);
    Ok(())
}

/// `curveVertex(x, y)`, or with a z coordinate, a control point of a Catmull-Rom curve.
fn pcurvevertex(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[2, 3])?;
    let (detail, tightness) = {
        let graphics = params.graphics.borrow();
        (
            steps(graphics.style.curve_detail),
            graphics.style.curve_tightness,
        )
    };
    let values = params.float_list()?;
    let [x, y, ..] = values[..] else {
        return Err(params.arity());
    };
    with_polygon(&params, CURVE_NEEDS_POLYGON, |recording| {
        recording.curve_vertex([x, y], detail, tightness)
    })?;
    give_back(stack, params.graphics);
    Ok(())
}
