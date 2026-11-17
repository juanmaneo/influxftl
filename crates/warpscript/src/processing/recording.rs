// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The shape that `beginShape` starts and `endShape` draws. Its vertices are recorded in the
//! coordinates of the script, as given: the transform applies when the shape is drawn, as Java2D
//! does. Bézier and curve vertices are flattened when they are recorded, with the detail the style
//! has then.

use super::curve::{bezier_steps, curve_steps};
use super::draw::Outline;
use super::finite;
use super::path::{Ring, Subpath, positive};
use crate::error::Result;
use crate::error::processing::ProcessingError;

/// Warp 10's message for a Bézier or quadratic vertex when no vertex comes before it.
pub const NO_VERTEX: &str =
    "vertex() must be used at least once before bezierVertex() or quadraticVertex()";

/// The kinds of shape `beginShape` takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `POLYGON`: the vertices outline a polygon, which may have contours.
    Polygon,
    /// `POINTS`: each vertex is a point.
    Points,
    /// `LINES`: each pair of vertices is a line.
    Lines,
    /// `TRIANGLES`: each three vertices are a triangle.
    Triangles,
    /// `TRIANGLE_STRIP`: each vertex after the second makes a triangle with the two before it.
    TriangleStrip,
    /// `TRIANGLE_FAN`: each vertex after the second makes a triangle with the first vertex and
    /// the one before it.
    TriangleFan,
    /// `QUADS`: each four vertices are a quadrilateral.
    Quads,
    /// `QUAD_STRIP`: each pair of vertices after the first two makes a quadrilateral with the
    /// pair before it.
    QuadStrip,
}

/// A shape between its `beginShape` and its `endShape`.
#[derive(Clone, Debug)]
pub struct Recording {
    kind: Kind,
    /// The vertices of the subpath being recorded.
    current: Vec<[f32; 2]>,
    /// The subpaths of the outline that `beginContour` interrupted or that were not contours.
    outline: Vec<Vec<[f32; 2]>>,
    /// The contours: subpaths that are holes or inner outlines, each closed.
    contours: Vec<Vec<[f32; 2]>>,
    /// Whether the subpath being recorded is a contour.
    in_contour: bool,
    /// The last vertex recorded, which a Bézier vertex starts from.
    last: Option<[f32; 2]>,
    /// The control points of the curve vertices, all of them since `beginShape`.
    curve: Vec<[f32; 2]>,
}

/// Two thirds of the way from `from` to `control`: the cubic control point that a quadratic one
/// becomes.
pub(crate) fn two_thirds_toward(from: [f32; 2], control: [f32; 2]) -> Result<[f32; 2]> {
    Ok([
        finite(from[0] + 2.0 / 3.0 * (control[0] - from[0]))?,
        finite(from[1] + 2.0 / 3.0 * (control[1] - from[1]))?,
    ])
}

/// The polygons of a shape that is not a `POLYGON`: each is filled and stroked as closed, and
/// filled positively so that overlapping polygons add up rather than cancel.
fn polygons(list: Vec<Ring>) -> Outline {
    let fill = list.iter().map(|ring| positive(ring.clone())).collect();
    let stroke = list
        .into_iter()
        .map(|points| Subpath {
            points,
            closed: true,
        })
        .collect();
    Outline {
        fill,
        stroke,
        points: Vec::new(),
    }
}

impl Recording {
    /// A new shape of `kind`, with no vertex.
    pub fn begin(kind: Kind) -> Self {
        Recording {
            kind,
            current: Vec::new(),
            outline: Vec::new(),
            contours: Vec::new(),
            in_contour: false,
            last: None,
            curve: Vec::new(),
        }
    }

    /// The kind of shape being recorded.
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// A vertex (`vertex`).
    pub fn vertex(&mut self, point: [f32; 2]) {
        self.current.push(point);
        self.last = Some(point);
    }

    /// Close the subpath being recorded, as an outline piece or as a contour.
    fn end_piece(&mut self) {
        let points = std::mem::take(&mut self.current);
        if points.is_empty() {
            return;
        }
        if self.in_contour {
            self.contours.push(points);
        } else {
            self.outline.push(points);
        }
    }

    /// `beginContour`: the vertices after it form a contour, a hole in the outline.
    pub fn begin_contour(&mut self) {
        self.end_piece();
        self.in_contour = true;
    }

    /// `endContour`: the contour is closed, and the vertices after it belong to the outline again.
    /// Without a contour open, there is nothing to end.
    pub fn end_contour(&mut self) {
        if self.in_contour {
            self.end_piece();
            self.in_contour = false;
        }
    }

