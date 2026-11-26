// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The Processing curve functions: the cubic Bézier curve (`Pbezier`) and the Catmull-Rom
//! spline (`Pcurve`), drawn as one segment, and the functions that evaluate a point or a tangent
//! on either curve for one axis (`PbezierPoint`, `PbezierTangent`, `PcurvePoint`,
//! `PcurveTangent`).

// The coordinates of a curve are named `a`, `b`, `c`, `d` and `t`, as Processing's `bezierPoint`
// and `curvePoint` name them.
#![allow(clippy::many_single_char_names)]

use super::Entry;
use super::processing::{Params, answer, draw_and_return, parse_params};
use crate::error::Result;
use crate::error::processing::ProcessingError;
use crate::processing::curve::{bezier_point, bezier_tangent, curve_point, curve_tangent, steps};
use crate::processing::primitive;
use crate::stack::Stack;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("Pbezier", Entry::Stack(pbezier)),
    ("Pcurve", Entry::Stack(pcurve)),
    ("PbezierPoint", Entry::Stack(pbezierpoint)),
    ("PbezierTangent", Entry::Stack(pbeziertangent)),
    ("PcurvePoint", Entry::Stack(pcurvepoint)),
    ("PcurveTangent", Entry::Stack(pcurvetangent)),
];

/// The four points of a curve given by its values. Eight values are a 2D curve. Twelve values
/// are a 3D curve, which is drawn flat on the plane of the context: its `z` coordinates are read
/// but not drawn (see `DIVERGENCES.md`). Twelve values on a 2D context are refused, as Warp 10
/// refuses them.
fn curve_points(params: &Params, function: &'static str) -> Result<[[f32; 2]; 4]> {
    let three_d = params.graphics.borrow().three_d;
    let values = params.float_list()?;
    match values[..] {
        [x1, y1, x2, y2, x3, y3, x4, y4] => Ok([[x1, y1], [x2, y2], [x3, y3], [x4, y4]]),
        [x1, y1, _, x2, y2, _, x3, y3, _, x4, y4, _] if three_d => {
            Ok([[x1, y1], [x2, y2], [x3, y3], [x4, y4]])
        }
        _ => Err(ProcessingError::Refused {
            function,
            message: " need a 3D container to draw a 3D Bezier curve.",
        }
        .into()),
    }
}

/// `Pbezier(x1, y1, x2, y2, x3, y3, x4, y4)`: a cubic Bézier from the first point to the last,
/// through the two others as control points.
fn pbezier(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[8, 12])?;
    let points = curve_points(&params, "Pbezier")?;
    let detail = steps(params.graphics.borrow().style.bezier_detail);
    let outline = primitive::bezier(points, detail)?;
    draw_and_return(stack, params, &outline)
}

/// `Pcurve(x1, y1, x2, y2, x3, y3, x4, y4)`: the Catmull-Rom segment from the second point to the
/// third, the first and the last shaping it.
fn pcurve(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[8, 12])?;
    let points = curve_points(&params, "Pcurve")?;
    let (detail, tightness) = {
        let graphics = params.graphics.borrow();
        (
            steps(graphics.style.curve_detail),
            graphics.style.curve_tightness,
        )
    };
    let outline = primitive::spline(points, detail, tightness)?;
    draw_and_return(stack, params, &outline)
}

/// The five values `a, b, c, d, t` of a point or tangent function, which the function reads
/// from the stack.
fn axis_values(stack: &mut Stack) -> Result<(Params, [f32; 5])> {
    let params = parse_params(stack, &[5])?;
    let values = params.floats::<5>()?;
    Ok((params, values))
}

/// `PbezierPoint(a, b, c, d, t)`: the coordinate of a cubic Bézier at `t`, for one axis.
fn pbezierpoint(stack: &mut Stack) -> Result<()> {
    let (params, [a, b, c, d, t]) = axis_values(stack)?;
    let value = bezier_point(a, b, c, d, t)?;
    answer(stack, params.graphics, f64::from(value));
    Ok(())
}

/// `PbezierTangent(a, b, c, d, t)`: the derivative of a cubic Bézier at `t`, for one axis.
fn pbeziertangent(stack: &mut Stack) -> Result<()> {
    let (params, [a, b, c, d, t]) = axis_values(stack)?;
    let value = bezier_tangent(a, b, c, d, t)?;
    answer(stack, params.graphics, f64::from(value));
    Ok(())
}

/// `PcurvePoint(a, b, c, d, t)`: the coordinate of a Catmull-Rom spline at `t`, for one axis,
/// with the tightness of the style.
fn pcurvepoint(stack: &mut Stack) -> Result<()> {
    let (params, [a, b, c, d, t]) = axis_values(stack)?;
    let tightness = params.graphics.borrow().style.curve_tightness;
    let value = curve_point(a, b, c, d, t, tightness)?;
    answer(stack, params.graphics, f64::from(value));
    Ok(())
}

/// `PcurveTangent(a, b, c, d, t)`: the derivative of a Catmull-Rom spline at `t`, for one axis,
/// with the tightness of the style.
fn pcurvetangent(stack: &mut Stack) -> Result<()> {
    let (params, [a, b, c, d, t]) = axis_values(stack)?;
    let tightness = params.graphics.borrow().style.curve_tightness;
    let value = curve_tangent(a, b, c, d, t, tightness)?;
    answer(stack, params.graphics, f64::from(value));
    Ok(())
}
