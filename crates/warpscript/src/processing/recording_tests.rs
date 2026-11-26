// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the recorded shapes: the outline and the contours that each kind of `beginShape`
//! makes, and the points of the Bézier, quadratic and curve vertices. The shapes follow the doc
//! comments of `recording.rs`. The curve points are the formulas at `t = 1/2`, worked out by hand.

use super::{Kind, NO_VERTEX, Recording};
use crate::error::Error;
use crate::processing::draw::Outline;
use crate::processing::path::Subpath;

const A: [f32; 2] = [0.0, 0.0];
const B: [f32; 2] = [10.0, 0.0];
const C: [f32; 2] = [0.0, 10.0];
const D: [f32; 2] = [-10.0, 0.0];
const E: [f32; 2] = [10.0, 10.0];

/// A shape of `kind` with the vertices given, in order.
fn shape(kind: Kind, vertices: &[[f32; 2]]) -> Recording {
    let mut recording = Recording::begin(kind);
    for &vertex in vertices {
        recording.vertex(vertex);
    }
    recording
}

/// The outline of closed rings: each is filled as it is, and stroked closed.
fn filled(rings: Vec<Vec<[f32; 2]>>) -> Outline {
    Outline {
        fill: rings.clone(),
        stroke: rings
            .into_iter()
            .map(|points| Subpath {
                points,
                closed: true,
            })
            .collect(),
        points: Vec::new(),
    }
}

/// A polygon of `A`, `B` and `C`, then a contour of `D` and `E`, then `B` and `C` again.
fn polygon_with_a_contour() -> Recording {
    let mut recording = shape(Kind::Polygon, &[A, B, C]);
    recording.begin_contour();
    recording.vertex(D);
    recording.vertex(E);
    recording.end_contour();
    recording.vertex(B);
    recording.vertex(C);
    recording
}

/// Whether two lists of points agree, each coordinate to within a small tolerance: the curves
/// pass through the rounding of `2 / 3`.
fn assert_close(actual: &[[f32; 2]], expected: &[[f32; 2]]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?}");
    for (a, e) in actual.iter().zip(expected) {
        assert!(
            (a[0] - e[0]).abs() < 1e-4 && (a[1] - e[1]).abs() < 1e-4,
            "{actual:?} is not {expected:?}"
        );
    }
}

#[test]
fn points_are_drawn_as_dots() {
    let recording = shape(Kind::Points, &[A, B, C]);
    assert_eq!(recording.kind(), Kind::Points);
    assert_eq!(
        recording.finish(false),
        Outline {
            points: vec![A, B, C],
            ..Outline::default()
        }
    );
}

#[test]
fn lines_join_each_pair_of_vertices_and_drop_an_odd_one() {
    let outline = shape(Kind::Lines, &[A, B, C]).finish(false);
    assert_eq!(
        outline,
        Outline {
            stroke: vec![Subpath {
                points: vec![A, B],
                closed: false,
            }],
            ..Outline::default()
        }
    );
}

#[test]
fn triangles_take_each_three_vertices() {
    // A, B and C run counterclockwise, so the fill keeps their order.
    let outline = shape(Kind::Triangles, &[A, B, C, D]).finish(false);
    assert_eq!(outline, filled(vec![vec![A, B, C]]));
}

#[test]
fn a_clockwise_triangle_is_filled_reversed_and_stroked_as_it_was_drawn() {
    // A, C and B run clockwise, so the fill is the reverse, and the stroke keeps the order.
    let outline = shape(Kind::Triangles, &[A, C, B]).finish(false);
    assert_eq!(
        outline,
        Outline {
            fill: vec![vec![B, C, A]],
            stroke: vec![Subpath {
                points: vec![A, C, B],
                closed: true,
            }],
            points: Vec::new(),
        }
    );
}

