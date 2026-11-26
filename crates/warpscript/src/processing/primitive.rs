// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The outlines of the primitives of the Processing API, in the coordinates of the script:
//! points, lines, triangles, quadrilaterals, rectangles (with rounded corners), ellipses and
//! arcs. A curved outline is flattened into a polygon with as many sides as its size needs, so
//! that it looks round at the resolution of the device.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use super::curve::{bezier_steps, curve_steps};
use super::draw::Outline;
use super::path::{Ring, Subpath};
use super::{Mode, finite};
use crate::error::Result;

/// The mode of an arc: `PIE` (the default), `CHORD` or `OPEN`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcMode {
    /// `PIE`: the arc, and the two radii that join its ends to the centre.
    Pie,
    /// `CHORD`: the arc, and the straight line that closes it.
    Chord,
    /// `OPEN`: the arc alone. A fill closes it with the chord, a stroke does not.
    Open,
}

/// The start and the length of a shape on one axis, placed by `mode`: `origin` is the first
/// coordinate, and `extent` the size (for `CORNERS`, the second coordinate). `CENTER` puts
/// `origin` at the middle, and `RADIUS` puts it at the middle with `extent` as the half-size.
pub fn place(mode: Mode, origin: f32, extent: f32) -> Result<(f32, f32)> {
    Ok(match mode {
        Mode::Corner => (origin, extent),
        Mode::Corners => (origin, finite(extent - origin)?),
        Mode::Center => (finite(origin - extent / 2.0)?, extent),
        Mode::Radius => (finite(origin - extent)?, finite(2.0 * extent)?),
    })
}

/// A point, refused when a coordinate is not finite.
fn at(x: f32, y: f32) -> Result<[f32; 2]> {
    Ok([finite(x)?, finite(y)?])
}

/// The number of segments of an arc of `radius` that sweeps `sweep` radians: about two per
/// unit of its length in the script's coordinates, between 2 and 1440.
fn arc_segments(radius: f32, sweep: f32) -> Result<usize> {
    let length = finite(radius.abs() * sweep.abs() * 2.0)?;
    Ok((length.ceil() as usize).clamp(2, 1440))
}

/// The points of an elliptical arc from the angle `start` over `sweep` radians, both ends
/// included. The angles run clockwise on the screen, as Processing's do.
fn arc_points(centre: [f32; 2], radii: [f32; 2], start: f32, sweep: f32) -> Result<Vec<[f32; 2]>> {
    let [cx, cy] = centre;
    let [rx, ry] = radii;
    let segments = arc_segments(rx.max(ry), sweep)?;
    (0..=segments)
        .map(|i| {
            let angle = start + sweep * i as f32 / segments as f32;
            at(cx + rx * angle.cos(), cy + ry * angle.sin())
        })
        .collect()
}

/// `point(x, y)`: a single point, drawn with the stroke.
pub fn point(x: f32, y: f32) -> Result<Outline> {
    Ok(Outline {
        points: vec![at(x, y)?],
        ..Outline::default()
    })
}

/// `line(x1, y1, x2, y2)`: a straight stroke between two points.
pub fn line(from: [f32; 2], to: [f32; 2]) -> Result<Outline> {
    let points = vec![at(from[0], from[1])?, at(to[0], to[1])?];
    Ok(Outline {
        stroke: vec![Subpath {
            points,
            closed: false,
        }],
        ..Outline::default()
    })
}

/// `triangle(x1, y1, x2, y2, x3, y3)`.
pub fn triangle(corners: [[f32; 2]; 3]) -> Result<Outline> {
    let ring = corners
        .iter()
        .map(|p| at(p[0], p[1]))
        .collect::<Result<Ring>>()?;
    Ok(Outline::closed(ring))
}

/// `quad(x1, y1, …, x4, y4)`: the four corners in order.
pub fn quad(corners: [[f32; 2]; 4]) -> Result<Outline> {
    let ring = corners
        .iter()
        .map(|p| at(p[0], p[1]))
        .collect::<Result<Ring>>()?;
    Ok(Outline::closed(ring))
}

