// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The outlines of drawn shapes, and the pixels they cover.
//!
//! A shape is a list of [`Subpath`]s in the coordinates of the script. Drawing moves them to the
//! device, then either fills them ([`cover`]: a pixel is covered when its centre lies inside the
//! rings, which fill with the non-zero rule) or strokes them ([`stroke_outline`]: the union of the
//! rectangles of the segments, the joins at the corners and the caps at the ends, each a
//! positively oriented ring so that the union is one non-zero fill).
//!
//! Anti-aliasing is not reproduced: a pixel is either covered or not (see `DIVERGENCES.md`).

use std::f32::consts::TAU;

use super::finite;
use super::{Cap, Join};
use crate::error::Result;

/// A closed polygon, filled with the non-zero rule.
pub type Ring = Vec<[f32; 2]>;

/// A sequence of points joined by segments, closed back to its first point when `closed`.
#[derive(Clone, Debug, PartialEq)]
pub struct Subpath {
    /// The points, in order.
    pub points: Vec<[f32; 2]>,
    /// Whether the last point is joined back to the first.
    pub closed: bool,
}

/// The miter limit of Java2D, which Processing strokes with: a corner whose miter would stick out
/// more than this many half-widths is cut as a bevel.
const MITER_LIMIT: f32 = 10.0;

/// The pixels of an image that a set of rings covers: a rectangle of rows and columns, with a
/// flag per pixel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coverage {
    x0: usize,
    y0: usize,
    columns: usize,
    covered: Vec<bool>,
}

impl Coverage {
    /// The coverage of nothing.
    fn none() -> Self {
        Coverage {
            x0: 0,
            y0: 0,
            columns: 0,
            covered: Vec::new(),
        }
    }

    /// Each covered pixel, as `(x, y)`, row by row.
    pub fn pixels(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        let columns = self.columns;
        self.covered
            .iter()
            .enumerate()
            .filter(|(_, covered)| **covered)
            .map(move |(index, _)| (self.x0 + index % columns, self.y0 + index / columns))
    }
}

/// A point, refused when a coordinate is not finite.
fn point(x: f32, y: f32) -> Result<[f32; 2]> {
    Ok([finite(x)?, finite(y)?])
}

/// The pixel index a coordinate falls on, kept within `low..=high`. The coordinate is finite.
fn index_within(coordinate: f32, low: usize, high: usize) -> usize {
    let index = coordinate.max(0.0) as usize;
    index.clamp(low, high)
}

/// The smallest and largest coordinates of the rings, `None` when there is no point.
fn extent(rings: &[Ring]) -> Option<([f32; 2], [f32; 2])> {
    let mut points = rings.iter().flatten();
    let first = *points.next()?;
    Some(points.fold((first, first), |(low, high), p| {
        (
            [low[0].min(p[0]), low[1].min(p[1])],
            [high[0].max(p[0]), high[1].max(p[1])],
        )
    }))
}

