// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the Bézier and Catmull-Rom steps and of the refusal of a control value that
//! overflows. The control points lie on a line at 0, 1, 2 and 3, so every curve through them is
//! that line, and its points are worked out by hand from the formulas in `curve.rs`.

use super::{bezier_steps, curve_point, curve_steps, curve_tangent};

const INF: f32 = f32::INFINITY;

const LINE: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [3.0, 0.0]];

#[test]
fn a_catmull_rom_coordinate_with_an_infinite_control_value_is_refused() {
    assert!(curve_point(0.0, INF, 0.0, 0.0, 0.5, 0.0).is_err());
    assert!(curve_tangent(0.0, INF, 0.0, 0.0, 0.5, 0.0).is_err());
}

#[test]
fn a_bezier_step_with_an_infinite_coordinate_is_refused() {
    assert!(bezier_steps([[INF, 0.0], [0.0, 0.0], [0.0, 0.0], [0.0, 0.0]], 3).is_err());
    assert!(bezier_steps([[0.0, INF], [0.0, 0.0], [0.0, 0.0], [0.0, 0.0]], 3).is_err());
}

#[test]
fn a_catmull_rom_step_with_an_infinite_coordinate_is_refused() {
    assert!(curve_steps([[0.0, 0.0], [INF, 0.0], [0.0, 0.0], [0.0, 0.0]], 2, 0.0).is_err());
    assert!(curve_steps([[0.0, 0.0], [0.0, INF], [0.0, 0.0], [0.0, 0.0]], 2, 0.0).is_err());
}

#[test]
fn a_bezier_through_collinear_points_steps_evenly_and_ends_on_its_last_anchor() {
    assert_eq!(bezier_steps(LINE, 1).expect("finite"), vec![[3.0, 0.0]]);
    assert_eq!(
        bezier_steps(LINE, 4).expect("finite"),
        vec![[0.75, 0.0], [1.5, 0.0], [2.25, 0.0], [3.0, 0.0]]
    );
}

#[test]
fn a_catmull_rom_segment_through_collinear_points_is_the_line_between_its_middle_points() {
    assert_eq!(
        curve_point(0.0, 1.0, 2.0, 3.0, 0.5, 0.0).expect("finite"),
        1.5
    );
    assert_eq!(
        curve_steps(LINE, 2, 0.0).expect("finite"),
        vec![[1.5, 0.0], [2.0, 0.0]]
    );
}