#[test]
fn a_triangle_strip_overlaps_each_triangle_with_the_two_before() {
    let outline = shape(Kind::TriangleStrip, &[A, B, C, D]).finish(false);
    assert_eq!(outline, filled(vec![vec![A, B, C], vec![B, C, D]]));
}

#[test]
fn a_triangle_fan_turns_about_its_first_vertex() {
    let outline = shape(Kind::TriangleFan, &[A, B, C, D]).finish(false);
    assert_eq!(outline, filled(vec![vec![A, B, C], vec![A, C, D]]));
}

#[test]
fn a_triangle_fan_with_fewer_than_three_vertices_has_no_triangle() {
    assert_eq!(
        shape(Kind::TriangleFan, &[]).finish(false),
        Outline::default()
    );
    assert_eq!(
        shape(Kind::TriangleFan, &[A]).finish(false),
        Outline::default()
    );
    assert_eq!(
        shape(Kind::TriangleFan, &[A, B]).finish(false),
        Outline::default()
    );
}

#[test]
fn quads_take_each_four_vertices() {
    let outline = shape(Kind::Quads, &[A, B, E, C, D]).finish(false);
    assert_eq!(outline, filled(vec![vec![A, B, E, C]]));
}

#[test]
fn a_quad_strip_pairs_its_vertices_along_two_sides() {
    let outline = shape(Kind::QuadStrip, &[A, B, C, E, [0.0, 20.0], [10.0, 20.0]]).finish(false);
    assert_eq!(
        outline,
        filled(vec![
            vec![A, B, E, C],
            vec![C, E, [10.0, 20.0], [0.0, 20.0]],
        ])
    );
}

#[test]
fn a_polygon_is_outlined_by_its_pieces_and_then_its_contours() {
    let closed = polygon_with_a_contour().finish(true);
    assert_eq!(closed, filled(vec![vec![A, B, C], vec![B, C], vec![D, E]]));

    let open = polygon_with_a_contour().finish(false);
    assert_eq!(
        open,
        Outline {
            fill: vec![vec![A, B, C], vec![B, C], vec![D, E]],
            stroke: vec![
                Subpath {
                    points: vec![A, B, C],
                    closed: false,
                },
                Subpath {
                    points: vec![B, C],
                    closed: false,
                },
                Subpath {
                    points: vec![D, E],
                    closed: true,
                },
            ],
            points: Vec::new(),
        }
    );
}

#[test]
fn ending_no_contour_changes_nothing_and_an_empty_contour_is_not_recorded() {
    let mut recording = shape(Kind::Polygon, &[A, B]);
    recording.end_contour();
    recording.begin_contour();
    recording.end_contour();
    recording.vertex(C);
    assert_eq!(
        recording.finish(false),
        Outline {
            fill: vec![vec![A, B], vec![C]],
            stroke: vec![
                Subpath {
                    points: vec![A, B],
                    closed: false,
                },
                Subpath {
                    points: vec![C],
                    closed: false,
                },
            ],
            points: Vec::new(),
        }
    );
}

#[test]
fn a_bezier_vertex_is_flattened_from_the_last_vertex() {
    // The midpoint of the cubic from (0, 0) through (0, 8) and (8, 8) to (8, 0) is (4, 6).
    let mut recording = shape(Kind::Polygon, &[A]);
    recording
        .bezier_vertex([[0.0, 8.0], [8.0, 8.0]], [8.0, 0.0], 2)
        .expect("a bezier");
    let outline = recording.finish(false);
    assert_eq!(
        outline.stroke,
        vec![Subpath {
            points: vec![A, [4.0, 6.0], [8.0, 0.0]],
            closed: false,
        }]
    );
}

#[test]
fn a_quadratic_vertex_is_drawn_as_the_cubic_with_the_same_shape() {
    // The midpoint of the quadratic from (0, 0) through (8, 8) to (16, 0) is (8, 4).
    let mut recording = shape(Kind::Polygon, &[A]);
    recording
        .quadratic_vertex([8.0, 8.0], [16.0, 0.0], 2)
        .expect("a quadratic");
    let outline = recording.finish(false);
    assert_close(&outline.stroke[0].points, &[A, [8.0, 4.0], [16.0, 0.0]]);
}