/// The pixels of a `width` by `height` image that `rings` cover. A pixel is covered when its
/// centre lies inside the rings, which fill with the non-zero rule. Only the rows and columns the
/// rings reach are visited.
///
/// Each row is scanned at the height of its pixel centres: the edges that cross that height are
/// sorted by their abscissa, and the spans between crossings with a non-zero winding are covered.
pub fn cover(rings: &[Ring], width: usize, height: usize) -> Result<Coverage> {
    let Some((low, high)) = extent(rings) else {
        return Ok(Coverage::none());
    };
    let x0 = index_within(low[0].floor(), 0, width);
    let x1 = index_within(high[0].ceil(), 0, width);
    let y0 = index_within(low[1].floor(), 0, height);
    let y1 = index_within(high[1].ceil(), 0, height);
    let columns = x1 - x0;
    let mut covered = vec![false; columns * (y1 - y0)];
    let mut crossings: Vec<(f32, i32)> = Vec::new();
    for row in y0..y1 {
        let centre = row as f32 + 0.5;
        crossings.clear();
        for ring in rings {
            for (i, &[px, py]) in ring.iter().enumerate() {
                let [qx, qy] = ring[(i + 1) % ring.len()];
                if (py <= centre) == (qy <= centre) {
                    continue;
                }
                let x = finite(px + (centre - py) * (qx - px) / (qy - py))?;
                crossings.push((x, if qy > py { 1 } else { -1 }));
            }
        }
        crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut winding = 0;
        for (i, &(from, step)) in crossings.iter().enumerate() {
            winding += step;
            let Some(&(to, _)) = crossings.get(i + 1) else {
                break;
            };
            if winding == 0 {
                continue;
            }
            // Column `c` is inside the span when `from <= c + 0.5 < to`.
            let first = index_within((from - 0.5).ceil(), x0, x1);
            let end = index_within((to - 0.5).ceil(), x0, x1);
            let start = (row - y0) * columns;
            for column in first..end {
                covered[start + column - x0] = true;
            }
        }
    }
    Ok(Coverage {
        x0,
        y0,
        columns,
        covered,
    })
}

/// The twice-signed area of a ring, in `f64` so that no coordinate can overflow it.
fn area(ring: &[[f32; 2]]) -> f64 {
    ring.iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|(a, b)| f64::from(a[0]) * f64::from(b[1]) - f64::from(b[0]) * f64::from(a[1]))
        .sum()
}

/// The ring with a positive area: a shape drawn as one of several rings is then added to the
/// others by the non-zero rule, rather than cancelled by an opposite winding.
pub fn positive(mut ring: Ring) -> Ring {
    if area(&ring) < 0.0 {
        ring.reverse();
    }
    ring
}

/// The unit vector from `from` to `to`, refused when the points are too close to have one.
fn direction(from: [f32; 2], to: [f32; 2]) -> Result<[f32; 2]> {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    let length = finite((dx * dx + dy * dy).sqrt())?;
    point(dx / length, dy / length)
}

/// A circle of `radius` around `centre`, as a polygon with enough sides to look round.
pub fn disc(centre: [f32; 2], radius: f32) -> Result<Ring> {
    let radius = finite(radius)?;
    let sides = (radius.abs() * TAU * 2.0).ceil().clamp(8.0, 1024.0) as usize;
    (0..sides)
        .map(|i| {
            let angle = TAU * i as f32 / sides as f32;
            point(
                centre[0] + radius * angle.cos(),
                centre[1] + radius * angle.sin(),
            )
        })
        .collect()
}

/// The rectangle of a segment from `a` to `b`, `half` wide on each side.
fn segment_ring(a: [f32; 2], b: [f32; 2], half: f32) -> Result<Ring> {
    let [ux, uy] = direction(a, b)?;
    let (nx, ny) = (-uy * half, ux * half);
    Ok(positive(vec![
        point(a[0] + nx, a[1] + ny)?,
        point(b[0] + nx, b[1] + ny)?,
        point(b[0] - nx, b[1] - ny)?,
        point(a[0] - nx, a[1] - ny)?,
    ]))
}