    /// `curveVertex`: a control point of a Catmull-Rom spline. The first control point only
    /// shapes the curve: each segment runs between two control points and is drawn once the
    /// point after it is known, in `detail` steps with the `tightness` of the style.
    pub fn curve_vertex(&mut self, point: [f32; 2], detail: usize, tightness: f32) -> Result<()> {
        self.curve.push(point);
        let n = self.curve.len();
        if n < 4 {
            return Ok(());
        }
        if n == 4 {
            // The curve starts at the second control point.
            let start = self.curve[1];
            self.vertex(start);
        }
        let [p0, p1, p2, p3] = [
            self.curve[n - 4],
            self.curve[n - 3],
            self.curve[n - 2],
            self.curve[n - 1],
        ];
        for step in curve_steps([p0, p1, p2, p3], detail, tightness)? {
            self.vertex(step);
        }
        Ok(())
    }

    /// The vertex a Bézier or quadratic vertex starts from.
    fn last_vertex(&self) -> Result<[f32; 2]> {
        let Some(last) = self.last else {
            return Err(ProcessingError::ShapeState { message: NO_VERTEX }.into());
        };
        Ok(last)
    }

    /// `bezierVertex`: a cubic Bézier curve from the last vertex to `end`, through its two
    /// control points, in `detail` steps.
    pub fn bezier_vertex(
        &mut self,
        controls: [[f32; 2]; 2],
        end: [f32; 2],
        detail: usize,
    ) -> Result<()> {
        let start = self.last_vertex()?;
        for step in bezier_steps([start, controls[0], controls[1], end], detail)? {
            self.vertex(step);
        }
        Ok(())
    }

    /// `quadraticVertex`: a quadratic Bézier curve from the last vertex to `end`, through the
    /// control point `control`. It is drawn as the cubic curve with the same shape.
    pub fn quadratic_vertex(
        &mut self,
        control: [f32; 2],
        end: [f32; 2],
        detail: usize,
    ) -> Result<()> {
        let start = self.last_vertex()?;
        let controls = [
            two_thirds_toward(start, control)?,
            two_thirds_toward(end, control)?,
        ];
        self.bezier_vertex(controls, end, detail)
    }

    /// `endShape`: the outline of the recorded vertices. A `POLYGON` is closed when `close`
    /// (`CLOSE`); its contours are always closed. The other kinds draw as Processing does: a
    /// point for each vertex, a line for each pair, and a polygon for each group of vertices.
    pub fn finish(mut self, close: bool) -> Outline {
        let points = &self.current;
        match self.kind {
            Kind::Polygon => {
                self.end_piece();
                let mut stroke: Vec<Subpath> = self
                    .outline
                    .into_iter()
                    .map(|points| Subpath {
                        points,
                        closed: close,
                    })
                    .collect();
                stroke.extend(self.contours.into_iter().map(|points| Subpath {
                    points,
                    closed: true,
                }));
                let fill = stroke.iter().map(|s| s.points.clone()).collect();
                Outline {
                    fill,
                    stroke,
                    points: Vec::new(),
                }
            }
            Kind::Points => Outline {
                points: self.current,
                ..Outline::default()
            },
            Kind::Lines => Outline {
                stroke: points
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| Subpath {
                        points: pair.to_vec(),
                        closed: false,
                    })
                    .collect(),
                ..Outline::default()
            },
            Kind::Triangles => polygons(
                points
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .map(|t| t.to_vec())
                    .collect(),
            ),
            Kind::TriangleStrip => polygons(points.windows(3).map(<[_]>::to_vec).collect()),
            Kind::TriangleFan => polygons(fan(points)),
            Kind::Quads => polygons(
                points
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|q| q.to_vec())
                    .collect(),
            ),
            Kind::QuadStrip => polygons(quad_strip(points)),
        }
    }
}

/// The triangles of a `TRIANGLE_FAN`: each one has the first vertex and two consecutive others.
fn fan(points: &[[f32; 2]]) -> Vec<Ring> {
    let Some((&hub, rest)) = points.split_first() else {
        return Vec::new();
    };
    rest.windows(2)
        .map(|pair| vec![hub, pair[0], pair[1]])
        .collect()
}

/// The quadrilaterals of a `QUAD_STRIP`: the vertices come in pairs along the two sides of the
/// strip, and each quadrilateral takes two consecutive pairs.
fn quad_strip(points: &[[f32; 2]]) -> Vec<Ring> {
    let count = points.len().saturating_sub(2) / 2;
    (0..count)
        .map(|i| {
            let k = 2 * i;
            vec![points[k], points[k + 1], points[k + 3], points[k + 2]]
        })
        .collect()
}