/// `rect` between two corners `(x, y)` and `(x + width, y + height)`, given in either order.
/// Its corners are rounded by `radii` (top-left, top-right, bottom-right, bottom-left), each
/// limited to half of the shorter side.
pub fn rect(x: f32, y: f32, width: f32, height: f32, radii: [f32; 4]) -> Result<Outline> {
    let (far_x, far_y) = (finite(x + width)?, finite(y + height)?);
    let (left, right) = (x.min(far_x), x.max(far_x));
    let (top, bottom) = (y.min(far_y), y.max(far_y));
    let limit = (right - left).min(bottom - top) / 2.0;
    let [tl, tr, br, bl] = radii.map(|radius| radius.clamp(0.0, limit));
    // Each corner is an arc of a quarter turn, clockwise on the screen from the side before it.
    let mut ring = Vec::new();
    ring.extend(arc_points([left + tl, top + tl], [tl, tl], PI, FRAC_PI_2)?);
    ring.extend(arc_points(
        [right - tr, top + tr],
        [tr, tr],
        3.0 * FRAC_PI_2,
        FRAC_PI_2,
    )?);
    ring.extend(arc_points(
        [right - br, bottom - br],
        [br, br],
        0.0,
        FRAC_PI_2,
    )?);
    ring.extend(arc_points(
        [left + bl, bottom - bl],
        [bl, bl],
        FRAC_PI_2,
        FRAC_PI_2,
    )?);
    Ok(Outline::closed(ring))
}

/// The centre and the half-sizes of the ellipse that fills the box of corner (`x`, `y`) and size
/// `width` by `height`.
fn inscribed(x: f32, y: f32, width: f32, height: f32) -> Result<([f32; 2], [f32; 2])> {
    let centre = [finite(x + width / 2.0)?, finite(y + height / 2.0)?];
    Ok((centre, [width.abs() / 2.0, height.abs() / 2.0]))
}

/// `ellipse` in the box of corner (`x`, `y`) and size `width` by `height`.
pub fn ellipse(x: f32, y: f32, width: f32, height: f32) -> Result<Outline> {
    let (centre, radii) = inscribed(x, y, width, height)?;
    Ok(Outline::closed(arc_points(centre, radii, 0.0, TAU)?))
}

/// `arc` of the ellipse in the box of corner (`x`, `y`) and size `width` by `height`, from the
/// angle `start` to `stop`, in the mode given.
pub fn arc(
    [x, y, width, height]: [f32; 4],
    start: f32,
    stop: f32,
    mode: ArcMode,
) -> Result<Outline> {
    let (centre, radii) = inscribed(x, y, width, height)?;
    let curve = arc_points(centre, radii, start, finite(stop - start)?)?;
    Ok(match mode {
        ArcMode::Pie => {
            let mut ring = vec![at(centre[0], centre[1])?];
            ring.extend(curve);
            Outline::closed(ring)
        }
        ArcMode::Chord => Outline::closed(curve),
        ArcMode::Open => Outline::open(curve),
    })
}

/// `bezier` from the first of four points to the last, through the other two as control points,
/// in `detail` steps.
pub fn bezier(points: [[f32; 2]; 4], detail: usize) -> Result<Outline> {
    let mut curve = vec![at(points[0][0], points[0][1])?];
    curve.extend(bezier_steps(points, detail)?);
    Ok(Outline::open(curve))
}

/// `curve`: the Catmull-Rom segment from the second of four points to the third, the first and
/// the last shaping it, in `detail` steps with the `tightness` of the style.
pub fn spline(points: [[f32; 2]; 4], detail: usize, tightness: f32) -> Result<Outline> {
    let mut curve = vec![at(points[1][0], points[1][1])?];
    curve.extend(curve_steps(points, detail, tightness)?);
    Ok(Outline::open(curve))
}

#[cfg(test)]
#[path = "primitive_tests.rs"]
mod primitive_tests;