/// The ring of a join at `corner`, between the segment from `prev` and the one to `next`.
///
/// The join fills the outer side of the turn. A round join is a disc, a bevel a triangle, and a
/// miter the quadrilateral that reaches the intersection of the two outer edges, unless that
/// intersection is further than [`MITER_LIMIT`] half-widths away, when it is a bevel.
fn join_ring(
    prev: [f32; 2],
    corner: [f32; 2],
    next: [f32; 2],
    half: f32,
    join: Join,
) -> Result<Option<Ring>> {
    let [ax, ay] = direction(prev, corner)?;
    let [bx, by] = direction(corner, next)?;
    let cross = ax * by - ay * bx;
    let dot = ax * bx + ay * by;
    if cross.abs() < 1e-6 && dot > 0.0 {
        return Ok(None);
    }
    let (n0, n1) = ([-ay, ax], [-by, bx]);
    let side = if cross > 0.0 { -half } else { half };
    let outer0 = point(corner[0] + side * n0[0], corner[1] + side * n0[1])?;
    let outer1 = point(corner[0] + side * n1[0], corner[1] + side * n1[1])?;
    let bevel = || positive(vec![corner, outer0, outer1]);
    let ring = match join {
        Join::Round => disc(corner, half)?,
        Join::Bevel => bevel(),
        Join::Miter => {
            // The miter length over the half-width is `|n0 + n1| / (1 + n0·n1)`, which is
            // `1 / sin(θ/2)` for the angle θ between the segments.
            let denominator = 1.0 + (n0[0] * n1[0] + n0[1] * n1[1]);
            let (sx, sy) = (n0[0] + n1[0], n0[1] + n1[1]);
            let within_limit = (sx * sx + sy * sy).sqrt() <= MITER_LIMIT * denominator;
            if denominator > 1e-6 && within_limit {
                let tip = point(
                    corner[0] + side * sx / denominator,
                    corner[1] + side * sy / denominator,
                )?;
                positive(vec![corner, outer0, tip, outer1])
            } else {
                bevel()
            }
        }
    };
    Ok(Some(ring))
}

/// The cap at an open end `end`, whose path leaves it in the direction `outward`.
fn cap_ring(end: [f32; 2], outward: [f32; 2], half: f32, cap: Cap) -> Result<Option<Ring>> {
    Ok(match cap {
        Cap::Square => None,
        Cap::Round => Some(disc(end, half)?),
        Cap::Project => {
            let [ox, oy] = outward;
            let (nx, ny) = (-oy * half, ox * half);
            let (ex, ey) = (ox * half, oy * half);
            Some(positive(vec![
                point(end[0] + nx, end[1] + ny)?,
                point(end[0] + nx + ex, end[1] + ny + ey)?,
                point(end[0] - nx + ex, end[1] - ny + ey)?,
                point(end[0] - nx, end[1] - ny)?,
            ]))
        }
    })
}

/// Drop the points that repeat the one before them (and, when closed, the one that closes the
/// path): a zero-length segment has no direction.
fn without_repeats(points: &[[f32; 2]], closed: bool) -> Vec<[f32; 2]> {
    let mut out: Vec<[f32; 2]> = Vec::with_capacity(points.len());
    for &p in points {
        if out.last() != Some(&p) {
            out.push(p);
        }
    }
    if closed && out.len() > 1 && out.first() == out.last() {
        out.pop();
    }
    out
}

/// The rings that make up the stroke of one subpath.
fn stroke_rings(
    subpath: &Subpath,
    half: f32,
    cap: Cap,
    join: Join,
    rings: &mut Vec<Ring>,
) -> Result<()> {
    let closed = subpath.closed;
    let points = without_repeats(&subpath.points, closed);
    let n = points.len();
    match n {
        0 => {}
        1 => {
            // A single point has no direction: only a round cap draws anything for it.
            if cap == Cap::Round {
                rings.push(disc(points[0], half)?);
            }
        }
        _ => {
            let segments = if closed { n } else { n - 1 };
            for i in 0..segments {
                rings.push(segment_ring(points[i], points[(i + 1) % n], half)?);
            }
            let corners = if closed { 0..n } else { 1..n - 1 };
            for i in corners {
                let prev = points[(i + n - 1) % n];
                let next = points[(i + 1) % n];
                rings.extend(join_ring(prev, points[i], next, half, join)?);
            }
            if !closed {
                let outward_start = direction(points[1], points[0])?;
                rings.extend(cap_ring(points[0], outward_start, half, cap)?);
                let outward_end = direction(points[n - 2], points[n - 1])?;
                rings.extend(cap_ring(points[n - 1], outward_end, half, cap)?);
            }
        }
    }
    Ok(())
}

