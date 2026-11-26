// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the outlines of the primitives: the placement of a shape by its mode, the refusal of
//! a coordinate or a size that is not finite or overflows once the shape is placed, and the points
//! of a curve. The values are worked out by hand from the formulas in `primitive.rs`.

use std::f32::consts::FRAC_PI_2;

use super::{ArcMode, arc, at, bezier, ellipse, line, place, point, quad, rect, spline, triangle};
use crate::processing::Mode;
use crate::processing::draw::Outline;
use crate::processing::path::Subpath;

const MAX: f32 = f32::MAX;
const INF: f32 = f32::INFINITY;
const NAN: f32 = f32::NAN;

const LINE: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [3.0, 0.0]];

#[test]
fn a_placement_whose_coordinates_overflow_is_refused() {
    assert!(place(Mode::Corners, MAX, -MAX).is_err());
    assert!(place(Mode::Center, INF, 1.0).is_err());
    assert!(place(Mode::Radius, INF, 1.0).is_err());
    assert!(place(Mode::Radius, 0.0, MAX).is_err());
}

#[test]
fn a_placement_gives_the_start_and_the_size_of_its_mode() {
    assert_eq!(place(Mode::Corner, 1.0, 4.0).expect("finite"), (1.0, 4.0));
    assert_eq!(place(Mode::Corners, 1.0, 4.0).expect("finite"), (1.0, 3.0));
    assert_eq!(place(Mode::Center, 5.0, 4.0).expect("finite"), (3.0, 4.0));
    assert_eq!(place(Mode::Radius, 5.0, 2.0).expect("finite"), (3.0, 4.0));
}

#[test]
fn a_point_with_a_coordinate_that_is_not_finite_is_refused() {
    assert!(at(NAN, 0.0).is_err());
    assert!(at(0.0, NAN).is_err());
    assert!(point(NAN, 0.0).is_err());
    assert!(line([NAN, 0.0], [0.0, 0.0]).is_err());
    assert!(line([0.0, 0.0], [NAN, 0.0]).is_err());
    assert!(triangle([[NAN, 0.0], [0.0, 0.0], [0.0, 0.0]]).is_err());
    assert!(quad([[0.0, 0.0], [NAN, 0.0], [0.0, 0.0], [0.0, 0.0]]).is_err());
}

#[test]
fn a_line_is_a_stroke_between_its_two_ends() {
    assert_eq!(
        line([0.0, 0.0], [3.0, 4.0]).expect("finite"),
        Outline {
            stroke: vec![Subpath {
                points: vec![[0.0, 0.0], [3.0, 4.0]],
                closed: false,
            }],
            ..Outline::default()
        }
    );
}

#[test]
fn a_rectangle_whose_far_corner_overflows_is_refused() {
    assert!(rect(MAX, 0.0, MAX, 1.0, [0.0; 4]).is_err());
    assert!(rect(0.0, MAX, 1.0, MAX, [0.0; 4]).is_err());
}

#[test]
fn a_rounded_corner_whose_arc_overflows_is_refused_at_each_corner() {
    for radii in [
        [MAX, 0.0, 0.0, 0.0],
        [0.0, MAX, 0.0, 0.0],
        [0.0, 0.0, MAX, 0.0],
        [0.0, 0.0, 0.0, MAX],
    ] {
        assert!(rect(0.0, 0.0, MAX, MAX, radii).is_err(), "{radii:?}");
    }
}

#[test]
fn a_rectangle_given_by_negative_sizes_is_the_same_as_its_two_corners() {
    assert_eq!(
        rect(4.0, 2.0, -4.0, -2.0, [0.0; 4]).expect("finite"),
        rect(0.0, 0.0, 4.0, 2.0, [0.0; 4]).expect("finite")
    );
}

#[test]
fn an_ellipse_whose_centre_or_radius_overflows_is_refused() {
    assert!(ellipse(MAX, 0.0, MAX, 0.0).is_err());
    assert!(ellipse(0.0, MAX, 0.0, MAX).is_err());
    assert!(ellipse(0.0, 0.0, MAX, MAX).is_err());
}

#[test]
fn an_ellipse_is_a_closed_ring_of_points_on_its_box() {
    let outline = ellipse(0.0, 0.0, 2.0, 2.0).expect("finite");
    assert_eq!(outline.fill.len(), 1);
    assert_eq!(outline.fill[0].len(), 14);
    assert_eq!(outline.fill[0][0], [2.0, 1.0]);
}

#[test]
fn an_arc_whose_centre_or_sweep_overflows_is_refused() {
    assert!(arc([MAX, 0.0, MAX, 0.0], 0.0, 1.0, ArcMode::Pie).is_err());
    assert!(arc([0.0, 0.0, MAX, MAX], 0.0, INF, ArcMode::Pie).is_err());
    assert!(arc([0.0, 0.0, MAX, MAX], 0.0, 2.0, ArcMode::Pie).is_err());
}

#[test]
fn a_pie_arc_starts_at_its_centre_and_then_at_the_angle_it_starts_from() {
    let outline = arc([0.0, 0.0, 4.0, 4.0], 0.0, FRAC_PI_2, ArcMode::Pie).expect("finite");
    assert_eq!(outline.fill[0][0], [2.0, 2.0]);
    assert_eq!(outline.fill[0][1], [4.0, 2.0]);
}

#[test]
fn a_bezier_with_a_point_that_is_not_finite_is_refused() {
    assert!(bezier([[NAN, 0.0], [0.0, 0.0], [0.0, 0.0], [0.0, 0.0]], 2).is_err());
    assert!(bezier([[0.0, 0.0], [INF, 0.0], [0.0, 0.0], [0.0, 0.0]], 2).is_err());
}

#[test]
fn a_bezier_through_collinear_points_is_the_line_in_its_steps() {
    assert_eq!(
        bezier(LINE, 4).expect("finite"),
        Outline::open(vec![
            [0.0, 0.0],
            [0.75, 0.0],
            [1.5, 0.0],
            [2.25, 0.0],
            [3.0, 0.0],
        ])
    );
}

#[test]
fn a_spline_with_a_point_that_is_not_finite_is_refused() {
    assert!(spline([[0.0, 0.0], [NAN, 0.0], [0.0, 0.0], [0.0, 0.0]], 2, 0.0).is_err());
    assert!(spline([[INF, 0.0], [0.0, 0.0], [0.0, 0.0], [0.0, 0.0]], 2, 0.0).is_err());
}

#[test]
fn a_spline_starts_at_its_second_point_and_steps_to_its_third() {
    assert_eq!(
        spline(LINE, 2, 0.0).expect("finite"),
        Outline::open(vec![[1.0, 0.0], [1.5, 0.0], [2.0, 0.0]])
    );
}
