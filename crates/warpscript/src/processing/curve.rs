// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The curves of the Processing API: cubic Bézier curves (`bezier`, `bezierVertex`) and
//! Catmull-Rom splines (`curve`, `curveVertex`), as one coordinate at a time. Each function
//! works on one axis: the four control values `a`, `b`, `c`, `d` and the parameter `t`.
//!
//! Each result is checked with [`finite`]: a curve built from finite points can only go
//! non-finite through an overflow, which is refused rather than drawn.

// The four control values of a curve are named `a`, `b`, `c` and `d` after the Bézier and
// Catmull-Rom formulas, as the references name them.
#![allow(clippy::many_single_char_names)]

use super::finite;
use crate::error::Result;

/// The largest number of steps a curve is flattened in. Java2D flattens curves by itself, so
/// Processing's detail does not change a 2D curve; here it only sets the resolution, within this
/// bound, which keeps a huge detail from allocating a huge polygon.
pub const MAX_STEPS: i32 = 1000;

/// The number of steps for a detail the script set, kept between 1 and [`MAX_STEPS`].
pub fn steps(detail: i32) -> usize {
    detail.clamp(1, MAX_STEPS) as usize
}

/// A point of a Bézier curve at `t`, for one coordinate (`bezierPoint`).
pub fn bezier_point(a: f32, b: f32, c: f32, d: f32, t: f32) -> Result<f32> {
    let t1 = 1.0 - t;
    finite(a * t1 * t1 * t1 + 3.0 * b * t * t1 * t1 + 3.0 * c * t * t * t1 + d * t * t * t)
}

/// The slope of a Bézier curve at `t`, for one coordinate (`bezierTangent`).
pub fn bezier_tangent(a: f32, b: f32, c: f32, d: f32, t: f32) -> Result<f32> {
    finite(
        3.0 * t * t * (-a + 3.0 * b - 3.0 * c + d) + 6.0 * t * (a - 2.0 * b + c) + 3.0 * (-a + b),
    )
}

/// The coefficients of the cubic `C3·t³ + C2·t² + C1·t + C0` that a Catmull-Rom segment through
/// `a`, `b`, `c`, `d` becomes, for a tightness `s` (`curveTightness`). The segment runs from `b`
/// to `c`; `s = 0` is the uniform spline of Processing's default.
fn catmull_rom_coefficients(a: f32, b: f32, c: f32, d: f32, s: f32) -> Result<[f32; 4]> {
    let k = (1.0 - s) / 2.0;
    let c1 = k * (c - a);
    let c2 = -3.0 * b + 3.0 * c - 2.0 * c1 - k * (d - b);
    let c3 = 2.0 * b - 2.0 * c + c1 + k * (d - b);
    let coefficients = [b, c1, c2, c3];
    for value in coefficients {
        finite(value)?;
    }
    Ok(coefficients)
}

/// A point of a Catmull-Rom segment between `b` and `c` at `t`, for one coordinate
/// (`curvePoint`).
pub fn curve_point(a: f32, b: f32, c: f32, d: f32, t: f32, s: f32) -> Result<f32> {
    let [c0, c1, c2, c3] = catmull_rom_coefficients(a, b, c, d, s)?;
    finite(((c3 * t + c2) * t + c1) * t + c0)
}

/// The slope of a Catmull-Rom segment at `t`, for one coordinate (`curveTangent`).
pub fn curve_tangent(a: f32, b: f32, c: f32, d: f32, t: f32, s: f32) -> Result<f32> {
    let [_, c1, c2, c3] = catmull_rom_coefficients(a, b, c, d, s)?;
    finite((3.0 * c3 * t + 2.0 * c2) * t + c1)
}

/// The points that a cubic Bézier curve from `p0` to `p3` passes through, in `detail` steps:
/// the points at `t = 1/detail`, …, `t = 1`, the last one being `p3` itself, so that the
/// curve ends exactly on its anchor.
pub fn bezier_steps([p0, p1, p2, p3]: [[f32; 2]; 4], detail: usize) -> Result<Vec<[f32; 2]>> {
    let mut points = Vec::with_capacity(detail);
    for step in 1..detail {
        let t = step as f32 / detail as f32;
        points.push([
            bezier_point(p0[0], p1[0], p2[0], p3[0], t)?,
            bezier_point(p0[1], p1[1], p2[1], p3[1], t)?,
        ]);
    }
    points.push(p3);
    Ok(points)
}

/// The points of a Catmull-Rom segment between `p1` and `p2` (`p0` and `p3` shape it), in
/// `detail` steps. The last point is `p2` itself.
pub fn curve_steps(
    [p0, p1, p2, p3]: [[f32; 2]; 4],
    detail: usize,
    tightness: f32,
) -> Result<Vec<[f32; 2]>> {
    let mut points = Vec::with_capacity(detail);
    for step in 1..detail {
        let t = step as f32 / detail as f32;
        points.push([
            curve_point(p0[0], p1[0], p2[0], p3[0], t, tightness)?,
            curve_point(p0[1], p1[1], p2[1], p3[1], t, tightness)?,
        ]);
    }
    points.push(p2);
    Ok(points)
}

#[cfg(test)]
#[path = "curve_tests.rs"]
mod curve_tests;