/// The rings whose non-zero union is the stroke of `subpaths`, which are in device coordinates
/// and are `half` wide on each side of their outline.
pub fn stroke_outline(subpaths: &[Subpath], half: f32, cap: Cap, join: Join) -> Result<Vec<Ring>> {
    let mut rings = Vec::new();
    for subpath in subpaths {
        stroke_rings(subpath, half, cap, join, &mut rings)?;
    }
    Ok(rings)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A square of side `size` with its corner at the origin.
    fn square(size: f32) -> Ring {
        vec![[0.0, 0.0], [size, 0.0], [size, size], [0.0, size]]
    }

    #[test]
    fn a_square_covers_the_pixels_whose_centres_it_contains() {
        let coverage = cover(&[square(2.0)], 4, 4).expect("a square is finite");
        let pixels: Vec<_> = coverage.pixels().collect();
        assert_eq!(pixels, vec![(0, 0), (1, 0), (0, 1), (1, 1)]);
    }

    #[test]
    fn a_square_is_clipped_to_the_image() {
        let ring = vec![[-2.0, -2.0], [2.0, -2.0], [2.0, 2.0], [-2.0, 2.0]];
        let coverage = cover(&[ring], 3, 3).expect("a square is finite");
        assert_eq!(coverage.pixels().count(), 4);
    }

    #[test]
    fn a_hole_with_the_opposite_winding_is_not_covered() {
        let outer = square(4.0);
        let hole = vec![[1.0, 1.0], [1.0, 3.0], [3.0, 3.0], [3.0, 1.0]];
        let coverage = cover(&[outer, hole], 4, 4).expect("the rings are finite");
        assert_eq!(coverage.pixels().count(), 12);
        assert!(!coverage.pixels().any(|p| p == (1, 1)));
    }

    #[test]
    fn nothing_is_covered_without_points() {
        let coverage = cover(&[], 4, 4).expect("no rings is finite");
        assert_eq!(coverage.pixels().count(), 0);
    }

    #[test]
    fn a_zero_length_path_has_no_stroke() {
        let subpath = Subpath {
            points: vec![[1.0, 1.0], [1.0, 1.0]],
            closed: false,
        };
        let rings =
            stroke_outline(&[subpath], 0.5, Cap::Square, Join::Miter).expect("the path is finite");
        assert_eq!(rings.len(), 0);
    }

    #[test]
    fn a_straight_stroke_covers_its_width() {
        let subpath = Subpath {
            points: vec![[0.0, 2.5], [6.0, 2.5]],
            closed: false,
        };
        let rings =
            stroke_outline(&[subpath], 1.0, Cap::Square, Join::Miter).expect("the path is finite");
        let coverage = cover(&rings, 8, 8).expect("the stroke is finite");
        assert_eq!(coverage.pixels().count(), 12);
    }

    fn stroke_of(points: Vec<[f32; 2]>, join: Join) -> Coverage {
        let subpath = Subpath {
            points,
            closed: false,
        };
        let rings = stroke_outline(&[subpath], 1.0, Cap::Square, join).expect("the path is finite");
        cover(&rings, 16, 16).expect("the stroke is finite")
    }

    #[test]
    fn a_sharp_miter_is_cut_as_a_bevel() {
        let sharp = vec![[0.0, 0.0], [10.0, 0.2], [0.0, 0.4]];
        assert_eq!(
            stroke_of(sharp.clone(), Join::Miter),
            stroke_of(sharp, Join::Bevel)
        );
    }

    #[test]
    fn a_right_angle_miter_reaches_the_outer_corner() {
        let corner = vec![[2.0, 2.0], [10.0, 2.0], [10.0, 10.0]];
        let mitred = stroke_of(corner.clone(), Join::Miter);
        let bevelled = stroke_of(corner, Join::Bevel);
        assert!(mitred.pixels().count() > bevelled.pixels().count());
    }

    #[test]
    fn a_non_finite_crossing_is_refused() {
        let ring = vec![[-3.0e38, 0.0], [3.0e38, 1.0], [0.0, 2.0]];
        assert!(cover(&[ring], 4, 4).is_err());
    }
}