#[test]
fn a_bezier_or_quadratic_vertex_needs_a_vertex_before_it() {
    let mut recording = Recording::begin(Kind::Polygon);
    let err = recording
        .bezier_vertex([A, A], A, 2)
        .expect_err("no vertex before");
    assert_eq!(err.to_string(), NO_VERTEX);

    let err = recording
        .quadratic_vertex(A, A, 2)
        .expect_err("no vertex before");
    assert_eq!(err.to_string(), NO_VERTEX);
}

#[test]
fn a_bezier_that_overflows_is_refused() {
    let mut recording = shape(Kind::Polygon, &[A]);
    let err = recording
        .bezier_vertex([[f32::MAX, 0.0], [0.0, 0.0]], A, 2)
        .expect_err("the curve overflows");
    assert!(
        matches!(err, Error::NotFinite(_)),
        "unexpected error: {err}"
    );
}

#[test]
fn a_quadratic_whose_control_points_overflow_is_refused() {
    // Each coordinate of each of the two control points overflows in one of the cases.
    for (start, control, end) in [
        ([-f32::MAX, 0.0], [f32::MAX, 0.0], A),
        (A, [f32::MAX, 0.0], [-f32::MAX, 0.0]),
        ([0.0, -f32::MAX], [0.0, f32::MAX], A),
        (A, [0.0, f32::MAX], [0.0, -f32::MAX]),
    ] {
        let mut recording = shape(Kind::Polygon, &[start]);
        let err = recording
            .quadratic_vertex(control, end, 2)
            .expect_err("the curve overflows");
        assert!(
            matches!(err, Error::NotFinite(_)),
            "unexpected error: {err}"
        );
    }
}

#[test]
fn a_curve_vertex_is_drawn_from_the_second_control_point_to_the_third() {
    // The middle point of the segment from (0, 0) to (16, 0), shaped by (0, 0) and (32, 0), is
    // 7 with the uniform tightness 0 and 8 with tightness 1.
    for (tightness, middle) in [(0.0, 7.0), (1.0, 8.0)] {
        let mut recording = Recording::begin(Kind::Polygon);
        for control in [A, A, [16.0, 0.0], [32.0, 0.0]] {
            recording
                .curve_vertex(control, 2, tightness)
                .expect("a curve");
        }
        let outline = recording.finish(false);
        assert_eq!(
            outline.stroke[0].points,
            vec![A, [middle, 0.0], [16.0, 0.0]],
            "tightness {tightness}"
        );
    }
}

#[test]
fn each_later_curve_vertex_adds_a_segment_to_the_curve() {
    // With one step a segment, the curve visits each control point after the second.
    let mut recording = Recording::begin(Kind::Polygon);
    for control in [A, A, B, [20.0, 0.0], [30.0, 0.0]] {
        recording.curve_vertex(control, 1, 0.0).expect("a curve");
    }
    let outline = recording.finish(false);
    assert_eq!(outline.stroke[0].points, vec![A, B, [20.0, 0.0]]);
}

#[test]
fn a_curve_that_overflows_is_refused() {
    let mut recording = Recording::begin(Kind::Polygon);
    recording.curve_vertex(A, 2, 0.0).expect("a control point");
    recording
        .curve_vertex([f32::MAX, 0.0], 2, 0.0)
        .expect("a control point");
    recording
        .curve_vertex([16.0, 0.0], 2, 0.0)
        .expect("a control point");
    let err = recording
        .curve_vertex([32.0, 0.0], 2, 0.0)
        .expect_err("the curve overflows");
    assert!(
        matches!(err, Error::NotFinite(_)),
        "unexpected error: {err}"
    );
}
